//! Mints shared across a process: a few at once, the recent ones kept, and one
//! mint for everyone asking for the same badge.
//!
//! Every binding mints through here, so a list of badges, a view built twice
//! or a still taken of what a view shows strikes each document once. A mint
//! runs on a thread of its own, never the caller's; a request is a future (or
//! [`Request::wait`]) that the caller can give up on, and the engine is told
//! to stop once everyone waiting for a mint has.

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};

use crate::{Badge, CancelToken, Error, MintOptions, mint_with};

/// The process's mints: two at once, the last eight badges kept.
pub fn shared() -> &'static Mints {
    static SHARED: OnceLock<Mints> = OnceLock::new();
    SHARED.get_or_init(|| Mints::new(2, 8))
}

/// Asks the process's mints for a badge: [`Mints::request`] on [`shared`].
pub fn request(svg: Vec<u8>, options: MintOptions) -> Request {
    shared().request(svg, options)
}

#[derive(Clone)]
pub struct Mints(Arc<Inner>);

struct Inner {
    running: usize,
    kept: usize,
    state: Mutex<State>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    svg: Arc<[u8]>,
    options: MintOptions,
}

#[derive(Default)]
struct State {
    flights: HashMap<Key, Flight>,
    /// Flights waiting for a slot, oldest first. One may have been given up
    /// on since, or given up on and asked for again: the serial tells.
    queue: VecDeque<(Key, u64)>,
    running: usize,
    /// Least recently asked for first.
    recent: VecDeque<(Key, Arc<Badge>)>,
    serial: u64,
}

struct Flight {
    serial: u64,
    token: Arc<CancelToken>,
    started: bool,
    waiting: HashMap<u64, Arc<Slot>>,
}

type Outcome = Result<Arc<Badge>, Error>;

/// Where a mint's outcome is left for one request.
#[derive(Default)]
struct Slot {
    state: Mutex<SlotState>,
    ready: Condvar,
}

#[derive(Default)]
struct SlotState {
    outcome: Option<Outcome>,
    waker: Option<Waker>,
}

impl Slot {
    fn settle(&self, outcome: Outcome) {
        let mut state = self.state.lock().unwrap();
        if state.outcome.is_some() {
            return;
        }
        state.outcome = Some(outcome);
        let waker = state.waker.take();
        drop(state);
        self.ready.notify_all();
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl Mints {
    /// `running` mints at once; `kept` badges kept once struck.
    pub fn new(running: usize, kept: usize) -> Self {
        Self(Arc::new(Inner { running: running.max(1), kept, state: Mutex::default() }))
    }

    /// Asks for the badge `svg` strikes with `options`. A badge struck lately
    /// is ready at once, one being struck is waited for, and anything else
    /// waits for a slot.
    pub fn request(&self, svg: Vec<u8>, options: MintOptions) -> Request {
        let key = Key { svg: svg.into(), options };
        let slot = Arc::new(Slot::default());
        let mut state = self.0.state.lock().unwrap();
        state.serial += 1;
        let id = state.serial;
        if let Some(at) = state.recent.iter().position(|(k, _)| *k == key) {
            let entry = state.recent.remove(at).unwrap();
            slot.settle(Ok(entry.1.clone()));
            state.recent.push_back(entry);
            return Request { mints: self.clone(), key, id, slot };
        }
        if !state.flights.contains_key(&key) {
            state.serial += 1;
            let serial = state.serial;
            state.flights.insert(
                key.clone(),
                Flight { serial, token: Arc::default(), started: false, waiting: HashMap::new() },
            );
            state.queue.push_back((key.clone(), serial));
        }
        state.flights.get_mut(&key).unwrap().waiting.insert(id, slot.clone());
        self.start(&mut state);
        Request { mints: self.clone(), key, id, slot }
    }

    /// Starts what is queued while there are slots for it.
    fn start(&self, state: &mut State) {
        while state.running < self.0.running {
            let Some((key, serial)) = state.queue.pop_front() else { return };
            let Some(flight) = state.flights.get_mut(&key) else { continue };
            if flight.serial != serial || flight.started {
                continue;
            }
            flight.started = true;
            state.running += 1;
            let token = flight.token.clone();
            let mints = self.clone();
            std::thread::Builder::new()
                .name("luster-mint".into())
                .spawn(move || {
                    let outcome = catch_unwind(AssertUnwindSafe(|| {
                        mint_with(&key.svg, &key.options, &token)
                    }))
                    .unwrap_or_else(|panic| Err(Error::Internal(panic_message(&panic))))
                    .map(Arc::new);
                    mints.finish(key, serial, outcome);
                })
                .expect("a thread to mint on");
        }
    }

    fn finish(&self, key: Key, serial: u64, outcome: Outcome) {
        let mut state = self.0.state.lock().unwrap();
        state.running -= 1;
        if let Ok(badge) = &outcome {
            state.recent.retain(|(k, _)| *k != key);
            state.recent.push_back((key.clone(), badge.clone()));
            while state.recent.len() > self.0.kept {
                state.recent.pop_front();
            }
        }
        // A flight given up on has been taken off already, and one asked for
        // again since is not this one. But a badge struck after all, its
        // stop having come too late, is the one the newer flight is striking:
        // that one is answered with it and stopped.
        let waiting = match state.flights.get(&key) {
            Some(flight) if flight.serial == serial || outcome.is_ok() => {
                let flight = state.flights.remove(&key).unwrap();
                if flight.serial != serial {
                    flight.token.cancel();
                }
                flight.waiting
            }
            _ => HashMap::new(),
        };
        self.start(&mut state);
        drop(state);
        for slot in waiting.into_values() {
            slot.settle(outcome.clone());
        }
    }

    /// Request `id` no longer wants its badge. The last to give up on a mint
    /// stops the engine, and the next to ask starts afresh rather than join a
    /// mint that is stopping.
    fn abandon(&self, key: &Key, id: u64) {
        let mut state = self.0.state.lock().unwrap();
        let Some(flight) = state.flights.get_mut(key) else { return };
        if flight.waiting.remove(&id).is_none() || !flight.waiting.is_empty() {
            return;
        }
        flight.token.cancel();
        state.flights.remove(key);
    }

    /// How many mints are running and waiting, for tests.
    #[cfg(test)]
    fn load(&self) -> (usize, usize) {
        let state = self.0.state.lock().unwrap();
        (state.running, state.flights.len())
    }
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "the engine stopped".into())
}

/// One ask for a badge. Dropping it, or [`Request::cancel`], gives it up.
pub struct Request {
    mints: Mints,
    key: Key,
    id: u64,
    slot: Arc<Slot>,
}

impl Request {
    /// Gives the badge up: what waits for it ends with [`Error::Cancelled`],
    /// and the engine stops if nobody else wants it. Nothing once it has come.
    pub fn cancel(&self) {
        self.mints.abandon(&self.key, self.id);
        self.slot.settle(Err(Error::Cancelled));
    }

    /// The badge, once it has come. Asking again gives the same outcome.
    pub fn poll_badge(&self, cx: &mut Context<'_>) -> Poll<Outcome> {
        let mut state = self.slot.state.lock().unwrap();
        match &state.outcome {
            Some(outcome) => Poll::Ready(outcome.clone()),
            None => {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    /// Waits for the badge.
    pub fn badge(&self) -> impl Future<Output = Outcome> + Send + '_ {
        std::future::poll_fn(|cx| self.poll_badge(cx))
    }

    /// Waits for the badge, blocking the calling thread.
    pub fn wait(&self) -> Outcome {
        let mut state = self.slot.state.lock().unwrap();
        loop {
            if let Some(outcome) = &state.outcome {
                return outcome.clone();
            }
            state = self.slot.ready.wait(state).unwrap();
        }
    }
}

impl Future for Request {
    type Output = Outcome;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Outcome> {
        self.poll_badge(cx)
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        self.mints.abandon(&self.key, self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
        <rect width="100" height="100" fill="#c33"/>
        <circle cx="50" cy="50" r="20" fill="none" stroke="#36c" stroke-width="6"/>
    </svg>"##;

    fn svg(n: usize) -> Vec<u8> {
        // A different document each time: a comment changes its bytes.
        format!("{SQUARE}<!-- {n} -->").into_bytes()
    }

    #[test]
    fn asking_twice_strikes_once() {
        let mints = Mints::new(2, 8);
        let a = mints.request(svg(0), MintOptions::default());
        let b = mints.request(svg(0), MintOptions::default());
        assert_eq!(mints.load().1, 1);
        let (a, b) = (a.wait().unwrap(), b.wait().unwrap());
        assert!(Arc::ptr_eq(&a, &b));
        // Struck lately: ready at once.
        let c = mints.request(svg(0), MintOptions::default());
        assert!(Arc::ptr_eq(&a, &c.wait().unwrap()));
        let other = MintOptions { metal_lines: true, ..Default::default() };
        let d = mints.request(svg(0), other).wait().unwrap();
        assert!(!Arc::ptr_eq(&a, &d));
    }

    #[test]
    fn a_few_at_once() {
        let mints = Mints::new(2, 8);
        let requests: Vec<_> =
            (0..5).map(|n| mints.request(svg(n), MintOptions::default())).collect();
        assert_eq!(mints.load(), (2, 5));
        for r in &requests {
            r.wait().unwrap();
        }
        assert_eq!(mints.load(), (0, 0));
    }

    #[test]
    fn giving_up_stops_only_when_everyone_has() {
        let mints = Mints::new(1, 8);
        let first = mints.request(svg(0), MintOptions::default());
        let a = mints.request(svg(1), MintOptions::default());
        let b = mints.request(svg(1), MintOptions::default());
        a.cancel();
        assert_eq!(a.wait().err(), Some(Error::Cancelled));
        assert_eq!(mints.load().1, 2);
        drop(b);
        assert_eq!(mints.load().1, 1);
        first.wait().unwrap();
        // Asked for again after everyone gave up: struck afresh.
        mints.request(svg(1), MintOptions::default()).wait().unwrap();
    }

    #[test]
    fn a_badge_struck_after_all_answers_the_ask_made_since() {
        let mints = Mints::new(1, 8);
        // The one slot held for the whole test: a real mint in it could
        // finish early and start the flight below.
        mints.0.state.lock().unwrap().running += 1;
        // Queued, given up on, and asked for again: a second flight for the
        // same badge.
        drop(mints.request(svg(1), MintOptions::default()));
        let again = mints.request(svg(1), MintOptions::default());
        let key = Key { svg: svg(1).into(), options: MintOptions::default() };
        let token = mints.0.state.lock().unwrap().flights[&key].token.clone();
        // The given-up flight comes back with its badge after all, its stop
        // too late (as if it had been running; serial 0 is no flight's now).
        let badge = Arc::new(mint_with(&svg(1), &MintOptions::default(), &CancelToken::new()).unwrap());
        mints.0.state.lock().unwrap().running += 1;
        mints.finish(key, 0, Ok(badge.clone()));
        assert!(Arc::ptr_eq(&again.wait().unwrap(), &badge));
        assert!(token.is_cancelled());
        // With the slot free, neither flight left in the queue starts.
        let mut state = mints.0.state.lock().unwrap();
        state.running -= 1;
        mints.start(&mut state);
        drop(state);
        assert_eq!(mints.load(), (0, 0));
    }

    #[test]
    fn errors_reach_everyone() {
        let mints = Mints::new(2, 8);
        let a = mints.request(b"not svg".to_vec(), MintOptions::default());
        let b = mints.request(b"not svg".to_vec(), MintOptions::default());
        assert!(matches!(a.wait(), Err(Error::InvalidSvg(_))));
        assert!(matches!(b.wait(), Err(Error::InvalidSvg(_))));
    }
}
