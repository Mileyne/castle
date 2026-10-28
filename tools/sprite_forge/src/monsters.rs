//! Les ennemis: un squelette guerrier rouillé aux yeux incandescents et une grande chauve-souris.

use std::f32::consts::TAU;

use glam::{Mat3, Vec3};

use crate::heroes::{Dims, Pose, joints};
use crate::render::Part;
use crate::shapes::*;

const BONE: u32 = 0xe9e1cb;
const BONE_D: u32 = 0xb4a98c;
const RUST: u32 = 0xa05a30;
const RUST_D: u32 = 0x6a3a20;
const IRON: u32 = 0x6a7080;
const SOUL: u32 = 0x9a70ff;

#[derive(Clone, Copy, Debug)]
pub enum Monster {
    Skeleton,
    Bat,
}

pub const MONSTERS: [Monster; 2] = [Monster::Skeleton, Monster::Bat];

pub(crate) fn skeleton_dims() -> Dims {
    Dims {
        thigh: 0.4,
        shin: 0.4,
        torso: 0.42,
        hip_x: 0.1,
        shoulder_x: 0.21,
        upper: 0.25,
        fore: 0.25,
        head_r: 0.23,
        head_rise: 0.08,
        leg_r: 0.04,
        arm_r: 0.035,
        torso_rx: 0.16,
        torso_rz: 0.11,
    }
}

pub(crate) fn skeleton(pose: Pose) -> Vec<Part> {
    let d = skeleton_dims();
    let j = joints(&d, pose, None);
    let mut p = Vec::new();
    for i in 0..2 {
        // Os des jambes et des bras, articulations en boules, pieds osseux.
        p.push(limb(j.hip[i], j.knee[i], 0.042, BONE));
        p.push(ball(j.knee[i], 0.058, BONE_D));
        p.push(limb(j.knee[i], j.ankle[i], 0.036, BONE));
        p.push(shiny(ellipsoid(j.ankle[i] + Vec3::new(0.0, -0.03, 0.06), Vec3::new(0.05, 0.04, 0.12), BONE_D), 0.4));
        p.push(limb(j.shoulder[i], j.elbow[i], 0.035, BONE));
        p.push(ball(j.elbow[i], 0.048, BONE_D));
        p.push(limb(j.elbow[i], j.hand[i], 0.03, BONE));
        p.push(ball(j.hand[i], 0.055, BONE_D));
    }
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    // Bassin, colonne, cage thoracique faite de côtes séparées.
    p.push(ellipsoid(c + Vec3::new(0.0, 0.03, 0.0), Vec3::new(0.15, 0.08, 0.09), BONE_D));
    p.push(limb(c + Vec3::new(0.0, 0.05, -0.04), Vec3::new(0.0, j.shoulder_y, -0.04), 0.03, BONE));
    for k in 0..5 {
        let y = c.y + 0.16 + k as f32 * 0.07;
        let w = 1.0 - ((k as f32 - 2.0) * 0.1).abs();
        p.push(shiny(ellipsoid(Vec3::new(0.0, y, 0.0), Vec3::new(d.torso_rx * w, 0.021, d.torso_rz * w), BONE), 0.4));
    }
    p.push(limb(Vec3::new(0.0, c.y + 0.14, d.torso_rz * 0.95), Vec3::new(0.0, j.shoulder_y - 0.04, d.torso_rz * 0.85), 0.022, BONE_D));
    // Épaulière rouillée à pointes, pagne et lambeaux de cape déchirée.
    p.push(shiny(ball(j.shoulder[0] + Vec3::new(-0.04, 0.04, 0.0), 0.12, RUST), 0.5));
    p.push(cone(j.shoulder[0] + Vec3::new(-0.1, 0.1, 0.0), j.shoulder[0] + Vec3::new(-0.2, 0.25, 0.0), 0.045, 0.008, RUST_D));
    p.push(panel(c + Vec3::new(0.0, 0.08, 0.13), c + Vec3::new(0.02 + j.sway, -0.3, 0.16), 0.2, 0.012, 0x6a2030));
    for (x, len) in [(-0.1, 0.5), (0.02, 0.62), (0.12, 0.45)] {
        let top = Vec3::new(x, j.shoulder_y - 0.02, -0.14);
        p.push(panel(top, top + Vec3::new(j.sway * 2.0, -len, -0.08 - j.trail * 0.6), 0.1, 0.01, 0x4a2a3a));
    }
    // Crâne: orbites sombres avec des flammes d'âme, nez, mâchoire dentée, fissure.
    let h = j.head;
    let r = d.head_r;
    p.push(shiny(ball(h, r, BONE), 0.5));
    p.push(ellipsoid(h + Vec3::new(0.0, -0.6 * r, 0.4 * r), Vec3::new(0.62, 0.34, 0.5) * r, BONE_D));
    for sg in [-1.0f32, 1.0] {
        p.push(ellipsoid(h + Vec3::new(sg * 0.38 * r, 0.05 * r, 0.82 * r), Vec3::new(0.25, 0.3, 0.14) * r, DARK));
        p.push(glow(ellipsoid(h + Vec3::new(sg * 0.38 * r, 0.02 * r, 0.93 * r), Vec3::new(0.12, 0.16, 0.06) * r, 0xff5a20)));
    }
    p.push(ellipsoid(h + Vec3::new(0.0, -0.22 * r, 0.93 * r), Vec3::new(0.1, 0.14, 0.07) * r, DARK));
    for k in 0..6 {
        let x = (k as f32 - 2.5) * 0.14;
        p.push(block(h + Vec3::new(x * r, -0.62 * r, 0.8 * r), Vec3::new(0.05, 0.07, 0.04) * r, Mat3::IDENTITY, WHITE));
    }
    p.push(block(h + Vec3::new(0.3 * r, 0.55 * r, 0.8 * r), Vec3::new(0.012, 0.18, 0.02) * r, Mat3::from_rotation_z(0.4), DARK));
    for (x, up) in [(-0.45, 0.7), (0.0, 1.0), (0.45, 0.75)] {
        let base = h + Vec3::new(x * r, 0.95 * r, 0.0);
        p.push(glow(cone(base, base + Vec3::new((x * 0.5 + j.sway * 3.0) * r, up * r, -0.2 * r), 0.22 * r, 0.02 * r, SOUL)));
    }
    // Épée rouillée ébréchée, petit bouclier rond fendu.
    let rot = Mat3::from_rotation_x(j.forearm_angle[0] * 0.5 + 0.15);
    let hand = j.hand[0];
    p.push(limb(hand - rot * Vec3::Y * 0.06, hand + rot * Vec3::Y * 0.06, 0.025, LEATHER_D));
    p.push(block(hand + rot * Vec3::Y * 0.09, Vec3::new(0.09, 0.02, 0.025), rot, RUST_D));
    p.push(shiny(block(hand + rot * Vec3::Y * 0.5, Vec3::new(0.04, 0.4, 0.012), rot, IRON), 0.6));
    p.push(block(hand + rot * Vec3::new(0.04, 0.55, 0.0), Vec3::new(0.015, 0.03, 0.014), rot, DARK));
    let center = (j.elbow[1] + j.hand[1]) / 2.0 + Vec3::new(0.06, 0.0, 0.08);
    let dir = Vec3::new(0.3, 0.0, 1.0).normalize();
    p.push(cone(center - dir * 0.02, center + dir * 0.02, 0.2, 0.2, 0x7a5030));
    p.push(cone(center + dir * 0.015, center + dir * 0.035, 0.07, 0.07, RUST));
    p.push(block(center + dir * 0.03, Vec3::new(0.012, 0.18, 0.01), Mat3::from_rotation_z(0.3), DARK));
    p
}

pub(crate) fn bat(pose: Pose) -> Vec<Part> {
    // Les ailes battent dans toutes les poses; au repos, plus lentement.
    let phase = match pose {
        Pose::Walk(p) => p,
        Pose::Idle(p) => p * 0.5,
        Pose::Attack(p) => p,
    };
    let flap = (phase * TAU).sin();
    let a = 0.25 + 0.75 * flap;
    let y0 = 0.7 + 0.05 * (phase * TAU * 2.0).sin();
    let body = 0x5a3a8a;
    let body_l = 0x9a7ac0;
    let mut p = Vec::new();
    p.push(shiny(ellipsoid(Vec3::new(0.0, y0, 0.0), Vec3::new(0.2, 0.23, 0.17), body), 0.5));
    p.push(ellipsoid(Vec3::new(0.0, y0 - 0.03, 0.09), Vec3::new(0.14, 0.17, 0.1), body_l));
    let head = Vec3::new(0.0, y0 + 0.3, 0.04);
    p.push(shiny(ellipsoid(head, Vec3::new(0.21, 0.19, 0.19), body), 0.5));
    for sg in [-1.0f32, 1.0] {
        // Grandes oreilles pointues, intérieur rose.
        p.push(cone(head + Vec3::new(sg * 0.12, 0.12, 0.0), head + Vec3::new(sg * 0.24, 0.5, -0.03), 0.1, 0.012, body));
        p.push(cone(head + Vec3::new(sg * 0.12, 0.14, 0.03), head + Vec3::new(sg * 0.22, 0.44, 0.0), 0.06, 0.01, 0xe890b0));
        // Gros yeux jaunes à fente, crocs.
        p.push(glow(ellipsoid(head + Vec3::new(sg * 0.085, 0.03, 0.17), Vec3::new(0.065, 0.085, 0.035), 0xffe040)));
        p.push(ellipsoid(head + Vec3::new(sg * 0.085, 0.02, 0.2), Vec3::new(0.014, 0.065, 0.02), DARK));
        p.push(glow(ball(head + Vec3::new(sg * 0.07, 0.07, 0.215), 0.017, 0xffffff)));
        p.push(cone(head + Vec3::new(sg * 0.05, -0.1, 0.17), head + Vec3::new(sg * 0.05, -0.2, 0.18), 0.022, 0.003, WHITE));
        // Aile: os du bras, trois doigts et membrane.
        let shoulder = Vec3::new(sg * 0.14, y0 + 0.1, -0.02);
        let wrist = shoulder + Vec3::new(sg * a.cos() * 0.42, a.sin() * 0.42, -0.04);
        p.push(limb(shoulder, wrist, 0.032, 0x3a2460));
        p.push(ball(wrist, 0.04, 0x3a2460));
        for (k, (spread, len)) in [(0.15f32, 0.6f32), (-0.45, 0.62), (-0.95, 0.5)].into_iter().enumerate() {
            let ang = a + spread * (0.6 + 0.4 * (1.0 - flap) * 0.5) - 0.15;
            let tip = wrist + Vec3::new(sg * ang.cos() * len, ang.sin() * len - 0.1, -0.1 - 0.06 * k as f32);
            p.push(limb(wrist, tip, 0.017, 0x3a2460));
            let attach = Vec3::new(sg * 0.1, y0 - 0.12 - 0.05 * k as f32, -0.02);
            p.push(panel(attach, tip, 0.3, 0.008, if k % 2 == 0 { 0x7a4aa8 } else { 0x6a3a98 }));
        }
        // Petites pattes à griffes.
        p.push(cone(Vec3::new(sg * 0.07, y0 - 0.2, 0.0), Vec3::new(sg * 0.07, y0 - 0.38, 0.04), 0.035, 0.008, 0x3a2460));
    }
    p.push(ball(head + Vec3::new(0.0, -0.01, 0.19), 0.025, 0xe890b0));
    p
}

pub fn build(monster: Monster, pose: Pose, theme: Theme) -> Vec<Part> {
    match theme {
        Theme::Manga | Theme::Medieval => build_manga(monster, pose),
        Theme::Gore => crate::monsters_gore::build(monster, pose),
    }
}

pub(crate) fn build_manga(monster: Monster, pose: Pose) -> Vec<Part> {
    match monster {
        Monster::Skeleton => skeleton(pose),
        Monster::Bat => bat(pose),
    }
}
