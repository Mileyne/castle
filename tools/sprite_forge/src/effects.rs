//! Projectiles (orbe magique, flèche, tir de tourelle) et effets de coups (croissant d'épée, onde de marteau),
//! dans les deux thèmes. Les projectiles sont des modèles rendus vers la droite (+X à l'écran); le jeu les tourne.

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::{Mat3, Vec3};

use crate::render::{Camera, Part, render};
use crate::shapes::*;

/// Côté d'une case de projectile et d'effet de coup, en pixels.
pub const PROJECTILE_CELL: usize = 64;
pub const FX_CELL: usize = 96;
/// Cases de la planche de projectiles: orbe (2 images), flèche, tir de tourelle.
pub const PROJECTILES: usize = 4;
/// Cases de la planche d'effets: croissant d'épée, onde de marteau.
pub const FX: usize = 2;

const BLOOD: u32 = 0xa01820;
const BONE: u32 = 0xe4dcc4;

fn pointing(parts: &[Part]) -> Vec<[f32; 4]> {
    // Le modèle regarde vers +Z; avec ce lacet il pointe vers la droite de l'image.
    let cam = Camera { yaw: FRAC_PI_2, elevation: 0.3, scale: 56.0, size: PROJECTILE_CELL, feet_y: PROJECTILE_CELL as f32 / 2.0, outline: true, shadow: false };
    render(parts, &cam, 3)
}

/// Traînée lumineuse derrière un projectile: trois cônes qui s'effilent, plus quelques étincelles.
fn trail(color: u32, inner: u32, length: f32, radius: f32) -> Vec<Part> {
    let mut p = vec![
        glow(cone(Vec3::new(0.0, 0.0, -0.05), Vec3::new(0.0, 0.0, -length), radius, 0.01, color)),
        glow(cone(Vec3::new(0.0, 0.02, -0.05), Vec3::new(0.0, 0.01, -length * 0.65), radius * 0.55, 0.008, inner)),
    ];
    for k in 0..4 {
        let z = -length * (0.35 + 0.2 * k as f32);
        p.push(glow(ball(Vec3::new(if k % 2 == 0 { 0.07 } else { -0.06 }, 0.06 * (k as f32 - 1.5), z), 0.022, inner)));
    }
    p
}

pub fn projectile(theme: Theme, index: usize) -> Vec<[f32; 4]> {
    let parts: Vec<Part> = match (theme, index) {
        // Orbe magique: noyau lumineux, halo, traînée (deux images qui pulsent).
        (Theme::Manga | Theme::Medieval, 0 | 1) => {
            let r = if index == 0 { 0.17 } else { 0.2 };
            let mut p = vec![glow(ball(Vec3::ZERO, r, 0x7ae8f8)), glow(ball(Vec3::new(0.0, 0.0, r * 0.45), r * 0.62, 0xf8f0ff))];
            p.push(glow(cone(Vec3::new(0.0, 0.0, -0.04), Vec3::new(0.0, 0.0, -0.01), r * 1.5, r * 1.4, 0xa050d0)));
            p.extend(trail(0xa050d0, 0x7ae8f8, 0.75, r * 0.9));
            p
        }
        // Crâne enflammé: orbites vertes, flammes vertes.
        (Theme::Gore, 0 | 1) => {
            let flame = if index == 0 { 0.7 } else { 0.9 };
            let mut p = vec![shiny(ball(Vec3::ZERO, 0.15, BONE), 0.5), ellipsoid(Vec3::new(0.0, -0.1, 0.05), Vec3::new(0.09, 0.05, 0.1), 0xc8c0a8)];
            for sg in [-1.0f32, 1.0] {
                p.push(ball(Vec3::new(sg * 0.06, 0.03, 0.12), 0.045, DARK));
                p.push(glow(ball(Vec3::new(sg * 0.06, 0.03, 0.15), 0.025, 0x70ff40)));
            }
            p.extend(trail(0x40c030, 0xc0ff80, flame, 0.17));
            p.push(glow(cone(Vec3::new(0.0, 0.1, -0.05), Vec3::new(0.0, 0.3, -0.25), 0.08, 0.01, 0x60e040)));
            p
        }
        // Flèche: fût, pointe d'acier, empennage.
        (Theme::Manga | Theme::Medieval, 2) => {
            let mut p = vec![
                shiny(limb(Vec3::new(0.0, 0.0, -0.4), Vec3::new(0.0, 0.0, 0.25), 0.02, 0xb08a58), 0.5),
                shiny(cone(Vec3::new(0.0, 0.0, 0.24), Vec3::new(0.0, 0.0, 0.42), 0.055, 0.004, 0xd8dff0), 0.9),
            ];
            for k in 0..3 {
                let a = k as f32 * TAU / 3.0;
                p.push(block(Vec3::new(a.cos() * 0.04, a.sin() * 0.04, -0.34), Vec3::new(0.005, 0.045, 0.07), Mat3::from_rotation_z(a), if k == 0 { 0xd03030 } else { 0xf4f0e8 }));
            }
            p
        }
        // Flèche d'os: pointe d'obsidienne qui luit, plumes pourries, sang qui goutte.
        (Theme::Gore, 2) => {
            let mut p = vec![
                shiny(limb(Vec3::new(0.0, 0.0, -0.4), Vec3::new(0.0, 0.0, 0.25), 0.024, BONE), 0.5),
                shiny(cone(Vec3::new(0.0, 0.0, 0.24), Vec3::new(0.0, 0.0, 0.42), 0.06, 0.004, 0x2a2430), 0.8),
                glow(ball(Vec3::new(0.0, 0.0, 0.43), 0.03, 0xff3020)),
                cone(Vec3::new(0.0, 0.0, 0.2), Vec3::new(0.0, -0.1, 0.2), 0.025, 0.005, BLOOD),
            ];
            for k in 0..3 {
                let a = k as f32 * TAU / 3.0;
                p.push(block(Vec3::new(a.cos() * 0.04, a.sin() * 0.04, -0.34), Vec3::new(0.005, 0.045, 0.07), Mat3::from_rotation_z(a), 0x5a1820));
            }
            p
        }
        // Tir de tourelle: carreau incandescent.
        (Theme::Manga | Theme::Medieval, _) => {
            let mut p = vec![glow(ellipsoid(Vec3::ZERO, Vec3::new(0.1, 0.1, 0.28), 0xff8a2a)), glow(ellipsoid(Vec3::new(0.0, 0.0, 0.08), Vec3::new(0.055, 0.055, 0.2), 0xfff0b0))];
            p.extend(trail(0xff8a2a, 0xffd040, 0.7, 0.12));
            p
        }
        // Morceau de viande qui crache des braises rouges.
        (Theme::Gore, _) => {
            let mut p = vec![
                shiny(ellipsoid(Vec3::ZERO, Vec3::new(0.14, 0.12, 0.2), 0xb04058), 0.8),
                limb(Vec3::new(0.0, 0.0, 0.1), Vec3::new(0.0, 0.03, 0.36), 0.03, BONE),
                glow(ball(Vec3::new(0.0, 0.1, 0.0), 0.04, 0xff6050)),
            ];
            p.extend(trail(0xc01020, 0xff6a50, 0.7, 0.13));
            p
        }
    };
    pointing(&parts)
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Peint une case de `FX_CELL` pixels: `shade(x, y)` donne la couleur et l'opacité de chaque pixel.
fn paint(shade: impl Fn(f32, f32) -> [f32; 4]) -> Vec<[f32; 4]> {
    let mut img = vec![[0.0; 4]; FX_CELL * FX_CELL];
    for y in 0..FX_CELL {
        for x in 0..FX_CELL {
            img[y * FX_CELL + x] = shade(x as f32 + 0.5, y as f32 + 0.5);
        }
    }
    img
}

/// Croissant lumineux devant le héros: centré à gauche, ouvert vers la droite.
pub fn slash(theme: Theme) -> Vec<[f32; 4]> {
    let mut rng = Rng(5);
    // Gouttes projetées (mode sanglant): positions et tailles tirées une fois.
    let drops: Vec<(f32, f32, f32)> = (0..14)
        .map(|_| {
            let a = rng.spread(1.0);
            let d = 55.0 + rng.next() * 32.0;
            (6.0 + a.cos() * d, 48.0 + a.sin() * d, 1.6 + rng.next() * 2.4)
        })
        .collect();
    paint(|x, y| {
        let (dx, dy) = (x - 6.0, y - 48.0);
        let d = (dx * dx + dy * dy).sqrt();
        let angle = dy.atan2(dx).abs().to_degrees();
        // Épaisseur maximale au centre de l'arc, effilée aux extrémités.
        let taper = 1.0 - (angle / 64.0).powf(1.6);
        let outer = 63.0 - 5.0 * (1.0 - taper);
        let inner = 38.0 + 22.0 * (1.0 - taper);
        let body = smoothstep(inner - 1.5, inner + 1.5, d) * (1.0 - smoothstep(outer - 1.5, outer + 1.5, d)) * (1.0 - smoothstep(60.0, 66.0, angle));
        let heat = ((d - inner) / (outer - inner).max(1.0)).clamp(0.0, 1.0);
        let color = match theme {
            Theme::Manga | Theme::Medieval => {
                let t = heat.powf(0.8);
                [0.62 + 0.38 * t, 0.85 + 0.15 * t, 1.0]
            }
            Theme::Gore => {
                let t = heat.powf(0.7);
                [0.25 + 0.7 * t, 0.02 + 0.08 * t, 0.04 + 0.05 * t]
            }
        };
        let mut a = body;
        if theme == Theme::Gore {
            for (cx, cy, r) in &drops {
                let dd = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                a = a.max(1.0 - smoothstep(r - 0.8, r + 0.8, dd));
            }
        }
        [color[0], color[1], color[2], a]
    })
}

/// Onde de choc à l'impact du marteau: anneau, rayons d'étincelles et noyau.
pub fn smash(theme: Theme) -> Vec<[f32; 4]> {
    let (cx, cy) = (56.0, 48.0);
    let mut rng = Rng(9);
    let blobs: Vec<(f32, f32, f32)> = (0..18)
        .map(|_| {
            let a = rng.next() * TAU;
            let d = 24.0 + rng.next() * 22.0;
            (cx + a.cos() * d, cy + a.sin() * d, 1.5 + rng.next() * 3.0)
        })
        .collect();
    paint(|x, y| {
        let (dx, dy) = (x - cx, y - cy);
        let d = (dx * dx + dy * dy).sqrt();
        let ring = smoothstep(19.0, 22.0, d) * (1.0 - smoothstep(26.0, 29.0, d));
        let angle = dy.atan2(dx);
        let ray = (angle * 4.0).cos().max(0.0).powi(6) * smoothstep(8.0, 14.0, d) * (1.0 - smoothstep(30.0, 42.0, d));
        let core = 1.0 - smoothstep(8.0, 16.0, d);
        let (ring_c, core_c) = match theme {
            Theme::Manga | Theme::Medieval => ([1.0, 0.54, 0.16], [1.0, 0.94, 0.62]),
            Theme::Gore => ([0.7, 0.05, 0.08], [0.95, 0.35, 0.3]),
        };
        let mut a = ring.max(ray * 0.9).max(core);
        let mut color = if core > ring { core_c } else { ring_c };
        if theme == Theme::Gore {
            for (bx, by, r) in &blobs {
                let dd = ((x - bx).powi(2) + (y - by).powi(2)).sqrt();
                let blob = 1.0 - smoothstep(r - 0.8, r + 0.8, dd);
                if blob > a {
                    a = blob;
                    color = [0.55, 0.03, 0.06];
                }
            }
        }
        [color[0], color[1], color[2], a]
    })
}
