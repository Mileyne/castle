//! Les trois boss de la campagne, bâtis sur les modèles des ennemis ordinaires mais plus sombres et parés de leurs signes:
//! le chef de meute (loup noir à collier de fer), le roi-troll (orque couronné) et le roi-liche (squelette en robe).
//! Comme le loup et l'orque, ils sont dessinés à la taille de leur case; c'est le jeu qui les agrandit à l'écran.

use std::f32::consts::TAU;

use glam::{Mat3, Vec3};

use crate::heroes::{Pose, joints};
use crate::monsters::{skeleton, skeleton_dims};
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

/// Mortemer: le squelette en grande robe violette, la couronne de flammes d'âme, un orbe de mort dans la main gauche.
pub(crate) fn lich(pose: Pose) -> Vec<Part> {
    let d = skeleton_dims();
    let j = joints(&d, pose, None);
    let mut p = skeleton(pose);
    let robe = 0x3a1c5a;
    let robe_d = 0x24103c;
    // Robe: des pans qui descendent des épaules jusqu'au sol, devant, derrière et sur les côtés.
    let top = j.shoulder_y - 0.02;
    for k in 0..9 {
        let a = k as f32 * TAU / 9.0;
        let (x, z) = (a.sin() * 0.2, a.cos() * 0.15);
        let sway = j.sway * 1.5 * (1.0 + a.cos());
        p.push(panel(
            Vec3::new(x, top, z),
            Vec3::new(x * 1.6 + sway, 0.04, z * 1.9 - j.trail * 0.2),
            0.16,
            0.02,
            if k % 2 == 0 { robe } else { robe_d },
        ));
    }
    p.push(shiny(
        ellipsoid(
            Vec3::new(0.0, top - 0.04, 0.0),
            Vec3::new(0.22, 0.1, 0.16),
            robe,
        ),
        0.3,
    ));
    p.push(shiny(
        limb(
            Vec3::new(-0.2, top, 0.0),
            Vec3::new(0.2, top, 0.0),
            0.05,
            GOLD,
        ),
        0.8,
    ));
    // Couronne de pointes d'or sur le crâne.
    let h = j.head;
    let r = d.head_r;
    p.push(shiny(
        ellipsoid(
            h + Vec3::new(0.0, 0.7 * r, 0.0),
            Vec3::new(0.9, 0.16, 0.9) * r,
            GOLD,
        ),
        0.8,
    ));
    for k in 0..5 {
        let a = k as f32 * TAU / 5.0 + 0.3;
        let base = h + Vec3::new(a.cos() * 0.75 * r, 0.78 * r, a.sin() * 0.75 * r);
        p.push(shiny(
            cone(
                base,
                base + Vec3::new(0.0, 0.55 * r, 0.0),
                0.14 * r,
                0.02 * r,
                GOLD,
            ),
            0.8,
        ));
    }
    // Orbe de mort flottant au-dessus de la main gauche, et son halo.
    let orb = j.hand[1] + Vec3::new(0.05, 0.22, 0.08);
    p.push(glow(ball(orb, 0.09, 0xb060ff)));
    p.push(glow(ball(orb, 0.05, 0xf0e0ff)));
    p.push(block(
        j.hand[1] + Vec3::new(0.05, 0.1, 0.08),
        Vec3::new(0.012, 0.1, 0.012),
        Mat3::IDENTITY,
        DARK,
    ));
    p
}
