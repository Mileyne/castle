//! Trois ennemis de plus: le gobelin (petit, vif et fourbe), l'orque (massif, armé d'une hache) et le goule (décharné,
//! courbé, aux longues griffes). Même squelette et mêmes poses que les autres monstres.

use std::f32::consts::TAU;

use glam::{Mat3, Vec3};

use crate::heroes::{Dims, Joints, Pose, held, joints};
use crate::monsters::Monster;
use crate::render::Part;
use crate::shapes::*;

const IRON: u32 = 0x6a7080;
const IRON_D: u32 = 0x3a3e4a;
const RUST: u32 = 0x8a5030;
const BONE: u32 = 0xe9e1cb;
const FANG: u32 = 0xf0e8cc;

pub(crate) fn goblin_dims() -> Dims {
    let dims = Dims {
        thigh: 0.2,
        shin: 0.2,
        torso: 0.3,
        hip_x: 0.075,
        shoulder_x: 0.16,
        upper: 0.19,
        fore: 0.19,
        head_r: 0.26,
        head_rise: 0.05,
        leg_r: 0.05,
        arm_r: 0.04,
        torso_rx: 0.14,
        torso_rz: 0.1,
    };
    dims.tweak(&crate::tuning::monster_key(Monster::Goblin))
}

pub(crate) fn orc_dims() -> Dims {
    let dims = Dims {
        thigh: 0.4,
        shin: 0.4,
        torso: 0.52,
        hip_x: 0.15,
        shoulder_x: 0.38,
        upper: 0.33,
        fore: 0.33,
        head_r: 0.25,
        head_rise: 0.0,
        leg_r: 0.115,
        arm_r: 0.095,
        torso_rx: 0.36,
        torso_rz: 0.25,
    };
    dims.tweak(&crate::tuning::monster_key(Monster::Orc))
}

pub(crate) fn ghoul_dims() -> Dims {
    let dims = Dims {
        thigh: 0.4,
        shin: 0.4,
        torso: 0.5,
        hip_x: 0.1,
        shoulder_x: 0.2,
        upper: 0.36,
        fore: 0.38,
        head_r: 0.2,
        head_rise: 0.06,
        leg_r: 0.052,
        arm_r: 0.04,
        torso_rx: 0.17,
        torso_rz: 0.12,
    };
    dims.tweak(&crate::tuning::monster_key(Monster::Ghoul))
}

/// Jambes, pieds griffus, bras et mains d'une créature à peau nue.
fn limbs(d: &Dims, j: &Joints, skin: u32, joint: u32, claw: u32, foot: f32) -> Vec<Part> {
    let mut p = Vec::new();
    for i in 0..2 {
        p.push(shiny(cone(j.hip[i], j.knee[i], d.leg_r, d.leg_r * 0.8, skin), 0.15));
        p.push(ball(j.knee[i], d.leg_r * 0.82, joint));
        p.push(shiny(cone(j.knee[i], j.ankle[i], d.leg_r * 0.8, d.leg_r * 0.6, skin), 0.15));
        let sole = j.ankle[i] + Vec3::new(0.0, -0.03, 0.07 * foot);
        p.push(ellipsoid(sole, Vec3::new(d.leg_r * 0.95, 0.05, 0.13 * foot), joint));
        for k in -1..=1 {
            let x = k as f32 * d.leg_r * 0.55;
            p.push(cone(sole + Vec3::new(x, -0.005, 0.11 * foot), sole + Vec3::new(x * 1.1, -0.02, 0.22 * foot), 0.022, 0.004, claw));
        }
        p.push(shiny(cone(j.shoulder[i], j.elbow[i], d.arm_r, d.arm_r * 0.8, skin), 0.15));
        p.push(ball(j.elbow[i], d.arm_r * 0.82, joint));
        p.push(shiny(cone(j.elbow[i], j.hand[i], d.arm_r * 0.8, d.arm_r * 0.6, skin), 0.15));
        p.push(ball(j.hand[i], d.arm_r * 1.05, joint));
    }
    p
}

/// Doigts crochus, de la main vers l'avant.
fn claws(hand: Vec3, size: f32, length: f32, color: u32) -> Vec<Part> {
    (-1..=1)
        .map(|k| {
            let x = k as f32 * size;
            cone(hand + Vec3::new(x, -0.01, 0.02), hand + Vec3::new(x * 1.3, -length * 0.55, 0.05 + length * 0.55), size * 0.7, size * 0.1, color)
        })
        .collect()
}

pub(crate) fn goblin(pose: Pose) -> Vec<Part> {
    let d = goblin_dims();
    let j = joints(&d, pose, None);
    let (skin, skin_d, cloth, leather) = (0x78a838, 0x58802a, 0x6a4a2a, 0x3a2414);
    let mut p = limbs(&d, &j, skin, skin_d, 0xd8d0b0, 1.0);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    // Buste maigre et un peu voûté, gilet de cuir rapiécé, ceinture et pagne en haillons.
    p.push(shiny(ellipsoid(c + Vec3::new(0.0, d.torso * 0.62, 0.0), Vec3::new(d.torso_rx, d.torso * 0.4, d.torso_rz), skin), 0.15));
    p.push(ellipsoid(c + Vec3::new(0.0, d.torso * 0.62, 0.015), Vec3::new(d.torso_rx * 1.06, d.torso * 0.34, d.torso_rz * 1.05), cloth));
    p.push(block(c + Vec3::new(0.0, d.torso * 0.9, d.torso_rz * 0.95), Vec3::new(0.025, 0.07, 0.01), Mat3::from_rotation_z(0.4), leather));
    p.push(cone(c + Vec3::new(0.0, 0.02, 0.0), c + Vec3::new(0.0, 0.1, 0.0), d.torso_rx, d.torso_rx * 0.95, leather));
    for (x, len) in [(-0.07, 0.2), (0.0, 0.26), (0.07, 0.18)] {
        let top = Vec3::new(x, c.y + 0.02, d.torso_rz * 0.9);
        p.push(panel(top, top + Vec3::new(j.sway * 1.5, -len, 0.03), 0.07, 0.01, cloth));
    }
    // Grosse tête: oreilles immenses et pointues, long nez crochu, yeux jaunes en fente, bouche large à crocs.
    let h = j.head + Vec3::new(0.0, 0.0, 0.03);
    let r = d.head_r;
    p.push(shiny(ball(h, r, skin), 0.2));
    p.push(ellipsoid(h + Vec3::new(0.0, -0.45 * r, 0.3 * r), Vec3::new(0.7, 0.45, 0.6) * r, skin));
    for sg in [-1.0f32, 1.0] {
        p.push(shiny(cone(h + Vec3::new(sg * 0.85 * r, 0.1 * r, -0.1 * r), h + Vec3::new(sg * 2.3 * r, 0.75 * r, -0.45 * r), 0.42 * r, 0.04 * r, skin), 0.2));
        p.push(cone(h + Vec3::new(sg * 0.95 * r, 0.12 * r, -0.05 * r), h + Vec3::new(sg * 2.0 * r, 0.7 * r, -0.4 * r), 0.25 * r, 0.03 * r, 0xe89aa0));
        p.push(glow(ellipsoid(h + Vec3::new(sg * 0.38 * r, 0.14 * r, 0.8 * r), Vec3::new(0.22, 0.15, 0.1) * r, 0xf0e030)));
        p.push(ellipsoid(h + Vec3::new(sg * 0.38 * r, 0.14 * r, 0.88 * r), Vec3::new(0.05, 0.12, 0.04) * r, DARK));
        p.push(block(h + Vec3::new(sg * 0.4 * r, 0.36 * r, 0.82 * r), Vec3::new(0.24, 0.035, 0.04) * r, Mat3::from_rotation_z(-sg * 0.3), skin_d));
        p.push(cone(h + Vec3::new(sg * 0.2 * r, -0.5 * r, 0.82 * r), h + Vec3::new(sg * 0.2 * r, -0.2 * r, 0.9 * r), 0.07 * r, 0.01 * r, FANG));
    }
    p.push(shiny(cone(h + Vec3::new(0.0, 0.0, 0.85 * r), h + Vec3::new(0.0, -0.32 * r, 1.55 * r), 0.2 * r, 0.04 * r, skin_d), 0.2));
    p.push(ellipsoid(h + Vec3::new(0.0, -0.5 * r, 0.9 * r), Vec3::new(0.38, 0.09, 0.1) * r, 0x3a1410));
    // Casque de fer cabossé trop petit, posé de travers.
    p.push(shiny(ellipsoid(h + Vec3::new(0.1 * r, 0.82 * r, -0.05 * r), Vec3::new(0.85, 0.42, 0.88) * r, IRON_D), 0.6));
    p.push(shiny(cone(h + Vec3::new(0.1 * r, 0.62 * r, -0.05 * r), h + Vec3::new(0.1 * r, 0.7 * r, -0.05 * r), 0.9 * r, 0.88 * r, IRON), 0.6));
    // Poignard rouillé en dents de scie dans la main droite.
    let rot = held(&j, 0, 0.35);
    let hand = j.hand[0];
    p.push(limb(hand - rot * Vec3::Y * 0.03, hand + rot * Vec3::Y * 0.05, 0.016, leather));
    p.push(block(hand + rot * Vec3::Y * 0.07, Vec3::new(0.05, 0.012, 0.015), rot, IRON_D));
    p.push(shiny(block(hand + rot * Vec3::Y * 0.2, Vec3::new(0.022, 0.13, 0.006), rot, RUST), 0.5));
    p.push(shiny(cone(hand + rot * Vec3::Y * 0.31, hand + rot * Vec3::Y * 0.4, 0.022, 0.003, RUST), 0.6));
    p.extend(claws(j.hand[1], 0.012, 0.07, 0xd8d0b0));
    p
}

pub(crate) fn orc(pose: Pose) -> Vec<Part> {
    let d = orc_dims();
    let j = joints(&d, pose, None);
    let (skin, skin_d, fur, leather, paint) = (0x5a7048, 0x3e5230, 0x6a4a2a, 0x4a2e1a, 0xa02020);
    let mut p = limbs(&d, &j, skin, skin_d, 0xc8c0a0, 1.2);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    // Torse de brute: pectoraux et ventre, harnais de cuir croisé clouté, ceinture à plaque de fer, jupe de fourrure.
    p.push(shiny(ellipsoid(c + Vec3::new(0.0, d.torso * 0.7, 0.0), Vec3::new(d.torso_rx, d.torso * 0.38, d.torso_rz), skin), 0.2));
    p.push(ellipsoid(c + Vec3::new(0.0, d.torso * 0.25, 0.03), Vec3::new(d.torso_rx * 0.85, d.torso * 0.25, d.torso_rz * 0.95), skin_d));
    for sg in [-1.0f32, 1.0] {
        p.push(block(c + Vec3::new(0.0, d.torso * 0.72, d.torso_rz * 0.92), Vec3::new(0.035, d.torso * 0.5, 0.014), Mat3::from_rotation_z(sg * 0.7), leather));
        p.push(shiny(ball(c + Vec3::new(sg * 0.1, d.torso * 0.84 + sg * 0.05, d.torso_rz + 0.03), 0.018, IRON), 0.8));
    }
    p.push(cone(c + Vec3::new(0.0, 0.0, 0.0), c + Vec3::new(0.0, 0.12, 0.0), d.torso_rx * 0.95, d.torso_rx * 0.92, leather));
    p.push(shiny(block(c + Vec3::new(0.0, 0.06, d.torso_rx * 0.8), Vec3::new(0.07, 0.06, 0.015), Mat3::IDENTITY, IRON), 0.8));
    for k in 0..7 {
        let x = (k as f32 - 3.0) * 0.095;
        let top = Vec3::new(x, c.y + 0.02, d.torso_rz * 0.85);
        p.push(panel(top, top + Vec3::new(j.sway * 1.5, -0.32 - 0.03 * (k % 2) as f32, 0.04), 0.1, 0.02, if k % 2 == 0 { fur } else { 0x3a2414 }));
    }
    // Épaulière de fer à pointes sur l'épaule gauche, brassards de fer.
    let sh = j.shoulder[1];
    p.push(shiny(ellipsoid(sh + Vec3::new(0.07, 0.05, 0.0), Vec3::new(0.16, 0.09, 0.14), IRON_D), 0.4));
    for k in -1..=1 {
        p.push(shiny(cone(sh + Vec3::new(0.12 + 0.02 * k as f32, 0.1, 0.05 * k as f32), sh + Vec3::new(0.2 + 0.02 * k as f32, 0.28, 0.07 * k as f32), 0.035, 0.005, IRON_D), 0.6));
    }
    for i in 0..2 {
        let dir = (j.hand[i] - j.elbow[i]).normalize();
        p.push(shiny(cone(j.hand[i] - dir * 0.2, j.hand[i] - dir * 0.03, d.arm_r * 1.2, d.arm_r * 1.05, IRON_D), 0.6));
    }
    // Tête: mâchoire en avant, deux défenses, sourcils lourds, petits yeux rouges, crête de cheveux noirs, peinture de guerre.
    let h = j.head + Vec3::new(0.0, 0.0, 0.05);
    let r = d.head_r;
    p.push(shiny(ellipsoid(h, Vec3::new(1.0, 0.95, 1.0) * r, skin), 0.2));
    p.push(shiny(ellipsoid(h + Vec3::new(0.0, -0.5 * r, 0.42 * r), Vec3::new(0.85, 0.5, 0.65) * r, skin), 0.2));
    p.push(block(h + Vec3::new(0.0, 0.34 * r, 0.82 * r), Vec3::new(0.62, 0.1, 0.1) * r, Mat3::IDENTITY, skin_d));
    for sg in [-1.0f32, 1.0] {
        p.push(glow(ellipsoid(h + Vec3::new(sg * 0.34 * r, 0.16 * r, 0.8 * r), Vec3::new(0.14, 0.09, 0.08) * r, 0xff4020)));
        p.push(block(h + Vec3::new(sg * 0.34 * r, 0.3 * r, 0.86 * r), Vec3::new(0.2, 0.04, 0.05) * r, Mat3::from_rotation_z(-sg * 0.35), DARK));
        p.push(shiny(cone(h + Vec3::new(sg * 0.38 * r, -0.62 * r, 0.82 * r), h + Vec3::new(sg * 0.46 * r, 0.12 * r, 1.0 * r), 0.1 * r, 0.015 * r, BONE), 0.5));
        p.push(shiny(cone(h + Vec3::new(sg * 0.95 * r, 0.05 * r, 0.0), h + Vec3::new(sg * 1.45 * r, 0.45 * r, -0.15 * r), 0.16 * r, 0.02 * r, skin), 0.2));
        p.push(block(h + Vec3::new(sg * 0.5 * r, -0.05 * r, 0.9 * r), Vec3::new(0.03, 0.2, 0.02) * r, Mat3::from_rotation_z(sg * 0.2), paint));
    }
    p.push(ellipsoid(h + Vec3::new(0.0, -0.1 * r, 1.0 * r), Vec3::new(0.2, 0.12, 0.1) * r, skin_d));
    p.push(ellipsoid(h + Vec3::new(0.0, -0.46 * r, 0.98 * r), Vec3::new(0.42, 0.07, 0.07) * r, 0x2a0e0c));
    for k in 0..5 {
        let t = k as f32 / 4.0;
        let base = h + Vec3::new(0.0, 0.9 * r - 0.15 * r * t, 0.3 * r - 0.9 * r * t);
        p.push(shiny(cone(base, base + Vec3::new(j.sway * 2.0, 0.45 * r, -0.3 * r - j.trail * 0.4), 0.2 * r, 0.04 * r, 0x16121a), 0.4));
    }
    // Grande hache à deux mains: long manche de bois, lame de fer en croissant.
    let rot = held(&j, 0, 0.2);
    let hand = j.hand[0];
    p.push(shiny(limb(hand - rot * Vec3::Y * 0.25, hand + rot * Vec3::Y * 0.65, 0.032, 0x5a3a20), 0.2));
    let top = hand + rot * Vec3::Y * 0.6;
    p.push(shiny(block(top, Vec3::new(0.05, 0.1, 0.05), rot, IRON_D), 0.6));
    p.push(shiny(block(top + rot * Vec3::new(0.0, 0.0, 0.14), Vec3::new(0.012, 0.16, 0.13), rot, IRON), 0.85));
    p.push(shiny(cone(top + rot * Vec3::new(0.0, 0.12, 0.2), top + rot * Vec3::new(0.0, 0.2, 0.3), 0.025, 0.004, IRON), 0.8));
    p.push(shiny(cone(top + rot * Vec3::new(0.0, -0.12, 0.2), top + rot * Vec3::new(0.0, -0.2, 0.3), 0.025, 0.004, IRON), 0.8));
    p.push(shiny(block(top + rot * Vec3::new(0.0, 0.0, -0.1), Vec3::new(0.012, 0.05, 0.06), rot, IRON_D), 0.6));
    p
}

pub(crate) fn ghoul(pose: Pose) -> Vec<Part> {
    let d = ghoul_dims();
    let j = joints(&d, pose, None);
    let (skin, skin_d, rag, claw) = (0x93a090, 0x6a7868, 0x4a4030, 0xd8d0b0);
    let mut p = limbs(&d, &j, skin, skin_d, claw, 1.1);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    // Buste décharné: cage thoracique saillante sous une peau tendue, ventre creux, bosse dans le dos.
    p.push(ellipsoid(c + Vec3::new(0.0, 0.05, 0.0), Vec3::new(0.14, 0.09, 0.09), skin_d));
    p.push(shiny(ellipsoid(c + Vec3::new(0.0, d.torso * 0.72, -0.01), Vec3::new(d.torso_rx, d.torso * 0.33, d.torso_rz), skin), 0.1));
    for k in 0..5 {
        let y = c.y + 0.2 + k as f32 * 0.065;
        let w = 1.0 - ((k as f32 - 2.0) * 0.12).abs();
        p.push(ellipsoid(Vec3::new(0.0, y, 0.02), Vec3::new(d.torso_rx * w * 1.02, 0.014, d.torso_rz * w * 1.02), 0xb4bcae));
    }
    p.push(ellipsoid(c + Vec3::new(0.0, 0.14, 0.02), Vec3::new(0.1, 0.07, 0.07), 0x2a2228));
    p.push(shiny(ellipsoid(Vec3::new(0.0, j.shoulder_y - 0.02, -d.torso_rz * 0.9), Vec3::new(0.2, 0.14, 0.14), skin), 0.1));
    for k in 0..4 {
        p.push(ball(Vec3::new(0.0, c.y + 0.16 + k as f32 * 0.1, -d.torso_rz * 0.95 - 0.05), 0.022, 0xb4bcae));
    }
    // Haillons du pagne, lambeaux accrochés aux hanches.
    for (x, len) in [(-0.1, 0.34), (-0.02, 0.4), (0.07, 0.3), (0.13, 0.36)] {
        let top = Vec3::new(x, c.y + 0.04, d.torso_rz * 0.85);
        p.push(panel(top, top + Vec3::new(j.sway * 2.0, -len, 0.04 + j.trail * 0.2), 0.08, 0.01, rag));
    }
    // Longues mains griffues qui pendent vers l'avant.
    for i in 0..2 {
        p.extend(claws(j.hand[i], 0.014, 0.13, claw));
    }
    // Tête projetée en avant: orbites creuses aux yeux pâles, bouche béante pleine de crocs irréguliers, langue pendante,
    // quelques mèches grises collées au crâne.
    let h = j.head + Vec3::new(0.0, -0.04, 0.1);
    let r = d.head_r;
    p.push(shiny(ellipsoid(h, Vec3::new(0.95, 1.05, 1.0) * r, skin), 0.1));
    p.push(ellipsoid(h + Vec3::new(0.0, -0.7 * r, 0.35 * r), Vec3::new(0.62, 0.34, 0.5) * r, skin_d));
    p.push(ellipsoid(h + Vec3::new(0.0, -0.62 * r, 0.7 * r), Vec3::new(0.4, 0.26, 0.12) * r, 0x2a0e14));
    for k in 0..5 {
        let x = (k as f32 - 2.0) * 0.14;
        p.push(cone(h + Vec3::new(x * r, -0.46 * r, 0.76 * r), h + Vec3::new(x * r * 1.05, -0.66 * r - 0.04 * (k % 2) as f32, 0.8 * r), 0.045 * r, 0.006 * r, FANG));
        p.push(cone(h + Vec3::new(x * r, -0.88 * r, 0.7 * r), h + Vec3::new(x * r * 1.05, -0.64 * r, 0.76 * r), 0.04 * r, 0.006 * r, FANG));
    }
    p.push(shiny(limb(h + Vec3::new(0.05 * r, -0.7 * r, 0.7 * r), h + Vec3::new(0.08 * r + j.sway * 0.3, -1.25 * r, 0.8 * r), 0.07 * r, 0x9a3a50), 0.6));
    for sg in [-1.0f32, 1.0] {
        p.push(ellipsoid(h + Vec3::new(sg * 0.38 * r, 0.12 * r, 0.78 * r), Vec3::new(0.26, 0.28, 0.14) * r, 0x1e1a22));
        p.push(glow(ball(h + Vec3::new(sg * 0.38 * r, 0.1 * r, 0.9 * r), 0.1 * r, 0xe8f070)));
        p.push(ball(h + Vec3::new(sg * 0.38 * r, 0.1 * r, 0.98 * r), 0.035 * r, DARK));
        p.push(cone(h + Vec3::new(sg * 0.95 * r, 0.1 * r, 0.0), h + Vec3::new(sg * 1.3 * r, 0.5 * r, -0.1 * r), 0.15 * r, 0.02 * r, skin));
    }
    p.push(ellipsoid(h + Vec3::new(0.0, -0.1 * r, 0.98 * r), Vec3::new(0.1, 0.07, 0.06) * r, 0x2a2228));
    for k in 0..6 {
        let a = k as f32 * TAU / 6.0 + 0.3;
        let base = h + Vec3::new(a.cos() * 0.5 * r, 0.9 * r, a.sin() * 0.5 * r - 0.1 * r);
        p.push(cone(base, base + Vec3::new(a.cos() * 0.3 * r + j.sway * 1.5, -0.9 * r, a.sin() * 0.3 * r - 0.2 * r - j.trail * 0.3), 0.05 * r, 0.008 * r, 0x9a9a98));
    }
    p
}

/// Le loup: un quadrupède gris, maigre et décharné, aux côtes saillantes, aux pattes fines et aux yeux jaunes, qui regarde
/// vers +Z. À la marche, les pattes en diagonale (avant gauche avec arrière droite) s'élancent ensemble; au repos, il
/// respire et la queue balance. `gore`: la gueule est ensanglantée et les yeux rougeoient.
pub(crate) fn wolf(pose: Pose, gore: bool) -> Vec<Part> {
    let (phase, walking) = match pose {
        Pose::Walk(p) => (p, true),
        Pose::Idle(p) => (p, false),
        Pose::Attack(p) => (p, true),
    };
    let fur = 0x6b6f78;
    let fur_d = 0x474a52;
    let rib = 0x2e3036;
    let belly = 0x8a8c94;
    let breath = if walking { 0.0 } else { 0.01 * (phase * TAU).sin() };
    let bob = if walking { 0.018 * (phase * TAU * 2.0).sin().abs() } else { breath };
    let mut p = Vec::new();
    // Corps étroit: poitrail, flancs creux, croupe osseuse.
    p.push(shiny(ellipsoid(Vec3::new(0.0, 0.47 + bob, 0.0), Vec3::new(0.115, 0.12, 0.4), fur), 0.15));
    p.push(shiny(ellipsoid(Vec3::new(0.0, 0.5 + bob, 0.24), Vec3::new(0.135, 0.155, 0.17), fur), 0.15));
    p.push(shiny(ellipsoid(Vec3::new(0.0, 0.48 + bob, -0.25), Vec3::new(0.115, 0.12, 0.15), fur), 0.15));
    p.push(ellipsoid(Vec3::new(0.0, 0.375 + bob, 0.02), Vec3::new(0.065, 0.05, 0.3), belly));
    // Crête de l'échine, côtes saillantes de chaque côté, os des hanches.
    p.push(ellipsoid(Vec3::new(0.0, 0.595 + bob, 0.02), Vec3::new(0.04, 0.03, 0.36), fur_d));
    for k in 0..6 {
        let z = 0.18 - k as f32 * 0.075;
        for sg in [-1.0f32, 1.0] {
            p.push(limb(Vec3::new(sg * 0.108, 0.55 + bob, z), Vec3::new(sg * 0.1, 0.4 + bob, z - 0.02), 0.008, rib));
        }
    }
    for sg in [-1.0f32, 1.0] {
        p.push(ball(Vec3::new(sg * 0.085, 0.555 + bob, -0.26), 0.026, fur_d));
        p.push(ball(Vec3::new(sg * 0.09, 0.6 + bob, 0.2), 0.024, fur_d));
    }
    // Cou fin et tête étroite au long museau.
    p.push(shiny(ellipsoid(Vec3::new(0.0, 0.6 + bob, 0.4), Vec3::new(0.075, 0.1, 0.15), fur), 0.15));
    let head = Vec3::new(0.0, 0.62 + bob, 0.56);
    p.push(shiny(ellipsoid(head, Vec3::new(0.09, 0.085, 0.115), fur), 0.15));
    p.push(shiny(cone(head + Vec3::new(0.0, -0.015, 0.07), head + Vec3::new(0.0, -0.035, 0.3), 0.06, 0.024, belly), 0.15));
    p.push(ball(head + Vec3::new(0.0, -0.03, 0.31), 0.024, 0x14101a));
    // Mâchoire et crocs.
    p.push(ellipsoid(head + Vec3::new(0.0, -0.07, 0.15), Vec3::new(0.045, 0.022, 0.11), if gore { 0x8a1018 } else { fur_d }));
    for sg in [-1.0f32, 1.0] {
        p.push(shiny(cone(head + Vec3::new(sg * 0.03, -0.05, 0.22), head + Vec3::new(sg * 0.03, -0.11, 0.22), 0.011, 0.002, 0xf0ecdc), 0.5));
        // Grandes oreilles dressées, intérieur rose.
        p.push(shiny(cone(head + Vec3::new(sg * 0.055, 0.06, -0.02), head + Vec3::new(sg * 0.085, 0.24, -0.05), 0.045, 0.007, fur_d), 0.15));
        p.push(cone(head + Vec3::new(sg * 0.055, 0.07, 0.0), head + Vec3::new(sg * 0.075, 0.2, -0.03), 0.024, 0.005, 0xc07a80));
        // Yeux enfoncés.
        p.push(glow(ball(head + Vec3::new(sg * 0.055, 0.03, 0.085), 0.017, if gore { 0xff3020 } else { 0xf0d040 })));
    }
    if gore {
        p.push(ellipsoid(head + Vec3::new(0.0, -0.06, 0.24), Vec3::new(0.04, 0.025, 0.05), 0xa01820));
    }
    // Pattes fines et hautes: (x, z du haut de la patte, avant ou arrière, phase). Les diagonales vont ensemble.
    for (x, z, front, offset) in [(-0.075f32, 0.24f32, true, 0.0f32), (0.075, 0.24, true, 0.5), (-0.075, -0.25, false, 0.5), (0.075, -0.25, false, 0.0)] {
        let swing = if walking { ((phase + offset) * TAU).sin() } else { 0.0 };
        let lift = if walking { ((phase + offset) * TAU).cos().max(0.0) * 0.08 } else { 0.0 };
        let top = Vec3::new(x, 0.42 + bob, z);
        let paw = Vec3::new(x, 0.03 + lift, z + swing * 0.18);
        let knee = Vec3::new(x, 0.24 + lift * 0.5, z + swing * 0.09 + if front { 0.04 } else { -0.09 });
        p.push(shiny(limb(top, knee, 0.034, fur), 0.1));
        p.push(shiny(limb(knee, paw, 0.022, fur_d), 0.1));
        p.push(ellipsoid(paw + Vec3::new(0.0, 0.0, 0.03), Vec3::new(0.03, 0.022, 0.05), fur_d));
    }
    // Queue mince et pendante, qui balance.
    let sway = if walking { 0.06 * (phase * TAU).sin() } else { 0.05 * (phase * TAU).sin() };
    let root = Vec3::new(0.0, 0.52 + bob, -0.4);
    let mid = Vec3::new(sway * 0.5, 0.48 + bob, -0.56);
    let tip = Vec3::new(sway, 0.36 + bob, -0.7);
    p.push(shiny(limb(root, mid, 0.03, fur), 0.1));
    p.push(shiny(limb(mid, tip, 0.024, fur_d), 0.1));
    p
}
