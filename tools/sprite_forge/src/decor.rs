//! Éléments de décor posés sur le sol (herbes, cailloux, flaques, ossements, tapis...): de petits motifs vus d'en haut, dessinés à
//! l'échelle des tuiles (48 px par tuile) et que le jeu mêle au sol de chaque niveau. Une case fait deux tuiles de côté.

use std::f32::consts::{PI, TAU};

use glam::{Mat3, Vec3};

use crate::render::Part;
use crate::shapes::*;

/// Nombre de sortes de décor, et de variantes de chacune. La planche range les variantes en lignes: case = variante * KINDS + sorte.
pub const KINDS: usize = 16;
pub const VARIANTS: usize = 4;
/// Côté d'une case, en pixels (deux tuiles).
pub const CELL: usize = 96;

/// Un petit disque aplati posé sur le sol.
fn patch(x: f32, z: f32, rx: f32, rz: f32, color: u32) -> Part {
    ellipsoid(Vec3::new(x, 0.0, z), Vec3::new(rx, 0.015, rz), color)
}

fn blade(rng: &mut Rng, at: Vec3, height: f32, colors: &[u32]) -> Part {
    let lean = Vec3::new(rng.spread(0.18), 0.0, rng.spread(0.18));
    let color = colors[(rng.next() * colors.len() as f32) as usize % colors.len()];
    cone(at, at + Vec3::new(0.0, height, 0.0) + lean, 0.035, 0.004, color)
}

fn tuft(rng: &mut Rng, center: Vec3, count: usize, height: f32, colors: &[u32]) -> Vec<Part> {
    (0..count)
        .map(|_| {
            let at = center + Vec3::new(rng.spread(0.22), 0.0, rng.spread(0.22));
            let h = height * (0.6 + 0.6 * rng.next());
            blade(rng, at, h, colors)
        })
        .collect()
}

pub fn build(kind: usize, variant: usize) -> Vec<Part> {
    let mut rng = Rng((kind * 977 + variant * 131 + 5) as u64);
    let v = variant as f32;
    let mut p: Vec<Part> = Vec::new();
    match kind {
        // Touffe d'herbe.
        0 => {
            p.push(patch(0.0, 0.0, 0.4, 0.34, 0x2a4a22));
            p.extend(tuft(&mut rng, Vec3::ZERO, 18 + variant * 3, 0.62, &[0x6aa84a, 0x8cc85a, 0x4f8a3a, 0xa4d870]));
        }
        // Fleurs des champs, sur un peu d'herbe.
        1 => {
            p.extend(tuft(&mut rng, Vec3::ZERO, 8, 0.3, &[0x4f8a3a, 0x6aa84a]));
            let petals = [0xf4f0e0, 0xf6d23a, 0xe86a9a, 0x6a8ae8][variant % 4];
            for _ in 0..(3 + variant % 3) {
                let base = Vec3::new(rng.spread(0.3), 0.0, rng.spread(0.3));
                let top = base + Vec3::new(rng.spread(0.05), 0.34 + 0.1 * rng.next(), rng.spread(0.05));
                p.push(limb(base, top, 0.012, 0x3f7a30));
                p.push(shiny(ellipsoid(top, Vec3::new(0.07, 0.035, 0.07), petals), 0.3));
                p.push(ball(top + Vec3::new(0.0, 0.03, 0.0), 0.025, 0xf0c030));
            }
        }
        // Fougère: des frondes en éventail.
        2 => {
            let n = 6 + variant;
            for k in 0..n {
                let a = k as f32 * TAU / n as f32 + v;
                let dir = Vec3::new(a.cos(), 0.0, a.sin());
                let mut prev = Vec3::new(0.0, 0.03, 0.0);
                for s in 1..=4 {
                    let t = s as f32 / 4.0;
                    let next = dir * (0.5 * t) + Vec3::new(0.0, 0.03 + 0.28 * (t * PI).sin(), 0.0);
                    p.push(limb(prev, next, 0.012, 0x3a6a2e));
                    p.push(ellipsoid((prev + next) / 2.0 + Vec3::new(0.0, 0.01, 0.0), Vec3::new(0.09, 0.012, 0.045), if s % 2 == 0 { 0x4f8a3a } else { 0x5a9a44 }));
                    prev = next;
                }
            }
        }
        // Cailloux.
        3 => {
            for _ in 0..(5 + variant) {
                let r = 0.04 + 0.07 * rng.next();
                let c = [0x8a8a92, 0x6e6e78, 0x9a968a, 0x5a5a64][(rng.next() * 4.0) as usize % 4];
                p.push(shiny(ellipsoid(Vec3::new(rng.spread(0.42), r * 0.5, rng.spread(0.36)), Vec3::new(r * 1.3, r * 0.6, r), c), 0.4));
            }
        }
        // Rocher.
        4 => {
            let size = 0.32 + 0.05 * v;
            p.push(ellipsoid(Vec3::new(0.02, 0.0, 0.03), Vec3::new(size * 1.2, 0.02, size), 0x1c1a20));
            p.push(shiny(ellipsoid(Vec3::new(0.0, size * 0.5, 0.0), Vec3::new(size * 1.1, size * 0.62, size * 0.9), 0x7a7a84), 0.45));
            p.push(shiny(ellipsoid(Vec3::new(size * 0.55, size * 0.3, size * 0.4), Vec3::new(size * 0.5, size * 0.34, size * 0.4), 0x8e8e98), 0.4));
            p.push(shiny(ellipsoid(Vec3::new(-size * 0.5, size * 0.22, -size * 0.45), Vec3::new(size * 0.4, size * 0.26, size * 0.32), 0x65656f), 0.4));
            // Mousse sur le dessus.
            p.push(ellipsoid(Vec3::new(-0.05, size * 0.95, 0.0), Vec3::new(size * 0.55, size * 0.12, size * 0.4), 0x5a8a40));
        }
        // Champignons.
        5 => {
            let (cap, dot) = if variant % 2 == 0 { (0xc0302a, 0xf4f0e0) } else { (0x9a6a3a, 0xc8a070) };
            for _ in 0..(2 + variant % 3) {
                let (x, z) = (rng.spread(0.3), rng.spread(0.25));
                let h = 0.14 + 0.12 * rng.next();
                let r = 0.09 + 0.07 * rng.next();
                p.push(cone(Vec3::new(x, 0.0, z), Vec3::new(x, h, z), 0.04, 0.03, 0xe8e0c8));
                p.push(shiny(ellipsoid(Vec3::new(x, h + 0.02, z), Vec3::new(r, r * 0.55, r), cap), 0.3));
                for k in 0..4 {
                    let a = k as f32 * 1.7;
                    p.push(ball(Vec3::new(x + a.cos() * r * 0.55, h + r * 0.45, z + a.sin() * r * 0.55), 0.016, dot));
                }
            }
        }
        // Racines qui sortent de terre.
        6 => {
            for k in 0..(3 + variant % 2) {
                let a = k as f32 * 1.9 + v;
                let dir = Vec3::new(a.cos(), 0.0, a.sin());
                let mut prev = Vec3::ZERO;
                for s in 1..=4 {
                    let t = s as f32 / 4.0;
                    let wobble = Vec3::new(rng.spread(0.08), 0.0, rng.spread(0.08));
                    let next = dir * (0.9 * t) + wobble + Vec3::new(0.0, 0.07 * (1.0 - t) + 0.02, 0.0);
                    p.push(limb(prev, next, 0.08 * (1.0 - t * 0.7), if s % 2 == 0 { 0x5a3a22 } else { 0x4a2e1a }));
                    prev = next;
                }
            }
            p.push(ellipsoid(Vec3::ZERO, Vec3::new(0.14, 0.08, 0.14), 0x4a2e1a));
        }
        // Feuilles mortes.
        7 => {
            let colors = [0xb8742a, 0xa0481f, 0xd0a030, 0x7a5a24, 0x8a3a1a];
            for _ in 0..(12 + variant * 2) {
                let c = colors[(rng.next() * 5.0) as usize % 5];
                let a = rng.next() * PI;
                let mut leaf = ellipsoid(Vec3::new(rng.spread(0.45), 0.012, rng.spread(0.4)), Vec3::new(0.075, 0.008, 0.04), c);
                leaf.rot = Mat3::from_rotation_y(a);
                p.push(leaf);
            }
        }
        // Flaque d'eau.
        8 => {
            let (rx, rz) = (0.5 + 0.1 * v, 0.34 + 0.06 * v);
            p.push(ellipsoid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(rx * 1.12, 0.01, rz * 1.15), 0x2c3a46));
            p.push(shiny(ellipsoid(Vec3::new(0.0, 0.012, 0.0), Vec3::new(rx, 0.012, rz), 0x2f5070), 0.2));
            p.push(glow(ellipsoid(Vec3::new(-rx * 0.3, 0.025, -rz * 0.3), Vec3::new(rx * 0.3, 0.004, rz * 0.14), 0x6a90b0)));
        }
        // Mousse.
        9 => {
            for _ in 0..(5 + variant) {
                p.push(patch(rng.spread(0.35), rng.spread(0.3), 0.14 + 0.12 * rng.next(), 0.1 + 0.1 * rng.next(), if rng.next() < 0.5 { 0x4a7a3a } else { 0x5e8f44 }));
            }
        }
        // Branche tombée.
        10 => {
            let a = v * 0.8;
            let dir = Vec3::new(a.cos(), 0.0, a.sin());
            let (s, e) = (dir * -0.55 + Vec3::new(0.0, 0.05, 0.0), dir * 0.55 + Vec3::new(0.0, 0.06, 0.0));
            p.push(limb(s, e, 0.06, 0x5a3e24));
            p.push(ball(e, 0.062, 0x8a6a44));
            for t in [-0.2f32, 0.15, 0.3] {
                let base = dir * t + Vec3::new(0.0, 0.06, 0.0);
                let side = Vec3::new(-dir.z, 0.0, dir.x) * (0.28 * if t > 0.0 { 1.0 } else { -1.0 });
                p.push(limb(base, base + side + dir * 0.1, 0.025, 0x4a3018));
            }
        }
        // Lézarde dans la pierre.
        11 => {
            let mut at = Vec3::new(-0.7, 0.01, rng.spread(0.3));
            let mut heading: f32 = 0.0;
            for _ in 0..7 {
                heading += rng.spread(0.7);
                let next = at + Vec3::new(heading.cos(), 0.0, heading.sin()) * 0.22;
                p.push(limb(at, next, 0.014, 0x120e16));
                if rng.next() < 0.4 {
                    let branch = next + Vec3::new((heading + 1.2).cos(), 0.0, (heading + 1.2).sin()) * 0.16;
                    p.push(limb(next, branch, 0.009, 0x120e16));
                }
                at = next;
            }
        }
        // Gravats.
        12 => {
            for _ in 0..(5 + variant) {
                let s = 0.05 + 0.08 * rng.next();
                let rot = Mat3::from_rotation_y(rng.next() * PI) * Mat3::from_rotation_x(rng.spread(0.3));
                let c = [0x7a7a82, 0x62626c, 0x8e8a80, 0x56565e][(rng.next() * 4.0) as usize % 4];
                p.push(shiny(block(Vec3::new(rng.spread(0.4), s * 0.6, rng.spread(0.35)), Vec3::new(s, s * 0.6, s * 0.8), rot, c), 0.2));
            }
            p.push(patch(0.0, 0.0, 0.45, 0.4, 0x3a363c));
        }
        // Ossements.
        13 => {
            for k in 0..3 {
                let a = k as f32 * 1.1 + v;
                let dir = Vec3::new(a.cos(), 0.0, a.sin());
                let c = Vec3::new(rng.spread(0.25), 0.04, rng.spread(0.25));
                p.push(limb(c - dir * 0.2, c + dir * 0.2, 0.025, 0xe4dcc4));
                p.push(ball(c + dir * 0.22, 0.04, 0xd8d0b4));
                p.push(ball(c - dir * 0.22, 0.04, 0xd8d0b4));
            }
            if variant % 2 == 0 {
                let skull = Vec3::new(0.15, 0.09, -0.1);
                p.push(shiny(ellipsoid(skull, Vec3::new(0.1, 0.09, 0.11), 0xe9e1cb), 0.4));
                p.push(ellipsoid(skull + Vec3::new(-0.04, 0.02, 0.09), Vec3::new(0.03, 0.035, 0.015), 0x1a1424));
                p.push(ellipsoid(skull + Vec3::new(0.04, 0.02, 0.09), Vec3::new(0.03, 0.035, 0.015), 0x1a1424));
            }
        }
        // Tapis (deux tuiles de large).
        14 => {
            let (body, border) = [(0x8a1c2c, 0xd8b04a), (0x24407a, 0xd8d0b0), (0x2e6a42, 0xd8b04a), (0x5a2a6a, 0xd8b04a)][variant % 4];
            let (w, d) = (1.05, 0.62);
            p.push(block(Vec3::new(0.0, 0.0, 0.0), Vec3::new(w + 0.04, 0.012, d + 0.04), Mat3::IDENTITY, 0x1c1418));
            p.push(block(Vec3::new(0.0, 0.016, 0.0), Vec3::new(w, 0.01, d), Mat3::IDENTITY, border));
            p.push(block(Vec3::new(0.0, 0.026, 0.0), Vec3::new(w - 0.1, 0.01, d - 0.1), Mat3::IDENTITY, body));
            p.push(block(Vec3::new(0.0, 0.034, 0.0), Vec3::new(w - 0.28, 0.008, d - 0.28), Mat3::IDENTITY, border));
            p.push(block(Vec3::new(0.0, 0.04, 0.0), Vec3::new(w - 0.34, 0.008, d - 0.34), Mat3::IDENTITY, body));
            p.push(shiny(ellipsoid(Vec3::new(0.0, 0.05, 0.0), Vec3::new(0.12, 0.01, 0.12), border), 0.5));
            for k in 0..9 {
                let z = -d + 0.12 + k as f32 * (d * 2.0 - 0.24) / 8.0;
                p.push(limb(Vec3::new(w, 0.012, z), Vec3::new(w + 0.1, 0.012, z), 0.012, border));
                p.push(limb(Vec3::new(-w, 0.012, z), Vec3::new(-w - 0.1, 0.012, z), 0.012, border));
            }
        }
        // Cendres et braises.
        _ => {
            for _ in 0..(5 + variant) {
                p.push(patch(rng.spread(0.4), rng.spread(0.35), 0.1 + 0.14 * rng.next(), 0.08 + 0.1 * rng.next(), if rng.next() < 0.5 { 0x2a2626 } else { 0x3a3432 }));
            }
            for _ in 0..(2 + variant) {
                p.push(glow(ball(Vec3::new(rng.spread(0.3), 0.03, rng.spread(0.28)), 0.02 + 0.015 * rng.next(), 0xf08a30)));
            }
        }
    }
    p
}
