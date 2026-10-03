//! The connected pieces of a mask.

use rayon::prelude::*;

/// The connected pieces of a mask, largest first, each as the indices of its
/// pixels in scan order. Specks under `minimum` pixels are left out; pieces of
/// one size keep the order of their first pixels.
#[cfg(test)]
pub fn components(mask: &[bool], width: usize, height: usize, minimum: usize) -> Vec<Vec<usize>> {
    let pieces = Pieces::of(mask, width, height);
    let mut found: Vec<Vec<usize>> = pieces.sizes.iter().map(|&size| Vec::with_capacity(size)).collect();
    for (index, &piece) in pieces.label.iter().enumerate() {
        if piece >= 0 {
            found[piece as usize].push(index);
        }
    }
    found.retain(|piece| piece.len() >= minimum);
    found.sort_by(|a, b| b.len().cmp(&a.len()));
    found
}

/// The 4-connected pieces of a mask, found a run of pixels at a time rather
/// than a pixel at a time.
pub(super) struct Pieces {
    /// Each pixel's piece, or -1 off the mask. Pieces are numbered in the
    /// order of their first pixels.
    pub(super) label: Vec<i32>,
    /// Pixels in each piece.
    pub(super) sizes: Vec<usize>,
}

impl Pieces {
    pub(super) fn of(mask: &[bool], width: usize, height: usize) -> Pieces {
        // The runs on each row, as [start, end) columns.
        let runs: Vec<Vec<(usize, usize)>> = mask
            .par_chunks(width)
            .map(|row| {
                let mut runs = Vec::new();
                let mut x = 0;
                while x < width {
                    if !row[x] {
                        x += 1;
                        continue;
                    }
                    let start = x;
                    while x < width && row[x] {
                        x += 1;
                    }
                    runs.push((start, x));
                }
                runs
            })
            .collect();

        // Runs that touch a run on the row above are joined with it.
        let mut first = Vec::with_capacity(height + 1);
        first.push(0usize);
        for row in &runs {
            first.push(first.last().unwrap_or(&0) + row.len());
        }
        let mut parent: Vec<usize> = (0..first[height]).collect();
        fn root(parent: &mut [usize], mut run: usize) -> usize {
            while parent[run] != run {
                parent[run] = parent[parent[run]];
                run = parent[run];
            }
            run
        }
        for y in 1..height {
            let (above, here) = (&runs[y - 1], &runs[y]);
            let mut i = 0;
            for (j, &(start, end)) in here.iter().enumerate() {
                while i < above.len() && above[i].1 <= start {
                    i += 1;
                }
                let mut k = i;
                while k < above.len() && above[k].0 < end {
                    let (a, b) = (root(&mut parent, first[y - 1] + k), root(&mut parent, first[y] + j));
                    parent[a.max(b)] = a.min(b);
                    k += 1;
                }
            }
        }

        // Pieces numbered by their first run, which holds their first pixel.
        let mut number = vec![-1i32; parent.len()];
        let mut sizes: Vec<usize> = Vec::new();
        let mut piece_of = vec![0i32; parent.len()];
        for (run, piece) in piece_of.iter_mut().enumerate() {
            let top = root(&mut parent, run);
            if number[top] < 0 {
                number[top] = sizes.len() as i32;
                sizes.push(0);
            }
            *piece = number[top];
        }
        let mut label = vec![-1i32; width * height];
        for (y, row) in runs.iter().enumerate() {
            for (j, &(start, end)) in row.iter().enumerate() {
                let piece = piece_of[first[y] + j];
                sizes[piece as usize] += end - start;
            }
        }
        label.par_chunks_mut(width).enumerate().for_each(|(y, line)| {
            for (j, &(start, end)) in runs[y].iter().enumerate() {
                line[start..end].fill(piece_of[first[y] + j]);
            }
        });
        Pieces { label, sizes }
    }
}
