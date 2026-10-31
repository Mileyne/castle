//! Les trois boss de la campagne, bâtis sur les modèles des ennemis ordinaires mais plus sombres et parés de leurs signes:
//! le chef de meute (loup noir à collier de fer), le roi-troll (orque couronné) et Lord Muldant (colosse orange à crinière blonde).
//! Comme le loup et l'orque, ils sont dessinés à la taille de leur case; c'est le jeu qui les agrandit à l'écran.

use std::f32::consts::TAU;

use glam::{Mat3, Vec3};

use crate::heroes::{Pose, joints};
use crate::monsters_more::{orc, orc_dims, wolf};
use crate::render::Part;
use crate::shapes::*;

const IRON: u32 = 0x6a7080;
const IRON_D: u32 = 0x3a3e4a;

/// Multiplie la couleur des parties (assombrir, teinter), sans toucher aux parties lumineuses.
fn tint(parts: &mut [Part], k: [f32; 3]) {
    for part in parts.iter_mut().filter(|p| !p.glow) {
        for c in 0..3 {
            part.color[c] = (part.color[c] * k[c]).clamp(0.0, 1.0);
        }
    }
}

/// Croc-Noir: un loup au pelage de nuit, yeux rouges et gueule sanglante, une crête de piques le long de l'échine et un
/// collier de fer hérissé.
pub(crate) fn warg(pose: Pose) -> Vec<Part> {
    let mut p = wolf(pose, true);
    tint(&mut p, [0.5, 0.48, 0.55]);
    for k in 0..9 {
        let a = k as f32 * TAU / 9.0;
        let base = Vec3::new(a.cos() * 0.09, 0.6 + a.sin() * 0.1, 0.4);
        let tip = base + Vec3::new(a.cos() * 0.1, a.sin() * 0.1, 0.02);
        p.push(shiny(cone(base, tip, 0.022, 0.003, IRON), 0.7));
    }
    p.push(shiny(
        limb(
            Vec3::new(-0.1, 0.6, 0.4),
            Vec3::new(0.1, 0.6, 0.4),
            0.05,
            IRON_D,
        ),
        0.5,
    ));
    for k in 0..7 {
        let z = 0.3 - k as f32 * 0.09;
        let base = Vec3::new(0.0, 0.62, z);
        p.push(shiny(
            cone(
                base,
                base + Vec3::new(0.0, 0.14, -0.04),
                0.03,
                0.004,
                IRON_D,
            ),
            0.6,
        ));
    }
    // Moins grand que le loup ordinaire: ses piques et sa queue doivent tenir dans la case.
    scaled(p, 1.25)
}

/// Grogoth: l'orque en plus lourd, la peau grise-verte, une couronne de fer et d'or ébréchée sur la tête.
pub(crate) fn troll(pose: Pose) -> Vec<Part> {
    let d = orc_dims();
    let j = joints(&d, pose, None);
    let mut p = orc(pose);
    tint(&mut p, [0.85, 0.95, 1.05]);
    let h = j.head + Vec3::new(0.0, 0.0, 0.05);
    let r = d.head_r;
    p.push(shiny(
        ellipsoid(
            h + Vec3::new(0.0, 0.78 * r, 0.0),
            Vec3::new(0.98, 0.2, 0.98) * r,
            GOLD,
        ),
        0.8,
    ));
    for k in 0..7 {
        let a = k as f32 * TAU / 7.0;
        let base = h + Vec3::new(a.cos() * 0.8 * r, 0.9 * r, a.sin() * 0.8 * r);
        let height = if k % 2 == 0 { 0.7 } else { 0.45 };
        p.push(shiny(
            cone(
                base,
                base + Vec3::new(0.0, height * r, 0.0),
                0.16 * r,
                0.02 * r,
                GOLD,
            ),
            0.8,
        ));
    }
    p.push(glow(ball(
        h + Vec3::new(0.0, 0.78 * r, 0.98 * r),
        0.1 * r,
        0xff3040,
    )));
    // La couronne dépasse: on le dessine plus petit que l'orque pour qu'elle reste dans la case.
    scaled(p, 0.95)
}

/// Remplace une couleur par une autre dans les parties non lumineuses.
fn recolor(parts: &mut [Part], from: u32, to: u32) {
    let from = rgb(from);
    for part in parts.iter_mut().filter(|p| !p.glow) {
        if part.color.iter().zip(from).all(|(c, f)| (c - f).abs() < 0.004) {
            part.color = rgb(to);
        }
    }
}

/// Lord Muldant: un colosse (l'orque en bien plus large) au teint très orange, une crinière blonde très jaune qui
/// l'entoure de partout, un manteau noir doublé de pourpre, et la Pierre Ancestrale corrompue qui brille sur sa poitrine.
pub(crate) fn muldant(pose: Pose) -> Vec<Part> {
    let d = orc_dims();
    let j = joints(&d, pose, None);
    let mut p = orc(pose);
    recolor(&mut p, 0x5a7048, 0xf28a24);
    recolor(&mut p, 0x3e5230, 0xc4620e);
    recolor(&mut p, 0x16121a, 0xffe51a);
    recolor(&mut p, 0xc8c0a0, 0xe8d8b0);
    // Gros: plus large de moitié, un peu moins haut.
    for part in &mut p {
        part.center.x *= 1.45;
        part.center.z *= 1.25;
        part.scale.x *= 1.45;
        part.scale.z *= 1.25;
    }
    let h = j.head + Vec3::new(0.0, 0.0, 0.05);
    let r = d.head_r;
    let yellow = 0xffe51a;
    let yellow_d = 0xe8c010;
    // Crinière: une masse derrière et autour de la tête, des mèches qui retombent sur les épaules et dans le dos.
    p.push(shiny(ellipsoid(h + Vec3::new(0.0, 0.15 * r, -0.35 * r), Vec3::new(1.35, 1.2, 1.0) * r, yellow), 0.3));
    for sg in [-1.0f32, 1.0] {
        p.push(shiny(ellipsoid(h + Vec3::new(sg * 1.0 * r, -0.1 * r, -0.1 * r), Vec3::new(0.5, 1.0, 0.7) * r, yellow_d), 0.3));
    }
    for k in 0..9 {
        let a = k as f32 * TAU / 9.0;
        let base = h + Vec3::new(a.cos() * 1.0 * r, 0.8 * r + 0.2 * r * a.sin(), -0.2 * r - 0.5 * r * a.sin().abs());
        let len = (0.7 + 0.25 * (k % 3) as f32) * r;
        p.push(shiny(cone(base, base + Vec3::new(a.cos() * 0.4 * r + j.sway * 2.0, len * 0.9, -0.2 * r - j.trail * 0.4), 0.22 * r, 0.03 * r, if k % 2 == 0 { yellow } else { yellow_d }), 0.35));
    }
    for sg in [-1.0f32, 1.0] {
        let top = h + Vec3::new(sg * 0.8 * r, -0.3 * r, -0.2 * r);
        p.push(shiny(limb(top, top + Vec3::new(sg * 0.2 * r + j.sway, -1.8 * r, -0.4 * r), 0.2 * r, yellow), 0.3));
    }
    // Manteau noir doublé de pourpre qui tombe dans le dos.
    let cloak = 0x1c1424;
    let lining = 0x5a1c6a;
    for k in 0..6 {
        let x = (k as f32 - 2.5) * 0.17;
        let top = Vec3::new(x, j.shoulder_y - 0.02, -d.torso_rz * 1.2);
        p.push(panel(top, top + Vec3::new(j.sway * 2.0, -0.62, -0.12 - j.trail * 0.4), 0.2, 0.02, if k % 2 == 0 { cloak } else { lining }));
    }
    // La Pierre Ancestrale sur la poitrine, dans une monture de fer.
    let stone = Vec3::new(0.0, j.hip_y + d.torso * 0.72, d.torso_rz * 1.25 + 0.04);
    p.push(shiny(ball(stone, 0.075, IRON_D), 0.7));
    p.push(glow(ball(stone + Vec3::new(0.0, 0.0, 0.03), 0.05, 0xb040ff)));
    p.push(glow(ball(stone + Vec3::new(0.0, 0.0, 0.05), 0.025, 0xf0d8ff)));
    // La crinière et la largeur dépassent: on le dessine plus petit pour qu'il tienne dans la case.
    scaled(p, 0.66)
}

