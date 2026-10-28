//! Les objets du château, rendus comme les personnages. Chacun a quatre images (impulsion lumineuse,
//! rotation, balancement) pour que le décor vive un peu. Dimensions en mètres du modèle.

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::{Mat3, Vec3};

use crate::render::Part;
use crate::shapes::*;

pub const FRAMES: usize = 4;

#[derive(Clone, Copy, Debug)]
pub enum Prop {
    Generator,
    Turret,
    Coin,
    Food,
    Key,
    Lever,
    Rune,
    Mechanism,
    TrapArmed,
    TrapOff,
    DoorLocked,
    DoorRoyal,
    DoorRoyalEmblem,
    Portal,
}

/// Ordre des lignes dans la planche (le client utilise les mêmes indices).
pub const PROPS: [Prop; 14] = [
    Prop::Generator,
    Prop::Turret,
    Prop::Coin,
    Prop::Food,
    Prop::Key,
    Prop::Lever,
    Prop::Rune,
    Prop::Mechanism,
    Prop::TrapArmed,
    Prop::TrapOff,
    Prop::DoorLocked,
    Prop::DoorRoyal,
    Prop::DoorRoyalEmblem,
    Prop::Portal,
];

const STONE: u32 = 0x5a5068;
const STONE_L: u32 = 0x7a708a;
const MAGENTA: u32 = 0xe040d0;
const BRASS: u32 = 0xc09040;

/// Disque ou cylindre dont l'axe est Z (face tournée vers le spectateur).
fn disc_z(center: Vec3, radius: f32, thick: f32, color: u32) -> Part {
    mk_disc(center, radius, thick, Mat3::from_rotation_x(FRAC_PI_2), color)
}

fn mk_disc(center: Vec3, radius: f32, thick: f32, rot: Mat3, color: u32) -> Part {
    crate::shapes::mk(
        crate::render::Shape::Frustum(radius, radius),
        center,
        rot,
        Vec3::new(1.0, thick / 2.0, 1.0),
        color,
    )
}

fn generator(t: f32) -> Vec<Part> {
    let pulse = 1.0 + 0.07 * (t * TAU).sin();
    let bob = 0.05 * (t * TAU).sin();
    let mut p = vec![
        shiny(cone(Vec3::ZERO, Vec3::new(0.0, 0.2, 0.0), 1.05, 0.9, STONE), 0.3),
        shiny(cone(Vec3::new(0.0, 0.2, 0.0), Vec3::new(0.0, 0.42, 0.0), 0.8, 0.62, STONE_L), 0.3),
        glow(cone(Vec3::new(0.0, 0.42, 0.0), Vec3::new(0.0, 0.45, 0.0), 0.6, 0.56, MAGENTA)),
    ];
    // Pointes d'os en couronne.
    for k in 0..7 {
        let a = k as f32 * TAU / 7.0;
        let (c, s) = (a.cos(), a.sin());
        p.push(shiny(cone(Vec3::new(c * 0.78, 0.2, s * 0.78), Vec3::new(c * 0.9, 0.85, s * 0.9), 0.07, 0.01, 0xe9e1cb), 0.5));
    }
    // Cristal flottant en deux pyramides, noyau blanc et étincelles en orbite.
    let y = 1.0 + bob;
    p.push(glow(cone(Vec3::new(0.0, y - 0.38 * pulse, 0.0), Vec3::new(0.0, y, 0.0), 0.02, 0.3 * pulse, MAGENTA)));
    p.push(glow(cone(Vec3::new(0.0, y, 0.0), Vec3::new(0.0, y + 0.4 * pulse, 0.0), 0.3 * pulse, 0.02, 0xff80ee)));
    p.push(glow(ball(Vec3::new(0.0, y, 0.0), 0.13 * pulse, 0xffffff)));
    for k in 0..3 {
        let a = t * TAU + k as f32 * 2.1;
        p.push(glow(ball(Vec3::new(a.cos() * 0.55, y + 0.1 * (a * 2.0).sin(), a.sin() * 0.55), 0.04, 0xffb0f0)));
    }
    p
}

fn turret(t: f32) -> Vec<Part> {
    let recoil = 0.04 * (t * TAU).sin().max(0.0);
    let mut p = vec![
        shiny(cone(Vec3::ZERO, Vec3::new(0.0, 0.14, 0.0), 0.62, 0.55, BRASS), 0.6),
        shiny(cone(Vec3::new(0.0, 0.14, 0.0), Vec3::new(0.0, 0.2, 0.0), 0.5, 0.5, 0x6a4a20), 0.3),
        shiny(ellipsoid(Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.4, 0.34, 0.4), 0x8a94a8), 0.7),
        shiny(cone(Vec3::new(0.0, 0.46, 0.0), Vec3::new(0.0, 0.54, 0.0), 0.43, 0.43, BRASS), 0.6),
        shiny(ellipsoid(Vec3::new(0.0, 0.82, 0.0), Vec3::new(0.3, 0.22, 0.3), BRASS), 0.8),
        // Canon incliné vers l'avant à droite, avec sa bouche incandescente.
        shiny(limb(Vec3::new(0.05, 0.78, 0.15), Vec3::new(0.42 - recoil, 0.9, 0.78 - recoil), 0.11, 0x4a5060), 0.7),
        shiny(limb(Vec3::new(0.38 - recoil, 0.88, 0.72 - recoil), Vec3::new(0.44 - recoil, 0.9, 0.84 - recoil), 0.14, BRASS), 0.7),
        glow(ball(Vec3::new(0.45 - recoil, 0.9, 0.86 - recoil), 0.07, 0xff9a30)),
        // Cadran lumineux et tuyaux.
        glow(ball(Vec3::new(-0.22, 0.55, 0.34), 0.07, 0xffd040)),
        shiny(limb(Vec3::new(-0.38, 0.2, 0.0), Vec3::new(-0.3, 0.75, -0.1), 0.045, BRASS), 0.6),
        shiny(limb(Vec3::new(0.38, 0.2, -0.1), Vec3::new(0.3, 0.7, -0.2), 0.045, BRASS), 0.6),
    ];
    // Engrenage visible sur le flanc.
    let g = Vec3::new(-0.34, 0.45, 0.2);
    p.push(shiny(disc_z(g, 0.17, 0.06, 0x9aa0b0), 0.6));
    for k in 0..8 {
        let a = k as f32 * TAU / 8.0 + t * TAU / 8.0;
        p.push(block(g + Vec3::new(a.cos() * 0.19, a.sin() * 0.19, 0.0), Vec3::new(0.035, 0.035, 0.03), Mat3::from_rotation_z(a), 0x9aa0b0));
    }
    p
}

fn coin(t: f32) -> Vec<Part> {
    let spin = Mat3::from_rotation_y(t * TAU / 2.0) * Mat3::from_rotation_x(FRAC_PI_2);
    let c = Vec3::new(0.0, 0.32 + 0.02 * (t * TAU).sin(), 0.0);
    vec![
        shiny(mk_disc(c, 0.3, 0.07, spin, 0xd9a520), 0.9),
        shiny(mk_disc(c + spin * Vec3::Y * 0.04, 0.22, 0.04, spin, 0xffd84a), 0.9),
        glow(mk_disc(c + spin * Vec3::Y * 0.065, 0.07, 0.02, spin, 0xfff0a0)),
    ]
}

fn food(t: f32) -> Vec<Part> {
    let bob = 0.02 * (t * TAU).sin();
    let meat = Vec3::new(0.0, 0.34 + bob, 0.0);
    vec![
        shiny(ellipsoid(meat, Vec3::new(0.3, 0.24, 0.24), 0xb8502a), 0.9),
        shiny(ellipsoid(meat + Vec3::new(-0.08, 0.1, 0.1), Vec3::new(0.16, 0.1, 0.1), 0xd8703a), 0.9),
        limb(meat + Vec3::new(0.2, -0.04, 0.0), meat + Vec3::new(0.55, -0.16, 0.0), 0.055, 0xf4f0e0),
        ball(meat + Vec3::new(0.58, -0.12, 0.05), 0.075, 0xf4f0e0),
        ball(meat + Vec3::new(0.58, -0.2, -0.05), 0.075, 0xf4f0e0),
        glow(ball(meat + Vec3::new(-0.15, 0.2, 0.12), 0.035, 0xffffff)),
    ]
}

fn key(t: f32) -> Vec<Part> {
    let yaw = Mat3::from_rotation_y(0.5 * (t * TAU).sin());
    let c = Vec3::new(0.0, 0.4 + 0.03 * (t * TAU).sin(), 0.0);
    let mut p = Vec::new();
    for k in 0..10 {
        let a = k as f32 * TAU / 10.0;
        p.push(shiny(ball(c + yaw * Vec3::new(-0.22 + a.cos() * 0.15, a.sin() * 0.15, 0.0), 0.045, 0xf6c93a), 0.9));
    }
    p.push(shiny(limb(c + yaw * Vec3::new(-0.08, 0.0, 0.0), c + yaw * Vec3::new(0.45, 0.0, 0.0), 0.045, 0xf6c93a), 0.9));
    for x in [0.3, 0.4] {
        p.push(shiny(block(c + yaw * Vec3::new(x, -0.07, 0.0), Vec3::new(0.025, 0.07, 0.03), yaw, 0xf6c93a), 0.9));
    }
    p.push(glow(ball(c + yaw * Vec3::new(-0.3, 0.14, 0.04), 0.025, 0xffffff)));
    p
}

fn lever(t: f32) -> Vec<Part> {
    let sway = 0.04 * (t * TAU).sin();
    vec![
        shiny(block(Vec3::new(0.0, 0.14, 0.0), Vec3::new(0.45, 0.14, 0.32), Mat3::IDENTITY, STONE), 0.3),
        block(Vec3::new(0.0, 0.29, 0.0), Vec3::new(0.3, 0.015, 0.06), Mat3::IDENTITY, DARK),
        shiny(limb(Vec3::new(0.0, 0.3, 0.0), Vec3::new(-0.2 + sway, 1.0, 0.08), 0.05, 0x9aa0b0), 0.8),
        shiny(ball(Vec3::new(-0.21 + sway, 1.06, 0.09), 0.11, 0xe03040), 0.9),
        ball(Vec3::new(0.0, 0.3, 0.0), 0.08, BRASS),
    ]
}

fn rune(t: f32) -> Vec<Part> {
    let pulse = 0.5 + 0.5 * (t * TAU).sin();
    let mut p = vec![
        shiny(cone(Vec3::ZERO, Vec3::new(0.0, 0.04, 0.0), 1.0, 1.0, 0x2a2040), 0.3),
        glow(cone(Vec3::new(0.0, 0.04, 0.0), Vec3::new(0.0, 0.06, 0.0), 0.92, 0.82, 0x40d8f0)),
        glow(cone(Vec3::new(0.0, 0.04, 0.0), Vec3::new(0.0, 0.07, 0.0), 0.5, 0.44, 0x7a60ff)),
    ];
    for k in 0..8 {
        let a = k as f32 * TAU / 8.0;
        p.push(glow(block(
            Vec3::new(a.cos() * 0.68, 0.07, a.sin() * 0.68),
            Vec3::new(0.14, 0.01, 0.03),
            Mat3::from_rotation_y(-a),
            0x60e8ff,
        )));
    }
    for k in 0..3 {
        let a = t * TAU / 3.0 + k as f32 * TAU / 3.0;
        p.push(glow(block(
            Vec3::new(a.cos() * 0.5, 0.45 + 0.12 * pulse + 0.05 * k as f32, a.sin() * 0.5),
            Vec3::new(0.06, 0.06, 0.012),
            Mat3::from_rotation_y(-a) * Mat3::from_rotation_z(0.785),
            0x9affff,
        )));
    }
    p
}

fn mechanism(t: f32) -> Vec<Part> {
    let g = Vec3::new(0.0, 0.85, 0.3);
    let mut p = vec![
        shiny(block(Vec3::new(0.0, 0.22, 0.0), Vec3::new(0.65, 0.22, 0.45), Mat3::IDENTITY, STONE), 0.3),
        shiny(disc_z(g, 0.4, 0.1, 0xb08838), 0.8),
        shiny(disc_z(g + Vec3::Z * 0.06, 0.12, 0.06, 0x6a4a20), 0.6),
        glow(block(Vec3::new(0.3, 0.3, 0.46), Vec3::new(0.16, 0.1, 0.01), Mat3::IDENTITY, 0xff8a2a)),
        shiny(limb(Vec3::new(-0.5, 0.42, 0.0), Vec3::new(-0.5, 1.3, -0.1), 0.055, 0x7f8aa6), 0.6),
    ];
    // Dents de l'engrenage: elles tournent d'un pas en quatre images.
    for k in 0..10 {
        let a = k as f32 * TAU / 10.0 + t * TAU / 10.0 * 0.9;
        p.push(shiny(block(g + Vec3::new(a.cos() * 0.44, a.sin() * 0.44, 0.0), Vec3::new(0.065, 0.065, 0.055), Mat3::from_rotation_z(a), 0xb08838), 0.8));
    }
    let small = Vec3::new(0.45, 0.62, 0.3);
    p.push(shiny(disc_z(small, 0.16, 0.08, 0x9aa0b0), 0.7));
    for k in 0..6 {
        let a = k as f32 * TAU / 6.0 - t * TAU / 6.0 * 0.9;
        p.push(block(small + Vec3::new(a.cos() * 0.18, a.sin() * 0.18, 0.0), Vec3::new(0.04, 0.04, 0.04), Mat3::from_rotation_z(a), 0x9aa0b0));
    }
    p
}

fn trap(armed: bool, t: f32) -> Vec<Part> {
    let mut p = vec![
        shiny(block(Vec3::new(0.0, 0.05, 0.0), Vec3::new(0.62, 0.05, 0.62), Mat3::IDENTITY, 0x5a6070), 0.5),
        block(Vec3::new(0.0, 0.11, 0.0), Vec3::new(0.56, 0.015, 0.56), Mat3::IDENTITY, 0x3a4050),
    ];
    for ix in -1..=1 {
        for iz in -1..=1 {
            let (x, z) = (ix as f32 * 0.32, iz as f32 * 0.32);
            if armed {
                let h = 0.5 + 0.03 * (t * TAU + ix as f32 + iz as f32).sin();
                p.push(shiny(cone(Vec3::new(x, 0.1, z), Vec3::new(x, 0.1 + h, z), 0.07, 0.004, 0xd8dff0), 0.9));
            } else {
                p.push(ellipsoid(Vec3::new(x, 0.11, z), Vec3::new(0.07, 0.02, 0.07), DARK));
                p.push(cone(Vec3::new(x, 0.1, z), Vec3::new(x, 0.17, z), 0.05, 0.004, 0x9aa0b0));
            }
        }
    }
    if armed {
        p.push(block(Vec3::new(0.22, 0.12, 0.45), Vec3::new(0.05, 0.005, 0.03), Mat3::IDENTITY, 0x8a1c2c));
    }
    p
}

fn door_locked(t: f32) -> Vec<Part> {
    let mut p = vec![
        shiny(block(Vec3::new(-0.58, 0.95, 0.0), Vec3::new(0.12, 0.95, 0.16), Mat3::IDENTITY, STONE_L), 0.3),
        shiny(block(Vec3::new(0.58, 0.95, 0.0), Vec3::new(0.12, 0.95, 0.16), Mat3::IDENTITY, STONE_L), 0.3),
        shiny(block(Vec3::new(0.0, 1.95, 0.0), Vec3::new(0.7, 0.1, 0.16), Mat3::IDENTITY, STONE), 0.3),
    ];
    for (k, x) in [-0.36, -0.12, 0.12, 0.36].into_iter().enumerate() {
        p.push(shiny(block(Vec3::new(x, 0.9, 0.0), Vec3::new(0.115, 0.9, 0.06), Mat3::IDENTITY, if k % 2 == 0 { 0x8a5630 } else { 0x74462a }), 0.4));
    }
    for y in [0.3, 1.45] {
        p.push(shiny(block(Vec3::new(0.0, y, 0.06), Vec3::new(0.5, 0.07, 0.04), Mat3::IDENTITY, 0x4a5060), 0.7));
        for x in [-0.4, -0.13, 0.13, 0.4] {
            p.push(ball(Vec3::new(x, y, 0.1), 0.025, 0x9aa0b0));
        }
    }
    p.push(shiny(block(Vec3::new(0.0, 0.85, 0.1), Vec3::new(0.14, 0.17, 0.03), Mat3::IDENTITY, 0xf6c93a), 0.9));
    p.push(ball(Vec3::new(0.0, 0.88, 0.14), 0.04, DARK));
    p.push(block(Vec3::new(0.0, 0.8, 0.14), Vec3::new(0.014, 0.05, 0.01), Mat3::IDENTITY, DARK));
    p.push(glow(ball(Vec3::new(0.06 + 0.02 * (t * TAU).sin(), 0.95, 0.15), 0.015, 0xffffff)));
    p
}

fn door_royal(emblem: bool, t: f32) -> Vec<Part> {
    let slab = 0x3e4672;
    let mut p = vec![
        shiny(block(Vec3::new(0.0, 0.95, 0.0), Vec3::new(0.6, 0.95, 0.14), Mat3::IDENTITY, slab), 0.4),
        shiny(block(Vec3::new(-0.58, 0.95, 0.04), Vec3::new(0.05, 0.95, 0.12), Mat3::IDENTITY, 0xf6c93a), 0.9),
        shiny(block(Vec3::new(0.58, 0.95, 0.04), Vec3::new(0.05, 0.95, 0.12), Mat3::IDENTITY, 0xf6c93a), 0.9),
        shiny(block(Vec3::new(0.0, 1.85, 0.04), Vec3::new(0.6, 0.05, 0.12), Mat3::IDENTITY, 0xf6c93a), 0.9),
        shiny(block(Vec3::new(0.0, 0.05, 0.04), Vec3::new(0.6, 0.05, 0.12), Mat3::IDENTITY, 0xf6c93a), 0.9),
        block(Vec3::new(0.0, 0.95, 0.12), Vec3::new(0.44, 0.78, 0.02), Mat3::IDENTITY, 0x4a5488),
        glow(block(Vec3::new(0.0, 0.95, 0.15), Vec3::new(0.01, 0.78, 0.01), Mat3::IDENTITY, 0xf6c93a)),
    ];
    if emblem {
        let glint = 0.9 + 0.1 * (t * TAU).sin();
        p.push(shiny(block(Vec3::new(0.0, 0.95, 0.16), Vec3::new(0.26, 0.09, 0.03), Mat3::IDENTITY, 0xf6c93a), 0.9));
        for (x, h) in [(-0.2, 0.2), (-0.1, 0.3), (0.0, 0.38), (0.1, 0.3), (0.2, 0.2)] {
            p.push(shiny(cone(Vec3::new(x, 1.0, 0.16), Vec3::new(x, 1.0 + h, 0.16), 0.045, 0.012, 0xf6c93a), 0.9));
            p.push(glow(ball(Vec3::new(x, 1.03 + h, 0.16), 0.03 * glint, 0xff5070)));
        }
    }
    p
}

fn portal(t: f32) -> Vec<Part> {
    let c = Vec3::new(0.0, 1.15, 0.0);
    let mut p = vec![
        shiny(cone(Vec3::ZERO, Vec3::new(0.0, 0.14, 0.0), 1.25, 1.1, STONE), 0.3),
        shiny(cone(Vec3::new(0.0, 0.14, 0.0), Vec3::new(0.0, 0.26, 0.0), 1.0, 0.9, STONE_L), 0.3),
    ];
    // Anneau de pierre runique.
    for k in 0..14 {
        let a = k as f32 * TAU / 14.0;
        let pos = c + Vec3::new(a.cos() * 1.0, a.sin() * 1.0, 0.0);
        p.push(shiny(block(pos, Vec3::new(0.16, 0.12, 0.14), Mat3::from_rotation_z(a), if k % 2 == 0 { STONE } else { STONE_L }), 0.4));
        p.push(glow(block(pos + Vec3::Z * 0.13, Vec3::new(0.05, 0.02, 0.01), Mat3::from_rotation_z(a), 0x60e8ff)));
    }
    // Tourbillon: disques concentriques et bras en spirale qui tournent.
    p.push(glow(disc_z(c + Vec3::Z * -0.02, 0.86, 0.02, 0x3a2a9a)));
    p.push(glow(disc_z(c, 0.64, 0.02, 0x6a50e8)));
    p.push(glow(disc_z(c + Vec3::Z * 0.02, 0.42, 0.02, 0x40c0f0)));
    p.push(glow(disc_z(c + Vec3::Z * 0.04, 0.2, 0.02, 0xd8f8ff)));
    for arm in 0..4 {
        for step in 0..6 {
            let a = arm as f32 * FRAC_PI_2 + t * FRAC_PI_2 + step as f32 * 0.35;
            let r = 0.12 + step as f32 * 0.12;
            p.push(glow(ball(c + Vec3::new(a.cos() * r, a.sin() * r, 0.08), 0.045 + 0.005 * step as f32, 0xaaf4ff)));
        }
    }
    p
}

pub fn build(prop: Prop, t: f32, theme: Theme) -> Vec<Part> {
    match theme {
        Theme::Manga | Theme::Medieval => build_manga(prop, t),
        Theme::Gore => crate::props_gore::build(prop, t),
    }
}

pub(crate) fn build_manga(prop: Prop, t: f32) -> Vec<Part> {
    match prop {
        Prop::Generator => generator(t),
        Prop::Turret => turret(t),
        Prop::Coin => coin(t),
        Prop::Food => food(t),
        Prop::Key => key(t),
        Prop::Lever => lever(t),
        Prop::Rune => rune(t),
        Prop::Mechanism => mechanism(t),
        Prop::TrapArmed => trap(true, t),
        Prop::TrapOff => trap(false, t),
        Prop::DoorLocked => door_locked(t),
        Prop::DoorRoyal => door_royal(false, t),
        Prop::DoorRoyalEmblem => door_royal(true, t),
        Prop::Portal => portal(t),
    }
}
