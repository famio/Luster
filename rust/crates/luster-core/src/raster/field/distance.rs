//! Distances to a mask's edge, exact and to a pixel's fraction.

use rayon::prelude::*;
use crate::math;

/// Exact Euclidean distance, in pixels, from every pixel to the nearest marked
/// one (Felzenszwalb & Huttenlocher: the lower envelope of a parabola per
/// entry, columns then rows).
///
/// Exactness matters: the plate's outline is a threshold of this field, and the
/// directional error of a chamfer approximation shows up as ripples in it.
#[cfg(test)]
pub fn spread(mask: &[bool], width: usize, height: usize) -> Vec<f32> {
    transform(mask, width, height, None, f32::INFINITY)
}

/// The same, exact only short of `reach`: a pixel at least that far from
/// every marked one is somewhere at least that far. For a caller that asks
/// nothing further out, it is quicker: a row's envelope takes only the
/// parabolas that come under `reach` somewhere, which lie near the mask's
/// edges.
pub fn spread_within(mask: &[bool], width: usize, height: usize, reach: f32) -> Vec<f32> {
    transform(mask, width, height, None, reach * reach)
}

/// The same as `spread`, and with `nearest` given, each pixel's nearest
/// marked pixel.
pub fn spread_with_nearest(
    mask: &[bool],
    width: usize,
    height: usize,
    nearest: Option<&mut [i32]>,
) -> Vec<f32> {
    transform(mask, width, height, nearest, f32::INFINITY)
}

/// `spread` and its variants: exact short of the square root of `beyond`.
fn transform(
    mask: &[bool],
    width: usize,
    height: usize,
    nearest: Option<&mut [i32]>,
    beyond: f32,
) -> Vec<f32> {
    // Squared distances throughout; the root is taken at the end.
    let far = (width * width + height * height) as f32 * 4.0;
    let span = width.max(height);

    // Columns. The mask is all marked or clear, so down a column the nearest
    // marked pixel is simply the nearest one above or below: a sweep down and
    // one up find it without the envelope or turning the field into columns.
    // A tie goes to the one above, as the envelope has it. The rows are swept
    // a band at a time, side by side, each band written where it lies: first
    // the first and last marked row of every column in each band, which tell
    // each band what lies above and below it.
    let bands = height.div_ceil(BAND);
    let ends: Vec<(Vec<i32>, Vec<i32>)> = (0..bands)
        .into_par_iter()
        .map(|band| {
            let (mut first, mut last) = (vec![NONE; width], vec![NONE; width]);
            for y in band * BAND..((band + 1) * BAND).min(height) {
                for (x, &marked) in mask[y * width..(y + 1) * width].iter().enumerate() {
                    if marked {
                        if first[x] == NONE {
                            first[x] = y as i32;
                        }
                        last[x] = y as i32;
                    }
                }
            }
            (first, last)
        })
        .collect();
    let mut from_above: Vec<Vec<i32>> = Vec::with_capacity(bands);
    let mut carry = vec![NONE; width];
    for (_, last) in &ends {
        from_above.push(carry.clone());
        for (carried, &row) in carry.iter_mut().zip(last) {
            if row != NONE {
                *carried = row;
            }
        }
    }
    let mut from_below: Vec<Vec<i32>> = vec![Vec::new(); bands];
    let mut carry = vec![NONE; width];
    for (band, (first, _)) in ends.iter().enumerate().rev() {
        from_below[band] = carry.clone();
        for (carried, &row) in carry.iter_mut().zip(first) {
            if row != NONE {
                *carried = row;
            }
        }
    }
    // A column with nothing marked is left to the envelope: the row pass
    // reads its values, and they are the envelope's own, the same for every
    // such column.
    let unmarked: Option<(Vec<f32>, Vec<i32>)> = carry.contains(&NONE).then(|| {
        let mut scratch = Scratch::new(span);
        let mut line = vec![far; height];
        envelope(&mut line, &mut scratch, far, beyond);
        (line, scratch.winner[..height].to_vec())
    });
    // Each band is swept down, noting the nearest marked row above every
    // pixel, then up, settling on it or the one below.
    let sweep = |band: usize, squared: &mut [f32], rows: &mut [i32]| {
        let y0 = band * BAND;
        let tall = squared.len() / width;
        let mut above = from_above[band].clone();
        for dy in 0..tall {
            let marks = &mask[(y0 + dy) * width..(y0 + dy + 1) * width];
            for (x, &marked) in marks.iter().enumerate() {
                if marked {
                    above[x] = (y0 + dy) as i32;
                }
            }
            rows[dy * width..(dy + 1) * width].copy_from_slice(&above);
        }
        let mut below = from_below[band].clone();
        for dy in (0..tall).rev() {
            let y = y0 + dy;
            let here = y as i32;
            let marks = &mask[y * width..(y + 1) * width];
            for (x, &marked) in marks.iter().enumerate() {
                if marked {
                    below[x] = here;
                }
                let at = dy * width + x;
                let (up, down) = (rows[at], below[x]);
                let row = if up == NONE || (down != NONE && down - here < here - up) { down } else { up };
                if row != NONE {
                    let gap = here as f32 - row as f32;
                    squared[at] = gap * gap;
                    rows[at] = row;
                } else if let Some((line, winner)) = &unmarked {
                    squared[at] = line[y];
                    rows[at] = winner[y];
                }
            }
        }
    };
    let mut squared: Vec<f32> = vec![0.0; width * height];
    // The row each pixel's nearest lies in is kept only when the nearest is
    // asked for; otherwise each band notes it where it is swept.
    let mut rows: Vec<i32> = Vec::new();
    if nearest.is_some() {
        rows = vec![NONE; width * height];
        squared
            .par_chunks_mut(BAND * width)
            .zip(rows.par_chunks_mut(BAND * width))
            .enumerate()
            .for_each(|(band, (squared, rows))| sweep(band, squared, rows));
    } else {
        squared.par_chunks_mut(BAND * width).enumerate().for_each_init(Vec::new, |rows, (band, squared)| {
            rows.resize(squared.len(), NONE);
            sweep(band, squared, rows);
        });
    }
    drop((from_above, from_below));

    // Rows.
    match nearest {
        Some(nearest) => {
            squared
                .par_chunks_mut(width)
                .zip(nearest.par_chunks_mut(width))
                .enumerate()
                .for_each_init(|| Scratch::new(span), |scratch, (y, (row, nearest))| {
                    envelope(row, scratch, far, beyond);
                    for value in row.iter_mut() {
                        *value = value.sqrt();
                    }
                    let from = &rows[y * width..(y + 1) * width];
                    for (x, found) in nearest.iter_mut().enumerate() {
                        let column = scratch.winner[x] as usize;
                        *found = from[column] * width as i32 + column as i32;
                    }
                });
        }
        None => {
            squared.par_chunks_mut(width).for_each_init(|| Scratch::new(span), |scratch, row| {
                envelope(row, scratch, far, beyond);
                for value in row.iter_mut() {
                    *value = value.sqrt();
                }
            });
        }
    }
    squared
}

/// Pixels of a row looked over at once for an edge.
const RUN: usize = 64;
/// Rows swept together in the column pass: enough bands for every thread,
/// and few enough that what each tells the others is little.
const BAND: usize = 64;
/// No marked pixel found yet.
const NONE: i32 = i32::MIN;

struct Scratch {
    /// The parabolas on the envelope, by where they are centred.
    centre: Vec<usize>,
    /// Each one's value at its centre, and that value plus its centre squared.
    base: Vec<f32>,
    lifted: Vec<f32>,
    border: Vec<f32>,
    winner: Vec<i32>,
}

impl Scratch {
    fn new(span: usize) -> Scratch {
        Scratch {
            centre: vec![0; span],
            base: vec![0.0; span],
            lifted: vec![0.0; span],
            border: vec![0.0; span + 1],
            winner: vec![0; span],
        }
    }
}

/// One line of the transform: the lower envelope of a parabola per entry,
/// sampled at every position, with the winning parabola's index alongside.
///
/// A marked entry is 0, and its own nearest. Inside a run of them a parabola
/// can win nowhere else, since the run's ends are nearer to everything
/// outside it, so only the ends join the envelope. Nor does an entry at or
/// past `beyond`, whose parabola lies at or above it everywhere: nothing
/// past it is asked of the line.
fn envelope(line: &mut [f32], scratch: &mut Scratch, far: f32, beyond: f32) {
    let count = line.len();
    if count == 0 {
        return;
    }
    let centre = &mut scratch.centre[..count];
    let base = &mut scratch.base[..count];
    let lifted = &mut scratch.lifted[..count];
    let border = &mut scratch.border[..count + 1];
    let winner = &mut scratch.winner[..count];
    let mut last = 0usize;
    centre[0] = 0;
    base[0] = line[0];
    lifted[0] = line[0];
    border[0] = -far;
    border[1] = far;
    for (here, &value) in line.iter().enumerate().skip(1) {
        if value >= beyond || (value == 0.0 && line[here - 1] == 0.0 && line.get(here + 1) == Some(&0.0)) {
            continue;
        }
        let raised = value + (here * here) as f32;
        let mut crossing;
        loop {
            let there = centre[last];
            crossing = (raised - lifted[last]) / (2 * here as i64 - 2 * there as i64) as f32;
            if crossing > border[last] || last == 0 {
                break;
            }
            last -= 1;
        }
        last += 1;
        centre[last] = here;
        base[last] = value;
        lifted[last] = raised;
        border[last] = crossing;
        border[last + 1] = far;
    }
    // Each parabola's own values were kept, so the line is written over as
    // it is read.
    last = 0;
    for (here, (out, won)) in line.iter_mut().zip(winner.iter_mut()).enumerate() {
        while border[last + 1] < here as f32 {
            last += 1;
        }
        if *out == 0.0 {
            *won = here as i32;
            continue;
        }
        let there = centre[last];
        let gap = here as f32 - there as f32;
        *out = gap * gap + base[last];
        *won = there as i32;
    }
}

/// Signed distance, in pixels, from every pixel to the edge of a coverage
/// field: positive outside, negative inside, sub-pixel accurate.
///
/// Where the distance falls in one of the `cuts`, the signed distances a
/// caller thresholds at, it is measured to where the edge runs through the
/// nearest edge pixel, found from that pixel's coverage and the slant of the
/// coverage round it. Elsewhere it is measured to the nearest edge pixel's
/// center and corrected by its coverage alone, which is quicker and can be off
/// by a pixel. `among` restricts the edge to the flagged pixels.
pub fn distance_to_edge(
    cover: &[f32],
    among: Option<&[bool]>,
    width: usize,
    height: usize,
    cuts: &[std::ops::Range<f32>],
) -> Vec<f32> {
    Edge::of(cover, among, width, height).distance(cover, width, height, cuts)
}

/// A coverage's edge pixels and the distance transform to them: the costly
/// part of `distance_to_edge`, which cuts of one coverage share.
pub struct Edge {
    edge: Vec<bool>,
    nearest: Vec<i32>,
    field: Vec<f32>,
}

impl Edge {
    /// The edge of `cover`, restricted to the pixels `among` flags.
    pub fn of(cover: &[f32], among: Option<&[bool]>, width: usize, height: usize) -> Edge {
        let inside: Vec<bool> = cover.par_iter().map(|&v| v >= 0.5).collect();
        // Coverage strictly between clear and solid, allowing for 8-bit rounding.
        let partial = |v: f32| v > 0.5 / 255.0 && v < 1.0 - 0.5 / 255.0;
        let mut edge = vec![false; width * height];
        edge.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
            let at = y * width;
            let here = &inside[at..at + width];
            let above = (y > 0).then(|| &inside[at - width..at]);
            let below = (y + 1 < height).then(|| &inside[at + width..at + 2 * width]);
            for start in (0..width).step_by(RUN) {
                let end = (start + RUN).min(width);
                // Most of a row is a stretch on one side, with the same side
                // above, below and at either end of it: no edge runs through
                // it, and it is passed over whole.
                let side = here[start];
                let even = |row: &[bool]| row[start..end].iter().all(|&v| v == side);
                if even(here)
                    && above.is_none_or(even)
                    && below.is_none_or(even)
                    && (start == 0 || here[start - 1] == side)
                    && (end == width || here[end] == side)
                {
                    continue;
                }
                for (x, flag) in (start..end).zip(&mut row[start..end]) {
                    if !among.is_none_or(|kind| kind[at + x]) {
                        continue;
                    }
                    // An edge pixel has a neighbour on the other side of the 0.5
                    // threshold. Partial coverage alone does not qualify: a filled slot
                    // leaves part-covered pixels inside solid metal.
                    let side = here[x];
                    let left = x > 0 && here[x - 1] != side;
                    let right = x + 1 < width && here[x + 1] != side;
                    let up = above.is_some_and(|row| row[x] != side);
                    let down = below.is_some_and(|row| row[x] != side);
                    if !(left || right || up || down) {
                        continue;
                    }
                    // Of two pixels either side of the threshold, the edge runs through
                    // the part-covered one, and only its coverage says where. A clear
                    // or solid pixel beside it would put the edge on the boundary
                    // between them, and a round edge would come out as a staircase.
                    let index = at + x;
                    let beside_partial = (left && partial(cover[index - 1]))
                        || (right && partial(cover[index + 1]))
                        || (up && partial(cover[index - width]))
                        || (down && partial(cover[index + width]));
                    *flag = partial(cover[index]) || !beside_partial;
                }
            }
        });

        let mut nearest = vec![0i32; width * height];
        let field = spread_with_nearest(&edge, width, height, Some(&mut nearest));
        Edge { edge, nearest, field }
    }

    /// The signed distance to the edge of `cover`, the coverage the edge was
    /// found in, refined near `cuts` as `distance_to_edge` has it.
    pub fn distance(&self, cover: &[f32], width: usize, height: usize, cuts: &[std::ops::Range<f32>]) -> Vec<f32> {
        let (edge, nearest) = (&self.edge, &self.nearest);
        let mut field = self.field.clone();

        // Where in each edge pixel the edge runs: from its center, up the slope of
        // the coverage, by as much as the coverage says for an edge at that slant.
        // None where the coverage is flat all round and gives no slant.
        let at = |index: usize| -> Option<(f32, f32)> {
            let (x, y) = ((index % width) as isize, (index / width) as isize);
            let c = |dx: isize, dy: isize| -> f32 {
                let (cx, cy) = ((x + dx).clamp(0, width as isize - 1), (y + dy).clamp(0, height as isize - 1));
                cover[cy as usize * width + cx as usize]
            };
            let gx = (c(1, -1) + 2.0 * c(1, 0) + c(1, 1)) - (c(-1, -1) + 2.0 * c(-1, 0) + c(-1, 1));
            let gy = (c(-1, 1) + 2.0 * c(0, 1) + c(1, 1)) - (c(-1, -1) + 2.0 * c(0, -1) + c(1, -1));
            let length = math::hypot(gx, gy);
            if length < 1e-6 {
                return None;
            }
            let (ux, uy) = (gx / length, gy / length);
            let offset = edge_offset(ux, uy, cover[index]);
            Some((x as f32 + ux * offset, y as f32 + uy * offset))
        };
        // From a pixel to the edge running through edge pixel `found`.
        // Worked out where needed: only pixels near a cut ask, and few of them.
        let reach = |x: f32, y: f32, solid: bool, found: usize| -> f32 {
            if let Some((ex, ey)) = at(found) {
                return math::hypot(x - ex, y - ey);
            }
            // No slant to go by: measured to the pixel's center and moved by its
            // coverage along the way there.
            let (fx, fy) = ((found % width) as f32, (found / width) as f32);
            let beyond = cover[found] - 0.5;
            let centre = math::hypot(x - fx, y - fy);
            if solid { centre + beyond } else { centre - beyond }
        };
        let none = (width + height) as f32;
        // The cuts with their slack, and the span they all lie in: most pixels
        // lie well outside it, and are told so by two comparisons.
        let near: Vec<(f32, f32)> = cuts.iter().map(|cut| (cut.start - CUT_SLACK, cut.end + CUT_SLACK)).collect();
        let (low, high) =
            near.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), &(start, end)| (low.min(start), high.max(end)));
        field.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
            // Where the walk ended for the pixel before, in this row: usually a
            // step or two from where this one's ends.
            let mut last: Option<usize> = None;
            for (x, value) in row.iter_mut().enumerate() {
                if *value >= none {
                    last = None;
                    continue;
                }
                let index = y * width + x;
                let solid = cover[index] >= 0.5;
                let beyond = cover[nearest[index] as usize] - 0.5;
                let rough = if solid { -(*value + beyond) } else { *value - beyond };
                // Away from every cut the nearest center, moved by its coverage,
                // is close enough: nothing is thresholded there. The rough value
                // is within a pixel or so of the true one, hence the slack.
                let cut = rough > low && rough < high && near.iter().any(|&(start, end)| rough > start && rough < end);
                if !cut {
                    *value = rough;
                    last = None;
                    continue;
                }
                let (px, py) = (x as f32, y as f32);
                let mut found = nearest[index] as usize;
                let mut nearer = reach(px, py, solid, found);
                if let Some(before) = last.filter(|&before| before != found) {
                    let distance = reach(px, py, solid, before);
                    if distance < nearer {
                        (found, nearer) = (before, distance);
                    }
                }
                // The nearest edge pixel by its center is not always the one whose
                // edge is nearest: walk along the edge while a neighbouring edge
                // pixel's is nearer. Along a smooth edge the distance falls to one
                // least and rises again.
                for _ in 0..REFINE_STEPS {
                    let (fx, fy) = ((found % width) as isize, (found / width) as isize);
                    let mut step = None;
                    for (dx, dy) in AROUND {
                        let (nx, ny) = (fx + dx, fy + dy);
                        if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
                            continue;
                        }
                        let next = ny as usize * width + nx as usize;
                        // An edge runs within a pixel's half-diagonal of its
                        // center, so one whose center is that much further
                        // than the best so far cannot be nearer.
                        let centre = math::hypot(px - nx as f32, py - ny as f32);
                        if edge[next] && centre - HALF_DIAGONAL < nearer.abs() {
                            let distance = reach(px, py, solid, next);
                            if distance < nearer {
                                nearer = distance;
                                step = Some(next);
                            }
                        }
                    }
                    match step {
                        Some(next) => found = next,
                        None => break,
                    }
                }
                last = Some(found);
                *value = if solid { -nearer } else { nearer };
            }
        });
        field
    }
}

/// How far outside a cut, in pixels, distances are still refined: the rough
/// distance can be this far off the true one.
const CUT_SLACK: f32 = 2.0;
/// Most steps taken along the edge from the nearest pixel by its center.
const REFINE_STEPS: usize = 16;
/// Half a pixel's diagonal, and a little: the furthest an edge can run from
/// the center of a pixel it passes through.
const HALF_DIAGONAL: f32 = 0.75;
/// A pixel's eight neighbours.
const AROUND: [(isize, isize); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// How far from a pixel's center an edge runs through it, towards the covered
/// side, for coverage `a` and an edge facing along the unit (`ux`, `uy`): the
/// pixel's area cut off by a straight edge at that slant (Gustavson and
/// Strand, 2011). Negative when the edge runs on the uncovered side.
fn edge_offset(ux: f32, uy: f32, a: f32) -> f32 {
    let (mut gx, mut gy) = (ux.abs(), uy.abs());
    if gx < gy {
        std::mem::swap(&mut gx, &mut gy);
    }
    let a = a.clamp(0.0, 1.0);
    if gy < 1e-6 {
        return 0.5 - a;
    }
    let corner = 0.5 * gy / gx;
    if a < corner {
        0.5 * (gx + gy) - (2.0 * gx * gy * a).sqrt()
    } else if a < 1.0 - corner {
        (0.5 - a) * gx
    } else {
        -0.5 * (gx + gy) + (2.0 * gx * gy * (1.0 - a)).sqrt()
    }
}
