//! Les ennemis version sanglante: le squelette est éventré, la chauve-souris est un amas de chair.

use std::f32::consts::TAU;

use glam::{Mat3, Vec3};

use crate::heroes::{Pose, joints};
use crate::monsters::{Monster, bat, skeleton, skeleton_dims};
use crate::render::Part;
use crate::shapes::*;

const BLOOD: u32 = 0xa01820;
const BLOOD_D: u32 = 0x5a0c14;
const FLESH: u32 = 0xc05068;

pub fn build(monster: Monster, pose: Pose) -> Vec<Part> {
    match monster {
        Monster::Skeleton => gutted_skeleton(pose),
        Monster::Bat => fleshbat(pose),
        // Les trois autres ne changent pas de style: leur allure est déjà brutale.
        Monster::Goblin | Monster::Orc | Monster::Ghoul => crate::monsters::build_manga(monster, pose),
    }
}

fn gutted_skeleton(pose: Pose) -> Vec<Part> {
    let d = skeleton_dims();
    let j = joints(&d, pose, None);
    let mut p = skeleton(pose);
    let mut rng = Rng(77);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    // Entrailles qui pendent de la cage thoracique, en boyaux enroulés qui se balancent.
    let mut at = c + Vec3::new(0.02, 0.2, d.torso_rz * 0.9);
    for k in 0..7 {
        let next = at + Vec3::new((k as f32 * 1.7).sin() * 0.04 + j.sway, -0.06, 0.012 + j.trail * 0.1);
        p.push(shiny(limb(at, next, 0.032, if k % 2 == 0 { FLESH } else { 0xa83a4c }), 0.7));
        at = next;
    }
    p.push(shiny(ball(at, 0.045, BLOOD), 0.6));
    // Taches de sang sur les os, côtes rougies.
    for _ in 0..9 {
        let y = c.y + 0.12 + rng.next() * 0.4;
        p.push(ellipsoid(Vec3::new(rng.spread(0.14), y, d.torso_rz * 0.95), Vec3::new(0.025 + rng.next() * 0.04, 0.03, 0.012), BLOOD));
    }
    for i in 0..2 {
        p.push(ellipsoid((j.elbow[i] + j.hand[i]) / 2.0, Vec3::new(0.03, 0.06, 0.03), BLOOD_D));
    }
    // Œil globuleux qui pend d'une orbite au bout de son nerf, mâchoire dégoulinante.
    let h = j.head;
    let r = d.head_r;
    let socket = h + Vec3::new(0.38 * r, 0.0, 0.9 * r);
    p.push(limb(socket, socket + Vec3::new(0.02, -0.28, 0.12), 0.012, BLOOD));
    p.push(shiny(ball(socket + Vec3::new(0.02, -0.32, 0.14), 0.06, WHITE), 0.7));
    p.push(ball(socket + Vec3::new(0.02, -0.32, 0.2), 0.03, 0x3aa83a));
    p.push(ball(socket + Vec3::new(0.02, -0.32, 0.225), 0.014, DARK));
    for x in [-0.4f32, 0.0, 0.4] {
        p.push(cone(h + Vec3::new(x * r, -0.75 * r, 0.7 * r), h + Vec3::new(x * r, (-1.2 - 0.3 * x.abs()) * r, 0.72 * r), 0.05 * r, 0.01 * r, BLOOD));
    }
    // Yeux verts et malsains à la place des flammes violettes.
    for sg in [-1.0f32, 1.0] {
        p.push(glow(ball(h + Vec3::new(sg * 0.38 * r, 0.03 * r, 0.88 * r), 0.09 * r, 0x70ff40)));
    }
    p
}

fn fleshbat(pose: Pose) -> Vec<Part> {
    let phase = match pose {
        Pose::Walk(p) => p,
        Pose::Idle(p) => p * 0.5,
        Pose::Attack(p) => p,
    };
    let y0 = 0.7 + 0.05 * (phase * TAU * 2.0).sin();
    let mut p = bat(pose);
    let head = Vec3::new(0.0, y0 + 0.3, 0.04);
    // Plaies à vif sur le dos et le ventre, gros œil injecté qui pend sur un nerf.
    p.push(shiny(ellipsoid(Vec3::new(0.0, y0 - 0.05, 0.16), Vec3::new(0.11, 0.14, 0.05), FLESH), 0.7));
    p.push(ellipsoid(Vec3::new(0.0, y0 - 0.05, 0.19), Vec3::new(0.06, 0.09, 0.03), BLOOD));
    p.push(limb(head + Vec3::new(-0.1, 0.0, 0.17), head + Vec3::new(-0.2, -0.22, 0.24), 0.012, BLOOD));
    p.push(shiny(ball(head + Vec3::new(-0.21, -0.26, 0.26), 0.07, 0xf8f0e0), 0.7));
    p.push(ball(head + Vec3::new(-0.21, -0.26, 0.32), 0.035, 0xc02030));
    p.push(ball(head + Vec3::new(-0.21, -0.26, 0.345), 0.015, DARK));
    // Crocs en rang serré et gueule sanglante.
    p.push(ellipsoid(head + Vec3::new(0.0, -0.1, 0.17), Vec3::new(0.1, 0.045, 0.04), 0x4a0810));
    for k in 0..5 {
        let x = (k as f32 - 2.0) * 0.04;
        p.push(cone(head + Vec3::new(x, -0.1, 0.19), head + Vec3::new(x, -0.19 - 0.02 * (k % 2) as f32, 0.2), 0.016, 0.003, WHITE));
    }
    p.push(cone(head + Vec3::new(0.06, -0.15, 0.19), head + Vec3::new(0.06, -0.38, 0.2), 0.02, 0.005, BLOOD));
    // Les ailes saignent près du corps.
    for sg in [-1.0f32, 1.0] {
        p.push(panel(Vec3::new(sg * 0.1, y0 - 0.1, -0.03), Vec3::new(sg * 0.32, y0 - 0.32, -0.08), 0.18, 0.01, BLOOD_D));
        p.push(cone(Vec3::new(sg * 0.34, y0 - 0.3, -0.08), Vec3::new(sg * 0.34, y0 - 0.5, -0.08), 0.02, 0.005, BLOOD));
    }
    let _ = Mat3::IDENTITY;
    p
}
