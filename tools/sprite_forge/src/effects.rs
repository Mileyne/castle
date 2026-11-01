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
        (Theme::Medieval, 0 | 1) => {
            let r = if index == 0 { 0.17 } else { 0.2 };
            let mut p = vec![glow(ball(Vec3::ZERO, r, 0x7ae8f8)), glow(ball(Vec3::new(0.0, 0.0, r * 0.45), r * 0.62, 0xf8f0ff))];
            p.push(glow(cone(Vec3::new(0.0, 0.0, -0.04), Vec3::new(0.0, 0.0, -0.01), r * 1.5, r * 1.4, 0xa050d0)));
            p.extend(trail(0xa050d0, 0x7ae8f8, 0.75, r * 0.9));
            p
        }
        // Flèche: fût, pointe d'acier, empennage.
        (Theme::Medieval, 2) => {
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
        // Tir de tourelle: carreau incandescent.
        (Theme::Medieval, _) => {
            let mut p = vec![glow(ellipsoid(Vec3::ZERO, Vec3::new(0.1, 0.1, 0.28), 0xff8a2a)), glow(ellipsoid(Vec3::new(0.0, 0.0, 0.08), Vec3::new(0.055, 0.055, 0.2), 0xfff0b0))];
            p.extend(trail(0xff8a2a, 0xffd040, 0.7, 0.12));
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
pub fn slash(_theme: Theme) -> Vec<[f32; 4]> {
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
        let t = heat.powf(0.8);
        let color = [0.62 + 0.38 * t, 0.85 + 0.15 * t, 1.0];
        let a = body;
        [color[0], color[1], color[2], a]
    })
}

/// Onde de choc à l'impact du marteau: anneau, rayons d'étincelles et noyau.
pub fn smash(_theme: Theme) -> Vec<[f32; 4]> {
    let (cx, cy) = (56.0, 48.0);
    paint(|x, y| {
        let (dx, dy) = (x - cx, y - cy);
        let d = (dx * dx + dy * dy).sqrt();
        let ring = smoothstep(19.0, 22.0, d) * (1.0 - smoothstep(26.0, 29.0, d));
        let angle = dy.atan2(dx);
        let ray = (angle * 4.0).cos().max(0.0).powi(6) * smoothstep(8.0, 14.0, d) * (1.0 - smoothstep(30.0, 42.0, d));
        let core = 1.0 - smoothstep(8.0, 16.0, d);
        let (ring_c, core_c) = ([1.0, 0.54, 0.16], [1.0, 0.94, 0.62]);
        let a = ring.max(ray * 0.9).max(core);
        let color = if core > ring { core_c } else { ring_c };
        [color[0], color[1], color[2], a]
    })
}
