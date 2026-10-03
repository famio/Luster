//! Joining and rounding a plate: bridges, fillets and stopped-up slots.

use rayon::prelude::*;

use super::distance::spread_within;
use super::label::Pieces;

/// The plate closed by `radius`, as coverage: grown by it and shrunk back,
/// which fills every gap narrower than twice that with edges that are arcs of
/// the radius.
fn closing(plate: &[bool], width: usize, height: usize, radius: f32) -> Vec<f32> {
    // Nothing is read of either distance more than a pixel past the radius.
    let reach = radius + 2.0;
    let to_plate = spread_within(plate, width, height, reach);
    let grown: Vec<bool> = to_plate.par_iter().map(|&d| d <= radius + 0.5).collect();
    let outside: Vec<bool> = grown.par_iter().map(|&g| !g).collect();
    let to_outside = spread_within(&outside, width, height, reach);
    to_outside.par_iter().map(|&d| (d - radius).clamp(0.0, 1.0)).collect()
}

/// Joins the separate pieces of a plate, given as coverage, with the metal
/// between them.
///
/// The plate is closed by `radius`, and of what the closing adds only the
/// parts touching two different pieces are kept: a fill ends in fillets, never
/// in spikes or lobes, and a piece's own notches, which touch only that piece,
/// are left alone.
pub fn bridge(cover: &mut [f32], width: usize, height: usize, radius: f32) {
    let count = width * height;
    let plate: Vec<bool> = cover.par_iter().map(|&v| v >= 0.5).collect();
    // Specks are not pieces to be joined.
    let pieces = Pieces::of(&plate, width, height);
    let whole: Vec<bool> = pieces.sizes.iter().map(|&size| size >= 16).collect();
    if whole.iter().filter(|&&w| w).count() < 2 {
        return;
    }
    let label: Vec<i32> = pieces
        .label
        .par_iter()
        .map(|&piece| if piece >= 0 && whole[piece as usize] { piece } else { -1 })
        .collect();
    drop(pieces);

    let closed = closing(&plate, width, height, radius);

    let added: Vec<bool> = (0..count).into_par_iter().map(|i| closed[i] >= 0.5 && !plate[i]).collect();
    let fills = Pieces::of(&added, width, height);
    let mut touched: Vec<i32> = vec![-1; fills.sizes.len()];
    let mut joins = vec![false; fills.sizes.len()];
    for (index, &fill) in fills.label.iter().enumerate() {
        if fill < 0 {
            continue;
        }
        let fill = fill as usize;
        let (x, y) = (index % width, index / width);
        let mut note = |other: usize| {
            let piece = label[other];
            if piece >= 0 {
                if touched[fill] < 0 {
                    touched[fill] = piece;
                } else if touched[fill] != piece {
                    joins[fill] = true;
                }
            }
        };
        if x > 0 {
            note(index - 1);
        }
        if x + 1 < width {
            note(index + 1);
        }
        if y > 0 {
            note(index - width);
        }
        if y + 1 < height {
            note(index + width);
        }
    }
    let kept: Vec<bool> =
        fills.label.par_iter().map(|&fill| fill >= 0 && joins[fill as usize]).collect();
    // The fill's edge, a pixel out, carries the closing's partial coverage.
    cover.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        for (x, value) in row.iter_mut().enumerate() {
            let index = y * width + x;
            if plate[index] || closed[index] <= *value {
                continue;
            }
            let near = kept[index]
                || (x > 0 && kept[index - 1])
                || (x + 1 < width && kept[index + 1])
                || (y > 0 && kept[index - width])
                || (y + 1 < height && kept[index + width]);
            if near {
                *value = closed[index];
            }
        }
    });
}

/// Rounds the inside corners of a coverage with fillets of `radius`: the
/// closing of it, where what the closing adds is a corner's worth. A corner's
/// fill meets the open side along one edge, where a fill across a gap between
/// two parts meets it along two, and is left open; so is one larger than
/// `largest` pixels.
pub fn fillet(cover: &mut [f32], width: usize, height: usize, radius: f32, largest: usize) {
    let count = width * height;
    let plate: Vec<bool> = cover.par_iter().map(|&v| v >= 0.5).collect();
    let closed = closing(&plate, width, height, radius);

    let added: Vec<bool> = (0..count).into_par_iter().map(|i| closed[i] >= 0.5 && !plate[i]).collect();
    let fills = Pieces::of(&added, width, height);
    // Each fill's edge on the open side, and how many pieces it comes in.
    let open = |i: usize| !plate[i] && !added[i];
    let rim: Vec<bool> = (0..count)
        .into_par_iter()
        .map(|i| {
            if !added[i] {
                return false;
            }
            let (x, y) = ((i % width) as i64, (i / width) as i64);
            (-1i64..=1).any(|dy| {
                (-1i64..=1).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx < 0 || ny < 0 || nx >= width as i64 || ny >= height as i64
                        || open(ny as usize * width + nx as usize)
                })
            })
        })
        .collect();
    let rims = Pieces::of(&rim, width, height);
    let mut edges = vec![0usize; fills.sizes.len()];
    let mut counted = vec![false; rims.sizes.len()];
    for (index, &piece) in rims.label.iter().enumerate() {
        if piece >= 0 && !counted[piece as usize] {
            counted[piece as usize] = true;
            edges[fills.label[index] as usize] += 1;
        }
    }
    let corner: Vec<bool> =
        fills.sizes.iter().zip(&edges).map(|(&size, &edges)| size <= largest && edges == 1).collect();
    let kept: Vec<bool> = fills.label.par_iter().map(|&fill| fill >= 0 && corner[fill as usize]).collect();
    // The fill's edge, a pixel out, carries the closing's partial coverage.
    cover.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        for (x, value) in row.iter_mut().enumerate() {
            let index = y * width + x;
            if plate[index] || closed[index] <= *value {
                continue;
            }
            let near = kept[index]
                || (x > 0 && kept[index - 1])
                || (x + 1 < width && kept[index + 1])
                || (y > 0 && kept[index - width])
                || (y + 1 < height && kept[index + width]);
            if near {
                *value = closed[index];
            }
        }
    });
}

/// Fills the enclosed openings of a mask that no disc of `radius` fits inside.
/// The mask's outer outline is unchanged.
pub fn stop_up(plate: &mut [bool], width: usize, height: usize, radius: f32) {
    // The openings are the pieces of what is not metal; those that reach the
    // raster's border are the outside.
    let open: Vec<bool> = plate.par_iter().map(|&p| !p).collect();
    let pieces = Pieces::of(&open, width, height);
    let mut outside = vec![false; pieces.sizes.len()];
    let mut mark = |index: usize| {
        if pieces.label[index] >= 0 {
            outside[pieces.label[index] as usize] = true;
        }
    };
    for x in 0..width {
        mark(x);
        mark((height - 1) * width + x);
    }
    for y in 0..height {
        mark(y * width);
        mark(y * width + width - 1);
    }

    // Whatever was not reached is enclosed. Usually nothing is, so the distance
    // transform below is computed only when there are holes.
    if outside.iter().all(|&o| o) {
        return;
    }
    // Each pixel's distance from the metal; its largest value over an opening
    // is the radius of the largest disc that fits in it.
    let room = spread_within(plate, width, height, radius + 1.0);
    let mut fits = outside;
    for (index, &piece) in pieces.label.iter().enumerate() {
        if piece >= 0 && room[index] >= radius {
            fits[piece as usize] = true;
        }
    }
    plate.par_iter_mut().zip(pieces.label.par_iter()).for_each(|(filled, &piece)| {
        if piece >= 0 && !fits[piece as usize] {
            *filled = true;
        }
    });
}
