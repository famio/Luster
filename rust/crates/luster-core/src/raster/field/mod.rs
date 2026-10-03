//! Measurements over a raster: distances, pieces, bridges and slots.

mod distance;
mod label;
mod morph;

pub use distance::{Edge, distance_to_edge};
#[cfg(test)]
pub use distance::spread;
#[cfg(test)]
pub use label::components;
pub use morph::{bridge, fillet, stop_up};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math;

    /// `spread` must be exact: the margin is cut from this field, so any error
    /// in it shows on the plate's outline.
    #[test]
    fn the_distance_field_is_exact() {
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = |bound: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % bound as u64) as usize
        };
        for (width, height) in [(37, 23), (23, 37), (64, 64), (5, 1), (1, 5)] {
            let mut mask = vec![false; width * height];
            let mut marks: Vec<(usize, usize)> = Vec::new();
            for _ in 0..=(width * height / 60).max(2) {
                let (x, y) = (next(width), next(height));
                mask[y * width + x] = true;
                marks.push((x, y));
            }
            let field = spread(&mask, width, height);
            for y in 0..height {
                for x in 0..width {
                    let nearest = marks
                        .iter()
                        .map(|&(mx, my)| {
                            let (dx, dy) = (mx as f64 - x as f64, my as f64 - y as f64);
                            (dx * dx + dy * dy).sqrt()
                        })
                        .fold(f64::MAX, f64::min);
                    let got = f64::from(field[y * width + x]);
                    assert!((got - nearest).abs() < 1e-4, "{width}x{height} at {x},{y}: {got} vs {nearest}");
                }
            }
        }
    }

    /// Masks with runs and blocks of marked pixels, not only scattered ones:
    /// inside a run, only its ends join a row's envelope. Short of its reach,
    /// `spread_within` is `spread` to the bit, and both are exact; past its
    /// reach it is at least that far. Every pixel's nearest marked pixel is
    /// one at the least distance.
    #[test]
    fn distances_are_exact_through_runs_and_short_of_a_reach() {
        let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = |bound: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % bound as u64) as usize
        };
        // Some are tall enough for several bands of rows, which pass on to
        // one another what lies above and below them; the last has a single
        // marked pixel, and columns with none.
        let sizes = [(41, 29), (29, 41), (64, 64), (7, 1), (1, 7), (23, 141), (61, 200), (9, 300)];
        for (case, (width, height)) in sizes.into_iter().enumerate() {
            let mut mask = vec![false; width * height];
            if case + 1 == sizes.len() {
                mask[height / 2 * width + width / 2] = true;
            } else {
                for _ in 0..=(width * height / 200).max(2) {
                    let (x, y) = (next(width), next(height));
                    let (w, h) = (1 + next(width / 3 + 1), 1 + next(height / 3 + 1));
                    for row in y..(y + h).min(height) {
                        for column in x..(x + w).min(width) {
                            mask[row * width + column] = true;
                        }
                    }
                }
            }
            let truth = |x: usize, y: usize| -> f64 {
                (0..width * height)
                    .filter(|&i| mask[i])
                    .map(|i| {
                        let (dx, dy) = ((i % width) as f64 - x as f64, (i / width) as f64 - y as f64);
                        (dx * dx + dy * dy).sqrt()
                    })
                    .fold(f64::MAX, f64::min)
            };
            let full = spread(&mask, width, height);
            let mut nearest = vec![0i32; width * height];
            let with = distance::spread_with_nearest(&mask, width, height, Some(&mut nearest));
            for y in 0..height {
                for x in 0..width {
                    let (index, exact) = (y * width + x, truth(x, y));
                    assert!((f64::from(full[index]) - exact).abs() < 1e-4, "{width}x{height} at {x},{y}");
                    assert_eq!(with[index].to_bits(), full[index].to_bits());
                    let found = nearest[index] as usize;
                    assert!(mask[found], "{width}x{height} at {x},{y}: nearest is not marked");
                    let (dx, dy) = ((found % width) as f64 - x as f64, (found / width) as f64 - y as f64);
                    assert!(((dx * dx + dy * dy).sqrt() - exact).abs() < 1e-4, "{width}x{height} at {x},{y}");
                }
            }
            for reach in [1.5f32, 4.0, 9.5] {
                let within = distance::spread_within(&mask, width, height, reach);
                for (index, (&near, &whole)) in within.iter().zip(&full).enumerate() {
                    if whole < reach {
                        assert_eq!(near.to_bits(), whole.to_bits(), "{width}x{height} at {index}, reach {reach}");
                    } else {
                        assert!(near >= reach - 1e-3, "{width}x{height} at {index}, reach {reach}: {near}");
                    }
                }
            }
        }
    }

    #[test]
    fn slots_too_narrow_to_cut_are_filled_in() {
        let side = 60;
        let mut plate = vec![true; side * side];
        let mut cut = |x: usize, y: usize, across: usize| {
            for row in y..y + across {
                for column in x..x + across {
                    plate[row * side + column] = false;
                }
            }
        };
        cut(6, 6, 3); // a slot: no disc of radius 5 fits in it
        cut(20, 20, 21); // an opening: one does
        cut(0, 50, 6); // a bay open to the outside: not an opening
        stop_up(&mut plate, side, side, 5.0);
        assert!(plate[7 * side + 7], "the slot was left in the gold");
        assert!(!plate[30 * side + 30], "the opening was filled in");
        assert!(!plate[52 * side + 2], "the outside was filled in");
    }

    #[test]
    fn distance_to_an_edge_is_signed_and_sub_pixel() {
        // A half-covered column: the edge runs through the middle of column 10.
        let (width, height) = (24, 3);
        let mut cover = vec![0.0f32; width * height];
        for y in 0..height {
            for x in 0..width {
                cover[y * width + x] = if x < 10 { 1.0 } else if x == 10 { 0.5 } else { 0.0 };
            }
        }
        let field = distance_to_edge(&cover, None, width, height, &[-64.0..64.0]);
        assert!(field[width + 5] < 0.0, "inside is negative");
        assert!(field[width + 15] > 0.0, "outside is positive");
        // Measured to the part-covered pixel the edge runs through (column 10,
        // five away) and corrected by its coverage, which puts the edge at its
        // center: not to the clear pixel beside it, which would put the edge on
        // their boundary half a pixel out.
        assert!((field[width + 15] - 5.0).abs() < 0.01, "{}", field[width + 15]);
    }

    /// A round edge measured from outside keeps its curve: the distance
    /// follows the circle within a small part of a pixel, where snapping the
    /// edge to pixel boundaries would leave a staircase half a pixel deep.
    #[test]
    fn a_round_edge_is_measured_without_a_staircase() {
        let (width, height) = (120, 120);
        let (cx, cy, r) = (60.3f32, 59.7f32, 40.37f32);
        // Coverage by supersampling, as an antialiased raster has it.
        let steps = 16;
        let cover: Vec<f32> = (0..width * height)
            .map(|i| {
                let (x, y) = ((i % width) as f32, (i / width) as f32);
                let mut hit = 0;
                for sy in 0..steps {
                    for sx in 0..steps {
                        let px = x - 0.5 + (sx as f32 + 0.5) / steps as f32;
                        let py = y - 0.5 + (sy as f32 + 0.5) / steps as f32;
                        if math::hypot(px - cx, py - cy) <= r {
                            hit += 1;
                        }
                    }
                }
                hit as f32 / (steps * steps) as f32
            })
            .collect();
        let field = distance_to_edge(&cover, None, width, height, &[-64.0..64.0]);
        let mut worst = 0.0f32;
        for (i, &value) in field.iter().enumerate() {
            let (x, y) = ((i % width) as f32, (i / width) as f32);
            let truth = math::hypot(x - cx, y - cy) - r;
            if (3.0..10.0).contains(&truth) {
                worst = worst.max((value - truth).abs());
            }
        }
        assert!(worst < 0.15, "off the circle by {worst} of a pixel");
    }

    #[test]
    fn components_are_found_largest_first_and_specks_dropped() {
        let (width, height) = (10, 10);
        let mut mask = vec![false; width * height];
        for y in 0..5 {
            for x in 0..5 {
                mask[y * width + x] = true; // 25 pixels
            }
        }
        for y in 7..9 {
            for x in 7..9 {
                mask[y * width + x] = true; // 4 pixels: a speck
            }
        }
        let pieces = components(&mask, width, height, 16);
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].len(), 25);
        assert_eq!(components(&mask, width, height, 1).len(), 2);
    }

    #[test]
    fn bridging_joins_two_pieces_but_not_one_piece_to_itself() {
        // Two blocks a gap apart, with room around them, and a notch cut into
        // the top of the left one. The room matters: with the blocks against
        // the raster's border, the closing reaches the border everywhere and
        // the notch joins the gap as one addition.
        let (width, height) = (64, 40);
        let mut cover = vec![0.0f32; width * height];
        let mut fill = |x0: usize, x1: usize, y0: usize, y1: usize, v: f32| {
            for y in y0..y1 {
                for x in x0..x1 {
                    cover[y * width + x] = v;
                }
            }
        };
        fill(8, 24, 8, 32, 1.0);
        fill(30, 46, 8, 32, 1.0);
        fill(12, 16, 8, 14, 0.0); // the notch: as narrow as the gap
        bridge(&mut cover, width, height, 4.0);
        assert!(cover[20 * width + 27] >= 0.5, "the gap between the pieces was bridged");
        assert!(cover[10 * width + 14] < 0.5, "the notch, which touches one piece, was left alone");
    }

    #[test]
    fn a_fillet_rounds_a_corner_but_leaves_a_slot_open() {
        // An L, whose inside corner is filled; a U, whose slot is too long to
        // be a corner; and two bars side by side, whose gap meets the open
        // side at both ends.
        let (width, height) = (128, 64);
        let mut cover = vec![0.0f32; width * height];
        let mut fill = |x0: usize, x1: usize, y0: usize, y1: usize| {
            for y in y0..y1 {
                for x in x0..x1 {
                    cover[y * width + x] = 1.0;
                }
            }
        };
        fill(8, 16, 8, 56);
        fill(8, 40, 48, 56);
        fill(56, 64, 8, 56);
        fill(70, 78, 8, 56);
        fill(56, 78, 48, 56);
        fill(92, 100, 8, 20);
        fill(106, 114, 8, 20);
        fillet(&mut cover, width, height, 4.0, 100);
        assert!(cover[47 * width + 16] >= 0.5, "the L's inside corner was rounded");
        assert!(cover[30 * width + 30] < 0.5, "the open side of the L was filled");
        assert!(cover[30 * width + 67] < 0.5, "the U's slot was filled");
        assert!(cover[14 * width + 103] < 0.5, "the gap between the bars was filled");
    }
}
