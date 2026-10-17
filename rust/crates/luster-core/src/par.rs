//! The engine's parallel passes, written once for every target.
//!
//! Where there are threads they are rayon's. WebAssembly here has one thread,
//! so the same calls run in order on the caller and rayon is not compiled in:
//! a pass gives the same result in either, since none depends on how its work
//! is shared out.

#[cfg(not(target_family = "wasm"))]
pub use rayon::{join, prelude, scope};

#[cfg(target_family = "wasm")]
pub use serial::{join, prelude, scope};

#[cfg(target_family = "wasm")]
mod serial {
    pub fn join<A, B, RA, RB>(a: A, b: B) -> (RA, RB)
    where
        A: FnOnce() -> RA,
        B: FnOnce() -> RB,
    {
        (a(), b())
    }

    /// What `scope` hands its body: each job spawned on it runs there and then.
    pub struct Scope(());

    impl Scope {
        pub fn spawn<F: FnOnce(&Scope)>(&self, job: F) {
            job(self)
        }
    }

    pub fn scope<F: FnOnce(&Scope) -> R, R>(body: F) -> R {
        body(&Scope(()))
    }

    /// rayon's names for the iterators the passes use, on std's.
    pub mod prelude {
        use std::iter::FlatMap;
        use std::slice::{Chunks, ChunksExact, ChunksMut, Iter, IterMut};

        pub trait ParallelSlice<T> {
            fn par_iter(&self) -> Iter<'_, T>;
            fn par_chunks(&self, size: usize) -> Chunks<'_, T>;
            fn par_chunks_exact(&self, size: usize) -> ChunksExact<'_, T>;
        }

        impl<T> ParallelSlice<T> for [T] {
            fn par_iter(&self) -> Iter<'_, T> {
                self.iter()
            }
            fn par_chunks(&self, size: usize) -> Chunks<'_, T> {
                self.chunks(size)
            }
            fn par_chunks_exact(&self, size: usize) -> ChunksExact<'_, T> {
                self.chunks_exact(size)
            }
        }

        pub trait ParallelSliceMut<T> {
            fn par_iter_mut(&mut self) -> IterMut<'_, T>;
            fn par_chunks_mut(&mut self, size: usize) -> ChunksMut<'_, T>;
        }

        impl<T> ParallelSliceMut<T> for [T] {
            fn par_iter_mut(&mut self) -> IterMut<'_, T> {
                self.iter_mut()
            }
            fn par_chunks_mut(&mut self, size: usize) -> ChunksMut<'_, T> {
                self.chunks_mut(size)
            }
        }

        pub trait IntoParallelIterator: IntoIterator + Sized {
            fn into_par_iter(self) -> Self::IntoIter {
                self.into_iter()
            }
        }

        impl<I: IntoIterator> IntoParallelIterator for I {}

        pub trait ParallelIterator: Iterator + Sized {
            fn flat_map_iter<U: IntoIterator, F: FnMut(Self::Item) -> U>(self, f: F) -> FlatMap<Self, U, F> {
                self.flat_map(f)
            }

            /// One scratch value for the whole pass, where rayon makes one per
            /// share of it.
            fn for_each_init<T, I: FnOnce() -> T, F: FnMut(&mut T, Self::Item)>(self, init: I, mut f: F) {
                let mut scratch = init();
                self.for_each(|item| f(&mut scratch, item));
            }
        }

        impl<I: Iterator> ParallelIterator for I {}
    }
}
