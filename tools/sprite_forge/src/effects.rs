//! Projectiles (orbe magique, flèche, tir de tourelle). Les projectiles sont des modèles rendus vers la droite (+X à l'écran); le jeu les tourne.

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::{Mat3, Vec3};

use crate::render::{Camera, Part, render};
use crate::shapes::*;

/// Côté d'une case de projectile, en pixels.
pub const PROJECTILE_CELL: usize = 64;
/// Cases de la planche de projectiles: orbe (2 images), flèche, tir de tourelle.
pub const PROJECTILES: usize = 4;


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
