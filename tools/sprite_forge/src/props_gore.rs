//! Les objets version sanglante: les modèles de base, assombris et salis, avec des détails macabres en plus
//! (crânes, yeux, viscères, sang qui goutte).

use std::f32::consts::TAU;

use glam::{Mat3, Vec3};

use crate::props::{Prop, build_manga};
use crate::render::Part;
use crate::shapes::*;

const BLOOD: u32 = 0xa01820;
const BLOOD_D: u32 = 0x5a0c14;
const BONE: u32 = 0xe4dcc4;
const FLESH: u32 = 0xc05068;

/// Assombrit et désature une couleur (et teinte en rouge ce qui brille).
fn grim(part: &mut Part, bone_keys: bool) {
    let [r, g, b] = part.color;
    let lum = 0.3 * r + 0.59 * g + 0.11 * b;
    if part.glow {
        // Les lueurs deviennent rouge sang, plus claires au centre.
        part.color = [0.55 + 0.45 * lum, 0.06 + 0.35 * lum * lum, 0.05 + 0.15 * lum];
        return;
    }
    // L'or devient de l'os pour la clé, du métal terni ailleurs.
    let golden = r > 0.8 && g > 0.6 && b < 0.4;
    if golden && bone_keys {
        part.color = [0.89, 0.86, 0.77];
        return;
    }
    let keep = 0.5;
    let dark = 0.72;
    part.color = [
        ((lum + (r - lum) * keep) * dark * 1.05).min(1.0),
        ((lum + (g - lum) * keep) * dark * 0.97).min(1.0),
        ((lum + (b - lum) * keep) * dark * 0.92).min(1.0),
    ];
}

fn skull(at: Vec3, r: f32) -> Vec<Part> {
    let mut p = vec![shiny(ball(at, r, BONE), 0.5), ellipsoid(at + Vec3::new(0.0, -r * 0.6, r * 0.25), Vec3::new(r * 0.6, r * 0.35, r * 0.5), 0xc8c0a8)];
    for sg in [-1.0f32, 1.0] {
        p.push(ball(at + Vec3::new(sg * r * 0.4, r * 0.1, r * 0.8), r * 0.25, DARK));
    }
    p
}

fn drip(at: Vec3, len: f32) -> Part {
    cone(at, at - Vec3::Y * len, 0.03, 0.006, BLOOD)
}

pub fn build(prop: Prop, t: f32) -> Vec<Part> {
    let mut parts = build_manga(prop, t);
    for part in &mut parts {
        grim(part, matches!(prop, Prop::Key));
    }
    let beat = 1.0 + 0.12 * (t * TAU).sin().max(0.0);
    match prop {
        Prop::Generator => {
            // Un cœur qui bat sur l'autel, des crânes et une mare de sang.
            parts.push(ellipsoid(Vec3::new(0.0, 0.46, 0.0), Vec3::new(0.95, 0.04, 0.95), BLOOD_D));
            parts.push(shiny(ellipsoid(Vec3::new(0.0, 0.85, 0.0), Vec3::new(0.2 * beat, 0.24 * beat, 0.18 * beat), 0xb02438), 0.9));
            parts.push(glow(ball(Vec3::new(0.0, 0.88, 0.1), 0.05, 0xff6070)));
            for (x, z, a) in [(-0.4, 0.5, 0.0), (0.45, 0.45, 1.0), (0.0, -0.6, 2.0)] {
                parts.extend(skull(Vec3::new(x, 0.55, z), 0.13));
                parts.push(drip(Vec3::new(x, 0.45, z + 0.1), 0.2 + 0.05 * a));
            }
        }
        Prop::Turret => {
            parts.extend(skull(Vec3::new(0.0, 1.12, 0.0), 0.12));
            parts.push(ellipsoid(Vec3::new(0.0, 0.62, 0.38), Vec3::new(0.2, 0.2, 0.02), BLOOD_D));
            parts.push(drip(Vec3::new(0.0, 0.45, 0.4), 0.25));
            parts.push(drip(Vec3::new(0.3, 0.48, 0.34), 0.15));
        }
        Prop::Coin => {
            parts.push(ellipsoid(Vec3::new(0.0, 0.34, 0.05), Vec3::new(0.08, 0.1, 0.02), 0x3a3030));
            parts.push(ellipsoid(Vec3::new(0.08, 0.26, 0.06), Vec3::new(0.05, 0.025, 0.012), BLOOD));
        }
        Prop::Food => {
            parts.push(ellipsoid(Vec3::new(0.0, 0.04, 0.0), Vec3::new(0.5, 0.025, 0.4), BLOOD_D));
            parts.push(drip(Vec3::new(-0.2, 0.3, 0.2), 0.22));
            parts.push(shiny(limb(Vec3::new(-0.15, 0.4, 0.0), Vec3::new(-0.35, 0.3, 0.1), 0.04, FLESH), 0.8));
        }
        Prop::Lever => {
            // Le pommeau est un œil.
            let knob = Vec3::new(-0.21, 1.06, 0.09);
            parts.push(shiny(ball(knob, 0.12, 0xf0e8d8), 0.8));
            parts.push(ball(knob + Vec3::new(0.0, 0.0, 0.1), 0.06, 0xc02030));
            parts.push(ball(knob + Vec3::new(0.0, 0.0, 0.14), 0.026, DARK));
        }
        Prop::Rune => {
            parts.push(ellipsoid(Vec3::new(0.0, 0.02, 0.0), Vec3::new(1.05, 0.02, 1.0), BLOOD_D));
        }
        Prop::Mechanism => {
            for (a, b) in [(Vec3::new(-0.3, 0.5, 0.45), Vec3::new(0.2, 0.9, 0.5)), (Vec3::new(0.4, 0.4, 0.45), Vec3::new(0.1, 0.7, 0.5))] {
                parts.push(shiny(limb(a, b, 0.035, FLESH), 0.8));
            }
            parts.push(drip(Vec3::new(0.2, 0.9, 0.5), 0.3));
        }
        Prop::TrapArmed => {
            for ix in -1..=1 {
                for iz in -1..=1 {
                    let (x, z) = (ix as f32 * 0.32, iz as f32 * 0.32);
                    parts.push(cone(Vec3::new(x, 0.45, z), Vec3::new(x, 0.62, z), 0.04, 0.004, BLOOD));
                }
            }
            parts.push(ellipsoid(Vec3::new(0.0, 0.12, 0.0), Vec3::new(0.5, 0.012, 0.5), BLOOD_D));
            parts.extend(skull(Vec3::new(0.5, 0.14, 0.4), 0.07));
        }
        Prop::TrapOff => {
            parts.push(ellipsoid(Vec3::new(0.0, 0.12, 0.0), Vec3::new(0.45, 0.012, 0.45), BLOOD_D));
        }
        Prop::DoorLocked => {
            parts.extend(skull(Vec3::new(0.0, 1.62, 0.12), 0.16));
            for x in [-0.4f32, 0.4] {
                parts.push(shiny(limb(Vec3::new(x, 1.4, 0.1), Vec3::new(x * 0.8, 0.5, 0.1), 0.025, 0x6a6e78), 0.6));
            }
            parts.push(ellipsoid(Vec3::new(-0.2, 0.7, 0.14), Vec3::new(0.12, 0.14, 0.012), BLOOD));
            parts.push(drip(Vec3::new(-0.2, 0.62, 0.14), 0.3));
        }
        Prop::DoorRoyal | Prop::DoorRoyalEmblem => {
            parts.push(drip(Vec3::new(-0.3, 1.7, 0.14), 0.5));
            parts.push(drip(Vec3::new(0.2, 1.7, 0.14), 0.35));
            if matches!(prop, Prop::DoorRoyalEmblem) {
                parts.extend(skull(Vec3::new(0.0, 1.0, 0.18), 0.17));
                for sg in [-1.0f32, 1.0] {
                    parts.push(glow(ball(Vec3::new(sg * 0.07, 1.03, 0.33), 0.05, 0xff3030)));
                }
            }
        }
        // Un œil injecté de sang veille au sommet de l'arche.
        Prop::Stairs => {
            let c = Vec3::new(0.0, 1.62, -0.2);
            parts.push(shiny(ball(c + Vec3::new(0.0, 0.0, 0.1), 0.17, 0xf0e8d8), 0.6));
            parts.push(glow(ball(c + Vec3::new(0.0, 0.0, 0.24), 0.085, 0xc01020)));
            parts.push(ellipsoid(c + Vec3::new(0.0, 0.0, 0.3), Vec3::new(0.02, 0.08, 0.015), DARK));
        }
        Prop::Key | Prop::Potion => {}
        Prop::GateClosed | Prop::GateOpen | Prop::BridgeBroken | Prop::BridgeRepaired | Prop::DrawbridgeRaised | Prop::DrawbridgeLowered | Prop::CaptiveCage => {}
        // Les éléments de l'énigme gardent le même dessin dans tous les styles: leurs couleurs disent à qui ils s'adressent.
        Prop::WallRune
        | Prop::WallGearsJammed
        | Prop::WallGearsFixed
        | Prop::WallTargetEmpty
        | Prop::WallTargetMounted
        | Prop::WallTargetHit
        | Prop::Portcullis
        | Prop::Ingredient
        | Prop::GearPart
        | Prop::TargetDisc => {}
    }
    let _ = Mat3::IDENTITY;
    parts
}
