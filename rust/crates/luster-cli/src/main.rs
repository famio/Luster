//! Command line access to the engine.
//!
//!   luster mint <svg>…            what each file builds, and how long it took
//!   luster export <glb> <svg>     writes the badge as a GLB
//!                                 [--metal-lines] [--remove-hidden] [scale]
//!   luster artwork <svg>…         what the reader makes of each file, as JSON
//!   luster parity <golden> <svg>… the same, what it strikes and the GLBs it
//!                                 writes, checked against the dumps in that
//!                                 directory
//!   luster golden <golden> <svg>… writes those dumps for what this engine
//!                                 makes now

use std::path::Path;
use std::time::Instant;

use luster_core::inspect::{self as design, Contours};
use luster_core::{CancelToken, kurbo};
use serde_json::{Value, json};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let failures = match args.first().map(String::as_str) {
        Some("mint") => mint(&args[1..]),
        Some("export") if args.len() > 2 => export(&args[1..]),
        Some("artwork") => artwork(&args[1..]),
        Some("parity") if args.len() > 2 => parity(&args[1], &args[2..]),
        Some("golden") if args.len() > 2 => golden(&args[1], &args[2..]),
        _ => {
            println!(
                "luster {}\nusage: luster mint <svg>…\n       luster export <glb> <svg> [--metal-lines] [--remove-hidden] [scale]\n       luster artwork <svg>…\n       luster parity <golden dir> <svg>…\n       luster golden <golden dir> <svg>…",
                luster_core::version()
            );
            0
        }
    };
    if failures > 0 {
        std::process::exit(1);
    }
}

fn mint(paths: &[String]) -> usize {
    let mut failures = 0;
    for path in paths {
        let data = read(path);
        let start = Instant::now();
        let result = luster_core::mint(&data, &CancelToken::new());
        let elapsed = start.elapsed();
        match result {
            Ok(badge) => {
                let triangles: u32 = badge.submeshes.iter().map(|s| s.index_count / 3).sum();
                println!(
                    "{path}: {} submeshes, {} materials, {triangles} triangles, {:.1} ms",
                    badge.submeshes.len(),
                    badge.materials.len(),
                    elapsed.as_secs_f64() * 1e3
                );
            }
            Err(e) => {
                println!("{path}: error: {e}");
                failures += 1;
            }
        }
    }
    failures
}

/// Writes one SVG out as a GLB. `scale` sizes it; at 1 the badge is one unit
/// (a metre, in glTF) across.
fn export(args: &[String]) -> usize {
    let flags: Vec<&String> = args.iter().filter(|a| a.starts_with("--")).collect();
    let rest: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let (out, path) = (rest[0], rest[1]);
    let scale = rest.get(2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(1.0);
    let options = luster_core::MintOptions {
        metal_lines: flags.iter().any(|f| *f == "--metal-lines"),
        without_hidden_faces: flags.iter().any(|f| *f == "--remove-hidden"),
    };
    match luster_core::mint_with(&read(path), &options, &CancelToken::new()) {
        Ok(badge) => {
            let glb = luster_core::export::glb(&badge, scale, luster_core::badge::GOLD);
            match std::fs::write(out, &glb) {
                Ok(()) => {
                    let triangles: u32 = badge.submeshes.iter().map(|s| s.index_count / 3).sum();
                    println!(
                        "{out}: {} submeshes, {triangles} triangles, {} KB",
                        badge.submeshes.len(),
                        glb.len() / 1024
                    );
                    0
                }
                Err(e) => {
                    eprintln!("{out}: {e}");
                    1
                }
            }
        }
        Err(e) => {
            eprintln!("{path}: {e}");
            1
        }
    }
}

fn artwork(paths: &[String]) -> usize {
    let mut failures = 0;
    for path in paths {
        match dump(&read(path)) {
            Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default()),
            Err(e) => {
                eprintln!("{path}: {e}");
                failures += 1;
            }
        }
    }
    failures
}

/// Writes what this engine makes of each file, as the dump `parity` reads.
fn golden(dir: &str, paths: &[String]) -> usize {
    let mut failures = 0;
    if let Err(e) = std::fs::create_dir_all(dir) {
        println!("{dir}: {e}");
        return 1;
    }
    for path in paths {
        let name = Path::new(path).file_stem().unwrap_or_default().to_string_lossy().to_string();
        let built = match dump(&read(path)) {
            Ok(value) => value,
            Err(e) => {
                println!("{name}: {e}");
                failures += 1;
                continue;
            }
        };
        let out = Path::new(dir).join(format!("{name}.json"));
        match serde_json::to_string_pretty(&built)
            .map_err(|e| e.to_string())
            .and_then(|text| std::fs::write(&out, text + "\n").map_err(|e| e.to_string()))
        {
            Ok(()) => println!("{name}: wrote {}", out.display()),
            Err(e) => {
                println!("{name}: {e}");
                failures += 1;
            }
        }
    }
    failures
}

/// Every place two dumps differ, as a path through them.
fn differences(wanted: &Value, built: &Value, at: &str, notes: &mut Vec<String>) {
    match (wanted, built) {
        (Value::Object(a), Value::Object(b)) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let null = Value::Null;
                differences(
                    a.get(key).unwrap_or(&null),
                    b.get(key).unwrap_or(&null),
                    &format!("{at}.{key}"),
                    notes,
                );
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                notes.push(format!("{at}: {} wanted, {} built", a.len(), b.len()));
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                differences(x, y, &format!("{at}[{i}]"), notes);
            }
        }
        (a, b) if a != b => notes.push(format!("{at}: {a} wanted, {b} built")),
        _ => {}
    }
}

fn parity(golden: &str, paths: &[String]) -> usize {
    let mut failures = 0;
    for path in paths {
        let name = Path::new(path).file_stem().unwrap_or_default().to_string_lossy().to_string();
        let expected = match std::fs::read(Path::new(golden).join(format!("{name}.json"))) {
            Ok(bytes) => serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null),
            Err(e) => {
                println!("{name}: no golden ({e})");
                failures += 1;
                continue;
            }
        };
        let built = match dump(&read(path)) {
            Ok(value) => value,
            Err(e) => {
                println!("{name}: {e}");
                failures += 1;
                continue;
            }
        };
        // Anything at all that moved is a difference, and worth looking at.
        let mut notes = Vec::new();
        differences(&expected, &built, "", &mut notes);
        notes.truncate(40);
        if notes.is_empty() {
            println!("{name}: artwork and badges match");
        } else {
            failures += 1;
            println!("{name}:");
            for note in notes {
                println!("  {note}");
            }
        }
    }
    failures
}

/// What the reader makes of a document, and the pieces the engine strikes of
/// it with its lines plated and left in the metal.
fn dump(data: &[u8]) -> Result<Value, luster_core::Error> {
    let cancel = CancelToken::new();
    let document = luster_core::inspect::read(data, &cancel)?;
    let placement = design::placing(document.frame);
    let elements: Vec<Value> = document
        .elements
        .iter()
        // A region whose paint could not be resolved (a pattern, say) still
        // covers what is under it, but it paints nothing, so it is left out.
        .filter(|element| element.color.is_some())
        .map(|element| {
            let placed: Contours = element
                .rings
                .iter()
                .map(|ring| ring.iter().map(|p| placement * *p).collect())
                .collect();
            json!({
                "area": round(design::area(&placed)),
                "bounds": bounds(&placed),
                "isStroke": element.is_stroke,
                "color": element.color.map_or("none".to_string(), hex),
                "alpha": element.color.map_or(1.0, |c| round(f64::from(c.a))),
            })
        })
        .collect();
    let frame = document.frame;
    let read = luster_core::inspect::cells(&document.elements, frame);
    let reading = luster_core::inspect::sampling(read, &document.elements, frame, &cancel)?;
    let reading: Vec<Value> = reading
        .cells
        .iter()
        .map(|cell| {
            let placed: Contours = cell
                .rings
                .iter()
                .map(|ring| ring.iter().map(|p| placement * *p).collect())
                .collect();
            json!({
                "color": hex(cell.color),
                "area": round(design::area(&placed)),
                "bounds": bounds(&placed),
                "isLine": cell.is_line,
            })
        })
        .collect();
    let art = design::artwork(data, &cancel)?;
    let struck = |metal_lines| -> Result<Vec<Value>, luster_core::Error> {
        let options = luster_core::MintOptions { metal_lines, ..luster_core::MintOptions::default() };
        let struck = luster_core::inspect::strike(&art, &options, &cancel)?;
        Ok(struck
            .pieces
            .iter()
            .map(|piece| {
                json!({
                    "name": piece.name,
                    "area": round(design::area(&piece.rings).abs()),
                    "bounds": bounds(&piece.rings),
                    "front": round(f64::from(piece.slab.front)),
                })
            })
            .collect())
    };
    let pieces = json!({
        "plated": struck(false)?,
        "metal-lines": struck(true)?,
    });
    // The badge itself, byte for byte: the same document has to give the same
    // GLB on every platform the engine is built for.
    let glb = |metal_lines, without_hidden_faces| -> Result<String, luster_core::Error> {
        let options = luster_core::MintOptions { metal_lines, without_hidden_faces };
        let badge = luster_core::mint_with(data, &options, &cancel)?;
        Ok(fnv(&luster_core::export::glb(&badge, 1.0, luster_core::badge::GOLD)))
    };
    let badges = json!({
        "plated": glb(false, false)?,
        "metal-lines": glb(true, false)?,
        "without-hidden-faces": glb(false, true)?,
    });
    Ok(json!({
        "elements": elements,
        "reading": reading,
        "pieces": pieces,
        "cells": art.cells.iter().map(|cell| json!({
            "color": hex(cell.color),
            "area": round(design::area(&cell.rings)),
            "bounds": bounds(&cell.rings),
            "isLine": cell.is_line,
        })).collect::<Vec<_>>(),
        "silhouette": region(&art.silhouette),
        "detail": region(&art.detail),
        "outline": art.outline.as_ref().map(|path| {
            let rings = luster_core::inspect::contours(path, 1e-4);
            region(&rings)
        }),
        "hasPainting": art.painting.is_some(),
        "glb": badges,
    }))
}

/// FNV-1a over a file's bytes, as hex: enough to tell two builds apart.
fn fnv(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn region(rings: &Contours) -> Value {
    json!({ "area": round(design::area(rings)), "bounds": bounds(rings) })
}

/// Bounds over the rings big enough to be seen: a hundredth of a percent of
/// the badge. Booleans leave crescents of about that size where two shapes
/// share an edge, and which of them survive is no part of the record.
fn bounds(rings: &Contours) -> Vec<f64> {
    let mut box_: Option<kurbo::Rect> = None;
    let substantial: Contours = rings
        .iter()
        .filter(|ring| design::area(&vec![(*ring).clone()]).abs() > 1e-4)
        .cloned()
        .collect();
    for point in substantial.iter().flatten() {
        let next = kurbo::Rect::new(point.x, point.y, point.x, point.y);
        box_ = Some(box_.map_or(next, |b: kurbo::Rect| b.union(next)));
    }
    match box_ {
        Some(b) => vec![round(b.x0), round(b.y0), round(b.width()), round(b.height())],
        None => vec![0.0, 0.0, 0.0, 0.0],
    }
}

fn round(value: f64) -> f64 {
    (value * 1e6).round() / 1e6
}

fn hex(color: luster_core::Rgba) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(color.r), byte(color.g), byte(color.b))
}

fn read(path: &str) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"))
}
