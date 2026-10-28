//! Les quatre héros, dans un esprit manga: grande tête et grands yeux, membres effilés, taille marquée,
//! mèches de cheveux, écharpes et capes qui flottent, avec un squelette animé (marche, repos).
//!
//! Le modèle regarde vers +Z (vers le spectateur), le sol est à y = 0, l'unité est le mètre.
//! Le côté droit du personnage est à -X: l'arme principale y est toujours tenue, quelle que soit la
//! direction de vue, car c'est la caméra qui tourne autour du modèle (et non un miroir).

use std::f32::consts::TAU;

use glam::{Mat3, Vec3};

use crate::render::{Part, Shape};
use crate::shapes::*;

#[derive(Clone, Copy, Debug)]
pub enum Class {
    Knight,
    Sorcerer,
    Ranger,
    Forgebraise,
}

pub const CLASSES: [Class; 4] = [Class::Knight, Class::Sorcerer, Class::Ranger, Class::Forgebraise];

/// Pose d'une image: marche (phase du cycle, de 0 à 1) ou repos (phase de la respiration).
#[derive(Clone, Copy)]
pub enum Pose {
    Walk(f32),
    Idle(f32),
    /// Coup avec l'arme: progression de 0 à 1 (préparation, élan, frappe, retour).
    Attack(f32),
}

pub(crate) struct Dims {
    pub thigh: f32,
    pub shin: f32,
    pub torso: f32,
    pub hip_x: f32,
    pub shoulder_x: f32,
    pub upper: f32,
    pub fore: f32,
    pub head_r: f32,
    pub head_rise: f32,
    pub leg_r: f32,
    pub arm_r: f32,
    pub torso_rx: f32,
    pub torso_rz: f32,
}

impl Dims {
    fn of(class: Class) -> Self {
        match class {
            Class::Knight => Dims { thigh: 0.45, shin: 0.45, torso: 0.5, hip_x: 0.12, shoulder_x: 0.3, upper: 0.29, fore: 0.29, head_r: 0.245, head_rise: 0.12, leg_r: 0.092, arm_r: 0.07, torso_rx: 0.26, torso_rz: 0.17 },
            Class::Sorcerer => Dims { thigh: 0.44, shin: 0.44, torso: 0.46, hip_x: 0.1, shoulder_x: 0.23, upper: 0.27, fore: 0.27, head_r: 0.265, head_rise: 0.12, leg_r: 0.072, arm_r: 0.055, torso_rx: 0.19, torso_rz: 0.13 },
            Class::Ranger => Dims { thigh: 0.46, shin: 0.46, torso: 0.5, hip_x: 0.115, shoulder_x: 0.27, upper: 0.29, fore: 0.29, head_r: 0.245, head_rise: 0.12, leg_r: 0.082, arm_r: 0.062, torso_rx: 0.23, torso_rz: 0.15 },
            Class::Forgebraise => Dims { thigh: 0.3, shin: 0.3, torso: 0.44, hip_x: 0.15, shoulder_x: 0.36, upper: 0.25, fore: 0.25, head_r: 0.29, head_rise: 0.1, leg_r: 0.115, arm_r: 0.09, torso_rx: 0.35, torso_rz: 0.24 },
        }
    }
}

/// Effets visuels d'une pose d'attaque (traînée de lame, cercle magique, corde tendue, onde de choc).
#[derive(Default)]
pub(crate) struct Fx {
    /// Multiplicateur de taille de l'orbe du bâton.
    pub glow: f32,
    /// Cercle magique: centre, axe, rayon.
    pub ring: Option<(Vec3, Vec3, f32)>,
    /// Traînée de l'arme: segments (début, fin, rayon).
    pub trail: Vec<(Vec3, Vec3, f32)>,
    /// Flèche encochée (talon, pointe) et point où la corde est tirée.
    pub arrow: Option<(Vec3, Vec3)>,
    pub string: Option<Vec3>,
    /// Onde de choc au sol: centre et rayon.
    pub shock: Option<(Vec3, f32)>,
}

impl Fx {
    fn shifted(mut self, by: Vec3) -> Self {
        self.ring = self.ring.map(|(c, n, r)| (c + by, n, r));
        for seg in &mut self.trail {
            seg.0 += by;
            seg.1 += by;
        }
        self.arrow = self.arrow.map(|(a, b)| (a + by, b + by));
        self.string = self.string.map(|p| p + by);
        self.shock = self.shock.map(|(c, r)| (c + by, r));
        self
    }
}

/// Pose d'une attaque: jambes, mains cibles des deux bras et orientation de l'arme.
struct Plan {
    alpha: [f32; 2],
    beta: [f32; 2],
    right_hand: Vec3,
    left_hand: Vec3,
    weapon_dir: Vec3,
    fx: Fx,
}

/// Interpolation (avec adoucissement) entre poses clés: (instant, position de la main relative à l'épaule, axe de l'arme).
fn sample(keys: &[(f32, Vec3, Vec3)], t: f32) -> (Vec3, Vec3) {
    let t = t.clamp(0.0, 1.0);
    for w in keys.windows(2) {
        if t <= w[1].0 {
            let k = ((t - w[0].0) / (w[1].0 - w[0].0)).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            return (w[0].1.lerp(w[1].1, k), w[0].2.lerp(w[1].2, k).normalize());
        }
    }
    let last = keys[keys.len() - 1];
    (last.1, last.2.normalize())
}

/// Segments de la traînée laissée par le bout de l'arme sur les instants qui précèdent `t`.
fn trail(keys: &[(f32, Vec3, Vec3)], shoulder: Vec3, length: f32, t: f32) -> Vec<(Vec3, Vec3, f32)> {
    let tip = |at: f32| {
        let (off, dir) = sample(keys, at);
        shoulder + off + dir * length
    };
    let mut segments = Vec::new();
    if !(0.4..=0.85).contains(&t) {
        return segments;
    }
    for k in 0..5 {
        let (t0, t1) = (t - 0.05 * k as f32, t - 0.05 * (k + 1) as f32);
        if t1 < 0.3 {
            break;
        }
        segments.push((tip(t0), tip(t1), 0.045 - 0.008 * k as f32));
    }
    segments
}

fn attack_plan(class: Class, d: &Dims, t: f32, hip_y: f32) -> Plan {
    let sy = hip_y + d.torso;
    let sr = Vec3::new(-d.shoulder_x, sy, 0.0);
    let sl = Vec3::new(d.shoulder_x, sy, 0.0);
    let bump = (t * std::f32::consts::PI).sin();
    let rest_r = Vec3::new(-0.03, -0.52, 0.12);
    let rest_l = Vec3::new(0.03, -0.52, 0.12);
    let up = Vec3::new(0.0, 1.0, 0.1);
    let mut fx = Fx { glow: 1.0, ..Default::default() };
    match class {
        Class::Knight => {
            // Coup d'épée: l'épée monte derrière la tête, puis s'abat vers l'avant et en bas.
            let keys = [
                (0.0, rest_r, up),
                (0.25, Vec3::new(-0.05, 0.3, -0.25), Vec3::new(0.0, 1.0, -0.6)),
                (0.42, Vec3::new(-0.02, 0.44, -0.1), Vec3::new(0.0, 1.0, -0.2)),
                (0.58, Vec3::new(0.0, 0.1, 0.5), Vec3::new(0.0, 0.1, 1.0)),
                (0.75, Vec3::new(0.05, -0.3, 0.42), Vec3::new(0.0, -0.7, 0.7)),
                (1.0, rest_r, up),
            ];
            let (off, dir) = sample(&keys, t);
            fx.trail = trail(&keys, sr, 1.15, t);
            Plan {
                alpha: [0.35 * bump, -0.25 * bump],
                beta: [0.15 * bump, 0.3 * bump],
                right_hand: sr + off,
                left_hand: sl + rest_l.lerp(Vec3::new(0.02, -0.2, 0.36), (bump * 2.0).min(1.0)),
                weapon_dir: dir,
                fx,
            }
        }
        Class::Sorcerer => {
            // Sort: le bâton est ramené en arrière, puis pointé vers l'avant; l'autre main s'ouvre.
            let keys = [
                (0.0, rest_r, up),
                (0.25, Vec3::new(-0.02, -0.1, -0.05), Vec3::new(0.0, 1.0, -0.55)),
                (0.42, Vec3::new(-0.06, 0.0, 0.3), Vec3::new(0.0, 0.8, 0.6)),
                (0.58, Vec3::new(-0.08, 0.05, 0.52), Vec3::new(0.0, 0.5, 0.85)),
                (0.75, Vec3::new(-0.08, 0.05, 0.52), Vec3::new(0.0, 0.55, 0.85)),
                (1.0, rest_r, up),
            ];
            let (off, dir) = sample(&keys, t);
            let right = sr + off;
            let strength = (1.0 - ((t - 0.58) / 0.3).abs()).clamp(0.0, 1.0);
            fx.glow = 1.0 + 0.9 * strength;
            if (0.45..=0.85).contains(&t) {
                fx.ring = Some((right + dir * 1.6, dir, 0.1 + 0.34 * strength));
            }
            Plan {
                alpha: [0.15 * bump, -0.2 * bump],
                beta: [0.1 * bump, 0.25 * bump],
                right_hand: right,
                left_hand: sl + rest_l.lerp(Vec3::new(0.14, 0.12, 0.36), (bump * 2.0).min(1.0)),
                weapon_dir: dir,
                fx,
            }
        }
        Class::Ranger => {
            // Tir: bras de l'arc tendu devant, l'autre main tire la corde vers la joue, puis lâche.
            let keys = [
                (0.0, rest_r, up),
                (0.25, Vec3::new(-0.05, 0.12, 0.5), Vec3::new(0.0, 1.0, 0.05)),
                (0.58, Vec3::new(-0.05, 0.12, 0.52), Vec3::new(0.0, 1.0, 0.05)),
                (0.75, Vec3::new(-0.05, 0.1, 0.48), Vec3::new(0.0, 1.0, 0.0)),
                (1.0, rest_r, up),
            ];
            let (off, dir) = sample(&keys, t);
            let right = sr + off;
            let grip = right + Vec3::new(0.03, 0.0, 0.0);
            let cheek = Vec3::new(-0.1, sy + 0.06, 0.1);
            let rest_left = sl + rest_l;
            let draw = ((t - 0.12) / 0.4).clamp(0.0, 1.0);
            let draw = draw * draw * (3.0 - 2.0 * draw);
            let left = if t < 0.58 {
                rest_left.lerp(grip, (t / 0.12).min(1.0)).lerp(cheek, draw)
            } else if t < 0.75 {
                cheek.lerp(Vec3::new(0.28, sy - 0.1, -0.05), (t - 0.58) / 0.17)
            } else {
                Vec3::new(0.28, sy - 0.1, -0.05).lerp(rest_left, (t - 0.75) / 0.25)
            };
            if (0.12..0.58).contains(&t) {
                fx.string = Some(left);
            }
            if (0.2..0.58).contains(&t) {
                fx.arrow = Some((left, grip + Vec3::Z * 0.3));
            }
            Plan {
                alpha: [0.2 * bump, -0.2 * bump],
                beta: [0.2 * bump, 0.2 * bump],
                right_hand: right,
                left_hand: left,
                weapon_dir: dir,
                fx,
            }
        }
        Class::Forgebraise => {
            // Coup de marteau à deux mains: le marteau monte derrière la tête, puis s'écrase devant, onde de choc au sol.
            let keys = [
                (0.0, rest_r, up),
                (0.25, Vec3::new(-0.1, 0.52, -0.2), Vec3::new(0.0, 1.0, -0.9)),
                (0.42, Vec3::new(-0.05, 0.45, 0.0), Vec3::new(0.0, 1.0, -0.2)),
                (0.58, Vec3::new(-0.05, -0.2, 0.5), Vec3::new(0.0, -0.35, 1.0)),
                (0.75, Vec3::new(-0.05, -0.35, 0.45), Vec3::new(0.0, -0.5, 1.0)),
                (1.0, rest_r, up),
            ];
            let (off, dir) = sample(&keys, t);
            let right = sr + off;
            fx.trail = trail(&keys, sr, 0.7, t);
            if (0.56..=0.92).contains(&t) {
                let (impact_off, impact_dir) = sample(&keys, 0.6);
                let head = sr + impact_off + impact_dir * 0.7;
                fx.shock = Some((Vec3::new(head.x, 0.05, head.z), 0.25 + (t - 0.56) * 2.2));
            }
            Plan {
                alpha: [0.3 * bump, -0.3 * bump],
                beta: [0.45 * bump, 0.45 * bump],
                right_hand: right,
                left_hand: right + Vec3::new(0.15, -0.12, 0.0),
                weapon_dir: dir,
                fx,
            }
        }
    }
}

/// Cinématique inverse à deux segments: place le coude pour que la main atteigne `target` (ou s'en approche
/// autant que les os le permettent); `pole` indique de quel côté le coude plie.
fn two_bone(shoulder: Vec3, target: Vec3, upper: f32, fore: f32, pole: Vec3) -> (Vec3, Vec3) {
    let to = target - shoulder;
    let dist = to.length().clamp(0.05, upper + fore - 0.002);
    let dir = to.normalize_or(Vec3::NEG_Y);
    let hand = shoulder + dir * dist;
    let cos_a = ((upper * upper + dist * dist - fore * fore) / (2.0 * upper * dist)).clamp(-1.0, 1.0);
    let (cos_a, sin_a) = (cos_a, (1.0 - cos_a * cos_a).max(0.0).sqrt());
    let perp = (pole - dir * pole.dot(dir)).normalize_or(Vec3::X);
    (shoulder + dir * (upper * cos_a) + perp * (upper * sin_a), hand)
}

/// Articulations; l'indice 0 est le côté droit du personnage (-X), l'indice 1 le gauche (+X).
pub(crate) struct Joints {
    pub hip: [Vec3; 2],
    pub knee: [Vec3; 2],
    pub ankle: [Vec3; 2],
    pub shoulder: [Vec3; 2],
    pub elbow: [Vec3; 2],
    pub hand: [Vec3; 2],
    pub forearm_angle: [f32; 2],
    pub hip_y: f32,
    pub shoulder_y: f32,
    pub head: Vec3,
    pub breath: f32,
    /// Balancement des éléments souples (cheveux, capes, écharpes) et leur traînée vers l'arrière.
    pub sway: f32,
    pub trail: f32,
    /// Orientation imposée à l'arme de la main droite pendant une attaque, et effets visuels de l'attaque.
    pub weapon: Option<Mat3>,
    pub fx: Fx,
}

fn dir_in_yz(angle: f32, side_out: f32) -> Vec3 {
    Vec3::new(side_out, -angle.cos(), angle.sin()).normalize()
}

pub(crate) fn joints(d: &Dims, pose: Pose, class: Option<Class>) -> Joints {
    let sides = [-1.0f32, 1.0];
    let (s, c, breath) = match pose {
        Pose::Walk(phase) => ((phase * TAU).sin(), (phase * TAU).cos(), 0.0),
        Pose::Idle(phase) => (0.0, 0.0, phase),
        Pose::Attack(_) => (0.0, 0.0, 0.0),
    };
    let walking = matches!(pose, Pose::Walk(_));
    let hip_y = d.thigh + d.shin + 0.05;
    let plan = match (pose, class) {
        (Pose::Attack(t), Some(class)) => Some(attack_plan(class, d, t, hip_y)),
        _ => None,
    };

    let (mut hip, mut knee, mut ankle) = ([Vec3::ZERO; 2], [Vec3::ZERO; 2], [Vec3::ZERO; 2]);
    let (mut shoulder, mut elbow, mut hand) = ([Vec3::ZERO; 2], [Vec3::ZERO; 2], [Vec3::ZERO; 2]);
    let mut forearm_angle = [0.0f32; 2];

    for i in 0..2 {
        let sg = sides[i];
        // Les deux jambes (et les deux bras) sont en opposition de phase.
        let phase_sign = if i == 0 { 1.0 } else { -1.0 };
        let (alpha, beta, gamma, delta) = if let Some(plan) = &plan {
            (plan.alpha[i], plan.beta[i], 0.0, 0.25)
        } else if walking {
            let swing = phase_sign * s;
            let lift = (phase_sign * c).max(0.0);
            (0.62 * swing, 1.0 * lift * lift, -0.6 * swing, 0.3 + 0.3 * (-swing).max(0.0))
        } else {
            (0.0, 0.0, 0.04 * (breath * TAU).sin() * phase_sign, 0.25)
        };
        hip[i] = Vec3::new(sg * d.hip_x, hip_y, 0.0);
        knee[i] = hip[i] + dir_in_yz(alpha, 0.0) * d.thigh;
        ankle[i] = knee[i] + dir_in_yz(alpha - beta, 0.0) * d.shin;
        shoulder[i] = Vec3::new(sg * d.shoulder_x, hip_y + d.torso, 0.0);
        if let Some(plan) = &plan {
            // Attaque: la main va où la pose l'exige, le coude suit (cinématique inverse).
            let target = if i == 0 { plan.right_hand } else { plan.left_hand };
            let (e, h) = two_bone(shoulder[i], target, d.upper, d.fore, Vec3::new(sg, -0.4, -0.35));
            elbow[i] = e;
            hand[i] = h;
            let fore = h - e;
            forearm_angle[i] = fore.z.atan2(-fore.y);
        } else {
            elbow[i] = shoulder[i] + dir_in_yz(gamma, sg * 0.1) * d.upper;
            forearm_angle[i] = gamma + delta;
            hand[i] = elbow[i] + dir_in_yz(forearm_angle[i], sg * 0.05) * d.fore;
        }
    }

    // Le pied le plus bas touche le sol: tout le corps est décalé en conséquence.
    let lowest = ankle[0].y.min(ankle[1].y);
    let bob = if matches!(pose, Pose::Idle(_)) { 0.01 * (breath * TAU).sin() } else { 0.0 };
    let shift = Vec3::Y * (0.05 - lowest + bob);
    for arr in [&mut hip, &mut knee, &mut ankle, &mut shoulder, &mut elbow, &mut hand] {
        for p in arr.iter_mut() {
            *p += shift;
        }
    }
    let shoulder_y = hip_y + d.torso + shift.y;
    let (sway, trail) = match pose {
        Pose::Walk(phase) => ((phase * TAU * 2.0).sin() * 0.06, 0.13),
        Pose::Idle(phase) => ((phase * TAU).sin() * 0.03, 0.0),
        // Pendant le coup, cheveux et capes sont entraînés vers l'arrière.
        Pose::Attack(t) => (0.04, 0.08 + 0.12 * (t * std::f32::consts::PI).sin()),
    };
    let (weapon, fx) = match plan {
        Some(plan) => (Some(align_y(plan.weapon_dir)), plan.fx.shifted(shift)),
        None => (None, Fx { glow: 1.0, ..Default::default() }),
    };
    Joints {
        hip,
        knee,
        ankle,
        shoulder,
        elbow,
        hand,
        forearm_angle,
        hip_y: hip_y + shift.y,
        shoulder_y,
        head: Vec3::new(0.0, shoulder_y + d.head_rise + d.head_r * 0.6, 0.0),
        breath,
        sway,
        trail,
        weapon,
        fx,
    }
}


pub(crate) struct Palette {
    pub pants: u32,
    pub boots: u32,
    pub torso: u32,
    pub waist: u32,
    pub sleeve: u32,
    pub glove: u32,
}

/// Corps commun: jambes effilées, bottes, buste à taille marquée, bras effilés et mains. Sans tête.
pub(crate) fn body(d: &Dims, j: &Joints, pal: &Palette, legs: bool) -> Vec<Part> {
    let mut parts = Vec::new();
    let breath = 1.0 + 0.018 * (j.breath * TAU).sin();
    for i in 0..2 {
        if legs {
            parts.push(cone(j.hip[i], j.knee[i], d.leg_r, d.leg_r * 0.8, pal.pants));
            parts.push(ball(j.knee[i], d.leg_r * 0.82, pal.pants));
            parts.push(cone(j.knee[i], j.ankle[i], d.leg_r * 0.8, d.leg_r * 0.58, pal.pants));
        }
        // Botte pointue: une forme allongée vers l'avant, inclinée avec la jambe.
        let toe = (j.ankle[i] - j.knee[i]).normalize();
        let pitch = Mat3::from_rotation_x(-toe.z * 0.6);
        parts.push(mk(
            Shape::Sphere,
            j.ankle[i] + Vec3::new(0.0, -0.025, 0.07),
            pitch,
            Vec3::new(d.leg_r * 0.78, 0.065, 0.16),
            pal.boots,
        ));
        parts.push(cone(j.ankle[i] - Vec3::Y * 0.03, j.ankle[i] + Vec3::Y * 0.17, d.leg_r * 0.66, d.leg_r * 0.78, pal.boots));
        // Bras effilés, coude arrondi.
        parts.push(cone(j.shoulder[i], j.elbow[i], d.arm_r, d.arm_r * 0.8, pal.sleeve));
        parts.push(ball(j.elbow[i], d.arm_r * 0.78, pal.sleeve));
        parts.push(cone(j.elbow[i], j.hand[i], d.arm_r * 0.8, d.arm_r * 0.62, pal.sleeve));
        parts.push(ball(j.hand[i], d.arm_r * 0.95, pal.glove));
    }
    // Bassin, taille fine, buste.
    parts.push(ellipsoid(Vec3::new(0.0, j.hip_y + 0.03, 0.0), Vec3::new(d.torso_rx * 0.88, 0.13, d.torso_rz * 0.92), pal.waist));
    parts.push(ellipsoid(Vec3::new(0.0, j.hip_y + 0.2, 0.0), Vec3::new(d.torso_rx * 0.7, 0.17, d.torso_rz * 0.78), pal.waist));
    parts.push(ellipsoid(
        Vec3::new(0.0, j.hip_y + d.torso * 0.72, 0.0),
        Vec3::new(d.torso_rx, d.torso * 0.34 * breath, d.torso_rz),
        pal.torso,
    ));
    parts.push(cone(Vec3::new(0.0, j.shoulder_y - 0.02, 0.0), j.head - Vec3::Y * d.head_r * 0.7, 0.065, 0.055, SKIN));
    parts
}

/// Visage manga: grands yeux brillants avec iris coloré et reflet, sourcils, nez, bouche, menton pointu.
/// `mood` règle l'expression: de -1 (sérieux, sourcils froncés) à 1 (grand sourire).
fn face(h: Vec3, r: f32, eye: u32, brow: u32, mood: f32) -> Vec<Part> {
    let at = |x: f32, y: f32, z: f32| h + Vec3::new(x * r, y * r, z * r);
    let mut p = Vec::new();
    // Menton un peu pointu sous le crâne.
    p.push(ellipsoid(at(0.0, -0.5, 0.48), Vec3::new(0.66, 0.52, 0.56) * r, SKIN));
    for sg in [-1.0f32, 1.0] {
        // Œil: blanc, grand iris, pupille, reflet brillant, cil supérieur marqué.
        p.push(ellipsoid(at(sg * 0.41, -0.03, 0.83), Vec3::new(0.27, 0.31, 0.13) * r, 0xfafaff));
        p.push(ellipsoid(at(sg * 0.41, -0.05, 0.915), Vec3::new(0.19, 0.265, 0.09) * r, eye));
        p.push(ellipsoid(at(sg * 0.41, -0.11, 0.975), Vec3::new(0.1, 0.14, 0.05) * r, DARK));
        p.push(glow(ball(at(sg * 0.35, 0.07, 1.0), 0.052 * r, 0xffffff)));
        p.push(glow(ball(at(sg * 0.47, -0.16, 0.99), 0.026 * r, 0xffffff)));
        p.push(ellipsoid(at(sg * 0.41, 0.24, 0.84), Vec3::new(0.32, 0.045, 0.1) * r, DARK));
        // Sourcil incliné selon l'humeur.
        let tilt = -sg * (0.18 - mood * 0.12);
        p.push(block(at(sg * 0.43, 0.42, 0.86), Vec3::new(0.2, 0.025, 0.035) * r, Mat3::from_rotation_z(tilt), brow));
        // Joue rosée.
        p.push(ellipsoid(at(sg * 0.6, -0.3, 0.74), Vec3::new(0.15, 0.07, 0.05) * r, 0xf59a9a));
    }
    p.push(ball(at(0.0, -0.2, 1.0), 0.045 * r, SKIN_D));
    // Bouche: simple trait, ou sourire ouvert avec les dents.
    if mood > 0.5 {
        p.push(ellipsoid(at(0.0, -0.45, 0.88), Vec3::new(0.22, 0.1, 0.06) * r, 0x7a1c28));
        p.push(block(at(0.0, -0.4, 0.93), Vec3::new(0.18, 0.025, 0.03) * r, Mat3::IDENTITY, WHITE));
    } else {
        p.push(block(at(0.0, -0.45, 0.9), Vec3::new(0.1, 0.014, 0.025) * r, Mat3::IDENTITY, 0x8a3a3a));
    }
    p
}

/// Rotation d'une arme tenue en main: elle suit un peu l'avant-bras, inclinée vers l'avant.
pub(crate) fn held(j: &Joints, side: usize, tilt: f32) -> Mat3 {
    match (side, j.weapon) {
        (0, Some(rot)) => rot,
        _ => Mat3::from_rotation_x(j.forearm_angle[side] * 0.5 + tilt),
    }
}

fn knight(d: &Dims, j: &Joints) -> Vec<Part> {
    let pal = Palette { pants: BLUE_D, boots: LEATHER_D, torso: STEEL, waist: BLUE_D, sleeve: BLUE_D, glove: STEEL_D };
    let mut p = body(d, j, &pal, true);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    let (sway, trail) = (j.sway, j.trail);

    // Cuirasse: arête dorée, jupe de plates (tassets), gorgerin, jambières et genouillères.
    p.push(shiny(ellipsoid(c + Vec3::new(0.0, d.torso * 0.72, 0.0), Vec3::new(d.torso_rx * 1.05, d.torso * 0.3, d.torso_rz * 1.06), STEEL), 0.7));
    p.push(block(c + Vec3::new(0.0, d.torso * 0.74, d.torso_rz * 1.0), Vec3::new(0.014, 0.16, 0.014), Mat3::IDENTITY, GOLD));
    p.push(cone(c + Vec3::new(0.0, -0.1, 0.0), c + Vec3::new(0.0, 0.14, 0.0), 0.34, 0.2, STEEL_D));
    p.push(cone(c + Vec3::new(0.0, -0.1, 0.0), c + Vec3::new(0.0, -0.06, 0.0), 0.345, 0.34, GOLD));
    p.push(cone(c + Vec3::new(0.0, d.torso * 0.98, 0.0), c + Vec3::new(0.0, d.torso * 1.06, 0.0), 0.15, 0.11, STEEL));
    for i in 0..2 {
        p.push(shiny(cone(j.knee[i], j.ankle[i] + Vec3::Y * 0.02, d.leg_r * 0.86, d.leg_r * 0.66, STEEL), 0.7));
        p.push(shiny(ball(j.knee[i] + Vec3::new(0.0, 0.0, 0.05), d.leg_r * 0.9, STEEL), 0.7));
        // Épaulière à pointe, brassard, gantelet.
        let out = if i == 0 { -1.0 } else { 1.0 };
        p.push(shiny(ball(j.shoulder[i] + Vec3::new(out * 0.05, 0.04, 0.0), 0.135, STEEL), 0.8));
        p.push(cone(j.shoulder[i] + Vec3::new(out * 0.1, 0.1, 0.0), j.shoulder[i] + Vec3::new(out * 0.22, 0.24, -0.02), 0.05, 0.01, STEEL));
        p.push(cone(j.elbow[i], j.hand[i] - (j.hand[i] - j.elbow[i]).normalize() * 0.04, d.arm_r * 0.95, d.arm_r * 0.8, STEEL));
        p.push(shiny(ball(j.hand[i], d.arm_r * 1.15, STEEL_D), 0.6));
    }
    // Longue cape rouge qui flotte, nouée aux épaules.
    for (x, w, len, k) in [(-0.17, 0.2, 0.62, 0.0), (0.0, 0.18, 0.75, 1.0), (0.17, 0.2, 0.6, 2.0)] {
        let top = Vec3::new(x, j.shoulder_y - 0.03, -d.torso_rz - 0.04);
        let swing = sway * (1.0 + k * 0.4) + k * 0.01;
        let bottom = top + Vec3::new(swing, -len, -0.1 - trail * (0.6 + 0.5 * (len)) );
        p.push(panel(top, bottom, w, 0.012, if k == 1.0 { RED_D } else { RED }));
    }
    p.push(ball(j.shoulder[0] + Vec3::new(0.1, 0.02, -0.06), 0.045, GOLD));
    p.push(ball(j.shoulder[1] + Vec3::new(-0.1, 0.02, -0.06), 0.045, GOLD));

    // Tête: visage sérieux et déterminé, cheveux blonds en mèches, diadème ailé.
    let h = j.head;
    let r = d.head_r;
    p.extend(face(h, r, 0x3a8cf0, 0xc8a040, -0.4));
    p.push(shiny(ball(h, r, SKIN), 0.0));
    let hair = 0xf4d878;
    p.push(shiny(ellipsoid(h + Vec3::new(0.0, 0.2 * r, -0.24 * r), Vec3::new(1.06, 1.0, 1.03) * r, hair), 0.9));
    let surf = |x: f32, y: f32| {
        let z = (1.0 - x * x - y * y).max(0.05).sqrt() * 1.03 + 0.04;
        h + Vec3::new(x * r, y * r, z * r)
    };
    for (x, ty) in [(-0.72, 0.28), (-0.36, 0.4), (0.0, 0.32), (0.36, 0.4), (0.72, 0.28)] {
        p.push(lock(h + Vec3::new(x * r * 0.95, 0.98 * r, 0.3 * r), surf(x * 1.05, ty), 0.2 * r, hair));
    }
    for sg in [-1.0f32, 1.0] {
        p.push(lock(h + Vec3::new(sg * 0.95 * r, 0.45 * r, 0.05 * r), h + Vec3::new(sg * 1.05 * r, -0.7 * r, 0.4 * r), 0.2 * r, hair));
        p.push(lock(
            h + Vec3::new(sg * 0.5 * r, 0.9 * r, -0.7 * r),
            h + Vec3::new(sg * (0.9 + sway * 3.0) * r, 0.25 * r, (-1.9 - trail * 3.0) * r),
            0.3 * r,
            hair,
        ));
    }
    p.push(lock(h + Vec3::new(0.0, 0.8 * r, -0.8 * r), h + Vec3::new(sway * 3.0 * r, -0.2 * r, (-2.1 - trail * 3.0) * r), 0.34 * r, hair));
    // Diadème d'acier avec gemme rouge et petites ailes.
    p.push(shiny(cone(h + Vec3::Y * 0.56 * r, h + Vec3::Y * 0.7 * r, 0.9 * r, 0.82 * r, STEEL), 0.8));
    p.push(glow(ball(surf(0.0, 0.64) + Vec3::Z * 0.02, 0.09 * r, 0xff4060)));
    for sg in [-1.0f32, 1.0] {
        p.push(cone(h + Vec3::new(sg * 0.88 * r, 0.66 * r, 0.0), h + Vec3::new(sg * 1.75 * r, 1.1 * r, -0.35 * r), 0.16 * r, 0.02 * r, WHITE));
        p.push(cone(h + Vec3::new(sg * 0.88 * r, 0.6 * r, 0.0), h + Vec3::new(sg * 1.55 * r, 0.62 * r, -0.4 * r), 0.12 * r, 0.02 * r, 0xdcdcf0));
    }

    // Épée longue dans la main droite, petit bouclier rond au bras gauche.
    let rot = held(j, 0, 0.1);
    let hand = j.hand[0];
    p.push(limb(hand - rot * Vec3::Y * 0.08, hand + rot * Vec3::Y * 0.07, 0.027, LEATHER_D));
    p.push(block(hand + rot * Vec3::Y * 0.1, Vec3::new(0.13, 0.022, 0.03), rot, GOLD));
    p.push(shiny(block(hand + rot * Vec3::Y * 0.56, Vec3::new(0.04, 0.45, 0.011), rot, 0xeef2ff), 0.9));
    p.push(shiny(cone(hand + rot * Vec3::Y * 1.0, hand + rot * Vec3::Y * 1.1, 0.04, 0.004, 0xeef2ff), 0.9));
    p.push(ball(hand - rot * Vec3::Y * 0.1, 0.034, GOLD));
    let center = (j.elbow[1] + j.hand[1]) / 2.0 + Vec3::new(0.07, 0.0, 0.08);
    let face_dir = Vec3::new(0.35, 0.0, 1.0).normalize();
    let a = center - face_dir * 0.025;
    let b = center + face_dir * 0.025;
    p.push(shiny(cone(a, b, 0.24, 0.24, WHITE), 0.5));
    p.push(cone(a + face_dir * 0.01, b + face_dir * 0.015, 0.2, 0.2, BLUE));
    p.push(shiny(ball(center + face_dir * 0.04, 0.06, GOLD), 0.9));
    p
}

fn sorcerer(d: &Dims, j: &Joints) -> Vec<Part> {
    let pal = Palette { pants: 0x2a1c44, boots: 0x3a2820, torso: 0x4a2a8a, waist: 0x2a1c44, sleeve: 0x5a34a0, glove: SKIN };
    let mut p = body(d, j, &pal, true);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    let (sway, trail) = (j.sway, j.trail);
    let robe = 0x5a34a8;

    // Longue robe brodée jusqu'aux chevilles, avec un galon doré sur le devant et à l'ourlet.
    let hem = Vec3::new(0.0, 0.1, 0.0);
    p.push(shiny(cone(hem, c + Vec3::new(0.0, 0.42, 0.0), 0.4, 0.22, robe), 0.3));
    p.push(cone(hem, hem + Vec3::Y * 0.06, 0.405, 0.395, GOLD));
    for sg in [-1.0f32, 1.0] {
        p.push(panel(c + Vec3::new(sg * 0.05, 0.4, 0.22), Vec3::new(sg * 0.07, 0.12, 0.4), 0.05, 0.012, GOLD));
    }
    // Lune et étoiles dorées sur le pan de la robe.
    for k in 0..7 {
        let a = -1.0 + k as f32 * 0.32;
        p.push(glow(ball(Vec3::new(-0.15 + a.cos() * 0.07, 0.42 + a.sin() * 0.07, 0.34), 0.012, GOLD)));
    }
    p.push(glow(ball(Vec3::new(0.12, 0.34, 0.37), 0.02, GOLD)));
    // Ceinture de cuir avec besace et pendentif sphérique, bandoulière.
    p.push(cone(c + Vec3::new(0.0, 0.05, 0.0), c + Vec3::new(0.0, 0.14, 0.0), d.torso_rx * 0.95, d.torso_rx * 0.9, LEATHER));
    p.push(shiny(ball(c + Vec3::new(0.0, 0.1, d.torso_rx + 0.02), 0.06, GOLD), 0.9));
    p.push(block(c + Vec3::new(0.22, 0.08, 0.1), Vec3::new(0.06, 0.07, 0.04), Mat3::IDENTITY, LEATHER_D));
    p.push(block(c + Vec3::new(0.0, d.torso * 0.72, d.torso_rz * 1.0), Vec3::new(0.018, 0.27, 0.012), Mat3::from_rotation_z(0.6), LEATHER));
    // Écharpe sombre autour du cou, un pan qui pend devant.
    p.push(cone(Vec3::new(0.0, j.shoulder_y, 0.0), Vec3::new(0.0, j.shoulder_y + 0.09, 0.0), 0.2, 0.16, 0x3a2a5a));
    p.push(panel(Vec3::new(-0.08, j.shoulder_y - 0.02, 0.15), Vec3::new(-0.1 + sway, j.shoulder_y - 0.4, 0.17 + trail * 0.2), 0.1, 0.012, 0x3a2a5a));
    // Grande cape brodée dans le dos qui flotte.
    for (x, len, k) in [(-0.2, 0.9, 0.0), (0.0, 1.0, 1.0), (0.2, 0.85, 2.0)] {
        let top = Vec3::new(x, j.shoulder_y - 0.03, -d.torso_rz - 0.04);
        p.push(panel(top, top + Vec3::new(sway * (1.0 + k * 0.4), -len, -0.12 - trail * 0.8), 0.2, 0.012, if k == 1.0 { 0x3c2272 } else { 0x4a2a8a }));
    }
    for i in 0..2 {
        // Manches en cloche à poignet doré.
        let dir = (j.hand[i] - j.elbow[i]).normalize();
        p.push(cone(j.elbow[i], j.hand[i] - dir * 0.04, 0.07, 0.15, 0x5a34a0));
        p.push(cone(j.hand[i] - dir * 0.06, j.hand[i] - dir * 0.03, 0.152, 0.152, GOLD));
    }

    // Tête: grand sourire, boucles brunes, bouc gris.
    let h = j.head;
    let r = d.head_r;
    p.extend(face(h, r, 0x8a60d8, 0x6a5a4a, 0.9));
    p.push(ball(h, r, SKIN));
    let hair = 0x6a4a34;
    p.push(shiny(ellipsoid(h + Vec3::new(0.0, 0.2 * r, -0.22 * r), Vec3::new(1.08, 1.0, 1.05) * r, hair), 0.5));
    for k in 0..9 {
        let a = k as f32 * TAU / 9.0;
        p.push(shiny(ball(h + Vec3::new(a.cos() * 1.0 * r, -0.1 * r + a.sin() * 0.25 * r, a.sin() * 0.5 * r - 0.3 * r), 0.3 * r, hair), 0.5));
    }
    p.push(shiny(ellipsoid(h + Vec3::new(0.0, -0.62 * r, 0.62 * r), Vec3::new(0.3, 0.4, 0.2) * r, 0xb0aca4), 0.3));
    for sg in [-1.0f32, 1.0] {
        p.push(shiny(ellipsoid(h + Vec3::new(sg * 0.2 * r, -0.28 * r, 0.9 * r), Vec3::new(0.2, 0.06, 0.07) * r, 0xb0aca4), 0.3));
    }
    // Chapeau pointu violet à large bord, pointe retombante, bande de cuir, broche solaire dorée et plume.
    let brim = h + Vec3::Y * 0.55 * r;
    let hat = 0x4a2a80;
    p.push(shiny(cone(brim, brim + Vec3::Y * 0.12 * r, 1.7 * r, 1.3 * r, hat), 0.4));
    p.push(cone(brim + Vec3::Y * 0.1 * r, brim + Vec3::Y * 0.5 * r, 0.98 * r, 0.9 * r, 0x3a2a24));
    p.push(shiny(cone(brim + Vec3::Y * 0.12 * r, brim + Vec3::new(0.3 * r, 1.5 * r, 0.0), 0.9 * r, 0.42 * r, hat), 0.4));
    p.push(shiny(cone(brim + Vec3::new(0.3 * r, 1.5 * r, 0.0), brim + Vec3::new(1.0 * r, 2.2 * r, -0.2 * r), 0.42 * r, 0.14 * r, hat), 0.4));
    p.push(shiny(cone(brim + Vec3::new(1.0 * r, 2.2 * r, -0.2 * r), brim + Vec3::new(1.5 * r, 1.9 * r, -0.5 * r), 0.14 * r, 0.03 * r, hat), 0.4));
    p.push(shiny(ball(brim + Vec3::new(0.0, 0.34 * r, 0.96 * r), 0.2 * r, GOLD), 0.9));
    for k in 0..8 {
        let a = k as f32 * TAU / 8.0;
        p.push(shiny(cone(brim + Vec3::new(0.0, 0.34 * r, 1.0 * r), brim + Vec3::new(a.cos() * 0.34 * r, 0.34 * r + a.sin() * 0.34 * r, 1.0 * r), 0.05 * r, 0.01 * r, GOLD), 0.8));
    }
    p.push(cone(brim + Vec3::new(0.8 * r, 0.4 * r, 0.6 * r), brim + Vec3::new(1.5 * r, 1.0 * r, 0.1 * r), 0.1 * r, 0.01 * r, 0x2a2a3a));
    // Bâton torsadé dans la main droite, couronné d'un anneau qui retient un orbe violet.
    let rot = held(j, 0, 0.12);
    let hand = j.hand[0];
    for k in 0..10 {
        let t = k as f32 / 9.0;
        let twist = (t * TAU * 2.0).sin() * 0.018;
        p.push(shiny(ball(hand + rot * Vec3::new(twist, -0.55 + t * 1.6, twist * 0.5), 0.034, 0x7a5030), 0.4));
    }
    p.push(limb(hand - rot * Vec3::Y * 0.55, hand + rot * Vec3::Y * 1.05, 0.024, 0x5a3a20));
    let orb = hand + rot * Vec3::Y * 1.3;
    for k in 0..10 {
        let a = k as f32 * TAU / 10.0;
        p.push(shiny(ball(orb + rot * Vec3::new(a.cos() * 0.17, a.sin() * 0.17, 0.0), 0.028, 0x7a5030), 0.4));
    }
    p.push(glow(ball(orb, 0.1 * j.fx.glow, 0xb060ff)));
    if let Some((center, axis, radius)) = j.fx.ring {
        // Cercle magique: anneau lumineux autour d'un disque sombre, runes sur le pourtour.
        let n = axis.normalize();
        p.push(glow(cone(center - n * 0.012, center + n * 0.012, radius, radius, 0xb060ff)));
        p.push(cone(center - n * 0.004, center + n * 0.02, radius * 0.8, radius * 0.8, 0x2a1050));
        let up = if n.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let (u, v) = (n.cross(up).normalize(), n.cross(n.cross(up)).normalize());
        for k in 0..8 {
            let a = k as f32 * TAU / 8.0;
            p.push(glow(ball(center + (u * a.cos() + v * a.sin()) * radius * 0.62 + n * 0.025, 0.03, 0xe0c8ff)));
        }
        p.push(glow(ball(center + n * 0.03, 0.05 * j.fx.glow, 0xffffff)));
    }
    p
}

fn ranger(d: &Dims, j: &Joints) -> Vec<Part> {
    let pal = Palette { pants: 0x2a4a44, boots: LEATHER_D, torso: 0x3a7a58, waist: 0x2a4a44, sleeve: 0x3a7a58, glove: LEATHER_D };
    let mut p = body(d, j, &pal, true);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    let (sway, trail) = (j.sway, j.trail);

    // Veste de cuir, baudrier en travers, ceinture à poches, bottes hautes à lacets.
    p.push(ellipsoid(c + Vec3::new(0.0, d.torso * 0.72, 0.0), Vec3::new(d.torso_rx * 1.03, d.torso * 0.31, d.torso_rz * 1.05), LEATHER));
    p.push(block(c + Vec3::new(0.0, d.torso * 0.72, d.torso_rz * 1.03), Vec3::new(0.018, 0.27, 0.012), Mat3::from_rotation_z(0.6), LEATHER_D));
    p.push(cone(c + Vec3::new(0.0, 0.06, 0.0), c + Vec3::new(0.0, 0.16, 0.0), d.torso_rx * 0.8, d.torso_rx * 0.7, LEATHER_D));
    p.push(block(c + Vec3::new(0.13, 0.08, 0.15), Vec3::new(0.06, 0.05, 0.035), Mat3::IDENTITY, LEATHER_D));
    p.push(glow(ball(c + Vec3::new(0.0, 0.11, d.torso_rx * 0.78), 0.026, GOLD)));
    for i in 0..2 {
        p.push(cone(j.ankle[i] + Vec3::Y * 0.1, j.knee[i] - Vec3::Y * 0.08, d.leg_r * 0.85, d.leg_r * 0.82, LEATHER_D));
        // Brassards de cuir et gants sans doigts.
        let dir = (j.hand[i] - j.elbow[i]).normalize();
        p.push(cone(j.hand[i] - dir * 0.16, j.hand[i] - dir * 0.02, d.arm_r * 0.85, d.arm_r * 0.7, LEATHER));
    }
    // Épaulière de cuir sur l'épaule gauche seulement (asymétrie).
    p.push(shiny(ball(j.shoulder[1] + Vec3::new(0.04, 0.05, 0.0), 0.12, LEATHER), 0.3));
    p.push(ball(j.shoulder[1] + Vec3::new(0.12, 0.0, 0.02), 0.05, GOLD));
    // Capuchon rabattu, longue écharpe rouge qui flotte, courte cape déchiquetée.
    p.push(ellipsoid(Vec3::new(0.0, j.shoulder_y + 0.0, -0.13), Vec3::new(0.27, 0.12, 0.17), 0x2a5a44));
    p.push(cone(Vec3::new(0.0, j.shoulder_y + 0.02, 0.0), Vec3::new(0.0, j.shoulder_y + 0.1, 0.0), 0.2, 0.17, 0xd04838));
    for (dx, len, k) in [(-0.2, 0.55, 0.0), (-0.05, 0.7, 1.0), (0.1, 0.5, 2.0), (0.22, 0.62, 3.0)] {
        let top = Vec3::new(dx, j.shoulder_y - 0.02, -d.torso_rz - 0.05);
        let bottom = top + Vec3::new(sway * (1.0 + k * 0.3), -len, -0.08 - trail * 0.7);
        p.push(panel(top, bottom, 0.15, 0.012, if k == 1.0 || k == 3.0 { 0x1e4434 } else { 0x2a5a44 }));
    }
    let tail_root = Vec3::new(0.12, j.shoulder_y + 0.08, -0.12);
    let tail_mid = tail_root + Vec3::new(0.05 + sway * 2.0, -0.12, -0.25 - trail * 1.5);
    let tail_end = tail_mid + Vec3::new(0.08 + sway * 3.0, -0.18, -0.3 - trail * 2.0);
    p.push(panel(tail_root, tail_mid, 0.1, 0.04, 0xd04838));
    p.push(panel(tail_mid, tail_end, 0.09, 0.04, 0xb03828));

    // Tête: regard vif ambré, cheveux turquoise en pagaille, bandeau.
    let h = j.head;
    let r = d.head_r;
    p.extend(face(h, r, 0xf0a828, 0x1a8a78, 0.5));
    p.push(ball(h, r, SKIN));
    let hair = 0x2fc4a8;
    let hair_l = 0x6ae8cc;
    p.push(shiny(ellipsoid(h + Vec3::new(0.0, 0.2 * r, -0.24 * r), Vec3::new(1.06, 1.0, 1.03) * r, hair), 0.9));
    let surf = |x: f32, y: f32| {
        let z = (1.0 - x * x - y * y).max(0.05).sqrt() * 1.03 + 0.04;
        h + Vec3::new(x * r, y * r, z * r)
    };
    for (x, ty, tx) in [(-0.8, 0.12, -0.2), (-0.4, 0.36, 0.0), (0.0, 0.3, 0.1), (0.4, 0.4, 0.0), (0.8, 0.2, 0.2)] {
        p.push(lock(h + Vec3::new(x * r * 0.95, 0.98 * r, 0.3 * r), surf(x + tx, ty), 0.21 * r, hair_l));
    }
    for sg in [-1.0f32, 1.0] {
        p.push(lock(h + Vec3::new(sg * 0.95 * r, 0.4 * r, 0.05 * r), h + Vec3::new(sg * 1.1 * r, -0.5 * r, 0.3 * r), 0.2 * r, hair));
    }
    for (x, up, len) in [(-0.7, 0.9, 1.6), (-0.25, 1.0, 2.0), (0.25, 1.0, 1.8), (0.7, 0.9, 1.5)] {
        p.push(lock(
            h + Vec3::new(x * r, up * r, -0.65 * r),
            h + Vec3::new((x * 1.3 + sway * 3.0) * r, (up - 0.55) * r, (-0.65 - len - trail * 3.0) * r),
            0.3 * r,
            hair,
        ));
    }
    p.push(cone(h + Vec3::Y * 0.5 * r, h + Vec3::Y * 0.62 * r, 0.92 * r, 0.88 * r, 0xd04838));
    p.push(panel(h + Vec3::new(0.6 * r, 0.52 * r, -0.7 * r), h + Vec3::new((0.9 + sway * 3.0) * r, -0.4 * r, (-1.4 - trail * 3.0) * r), 0.2 * r, 0.01, 0xd04838));

    // Carquois dans le dos, arc dans la main droite.
    let (qa, qb) = (c + Vec3::new(0.1, 0.0, -0.27), c + Vec3::new(0.2, 0.66, -0.3));
    p.push(cone(qa, qb, 0.065, 0.085, LEATHER_D));
    for (dx, color) in [(-0.035, 0xf4f0e8), (0.0, 0xd03030), (0.04, 0xf4f0e8)] {
        p.push(block(qb + Vec3::new(dx, 0.08, 0.0), Vec3::new(0.014, 0.06, 0.014), Mat3::IDENTITY, color));
    }
    let hand = j.hand[0];
    let rot = held(j, 0, 0.0);
    let arc = |t: f32| hand + rot * Vec3::new(-0.09 * (1.0 - t * t), t * 0.6, 0.13 * (1.0 - t * t));
    let steps = 8;
    for k in 0..steps {
        let (t0, t1) = (-1.0 + 2.0 * k as f32 / steps as f32, -1.0 + 2.0 * (k + 1) as f32 / steps as f32);
        p.push(shiny(limb(arc(t0), arc(t1), 0.026, 0x9a6a38), 0.5));
    }
    match j.fx.string {
        // Corde tirée jusqu'à la main qui bande l'arc.
        Some(pull) => {
            p.push(limb(arc(-1.0), pull, 0.008, 0xe8e0d0));
            p.push(limb(pull, arc(1.0), 0.008, 0xe8e0d0));
        }
        None => p.push(limb(arc(-1.0), arc(1.0), 0.008, 0xe8e0d0)),
    }
    if let Some((tail, tip)) = j.fx.arrow {
        let dir = (tip - tail).normalize_or(Vec3::Z);
        p.push(shiny(limb(tail, tip, 0.014, 0xc8a070), 0.5));
        p.push(shiny(cone(tip - dir * 0.02, tip + dir * 0.09, 0.03, 0.004, 0xd8dff0), 0.9));
        p.push(cone(tail, tail + dir * 0.09, 0.03, 0.03, 0xf4f0e8));
    }
    p
}

fn forgebraise(d: &Dims, j: &Joints) -> Vec<Part> {
    let pal = Palette { pants: 0x4a4a5a, boots: LEATHER_D, torso: 0xe8702a, waist: 0x4a4a5a, sleeve: 0xe8702a, glove: 0x5a5a6a };
    let mut p = body(d, j, &pal, true);
    let c = Vec3::new(0.0, j.hip_y, 0.0);
    let (sway, trail) = (j.sway, j.trail);

    // Salopette de cuir à bretelles, grosse ceinture, poches rivetées, sac à dos garni.
    p.push(block(c + Vec3::new(0.0, 0.25, d.torso_rz - 0.01), Vec3::new(0.21, 0.28, 0.03), Mat3::IDENTITY, LEATHER));
    for sg in [-1.0f32, 1.0] {
        p.push(panel(c + Vec3::new(sg * 0.17, 0.5, d.torso_rz - 0.02), c + Vec3::new(sg * 0.2, 0.9, 0.0), 0.06, 0.02, LEATHER_D));
        p.push(ball(c + Vec3::new(sg * 0.17, 0.52, d.torso_rz + 0.01), 0.022, GOLD));
        p.push(ball(c + Vec3::new(sg * 0.14, 0.14, d.torso_rz + 0.03), 0.016, STEEL));
    }
    p.push(cone(c + Vec3::new(0.0, 0.06, 0.0), c + Vec3::new(0.0, 0.16, 0.0), d.torso_rx * 0.86, d.torso_rx * 0.78, LEATHER_D));
    p.push(glow(block(c + Vec3::new(0.0, 0.11, d.torso_rx * 0.82), Vec3::new(0.05, 0.045, 0.012), Mat3::IDENTITY, GOLD)));
    p.push(block(c + Vec3::new(0.0, 0.34, -d.torso_rz - 0.1), Vec3::new(0.26, 0.32, 0.1), Mat3::IDENTITY, LEATHER));
    for sg in [-1.0f32, 1.0] {
        p.push(limb(c + Vec3::new(sg * 0.2, 0.1, -d.torso_rz - 0.22), c + Vec3::new(sg * 0.2, 0.78, -d.torso_rz - 0.22), 0.04, STEEL_D));
    }
    for x in [-0.16, 0.0, 0.16] {
        p.push(ball(c + Vec3::new(x, 0.1, -d.torso_rz - 0.23), 0.075, 0x3a3a48));
        p.push(glow(ball(c + Vec3::new(x, 0.2, -d.torso_rz - 0.23), 0.018, 0xffd040)));
    }
    for i in 0..2 {
        // Gros gants de forge.
        p.push(shiny(ball(j.hand[i], 0.125, 0x5a5a6a), 0.4));
        let dir = (j.hand[i] - j.elbow[i]).normalize();
        p.push(cone(j.hand[i] - dir * 0.12, j.hand[i], 0.09, 0.115, 0x6a6a7a));
    }
    // Tête: grand sourire, yeux verts, cheveux orange en piques, lunettes sur le front, bandeau rouge, suie.
    let h = j.head;
    let r = d.head_r;
    p.extend(face(h, r, 0x70d040, 0xc05a18, 1.0));
    p.push(ball(h, r, SKIN));
    let hair = 0xff7a28;
    let hair_l = 0xffa040;
    p.push(shiny(ellipsoid(h + Vec3::new(0.0, 0.22 * r, -0.2 * r), Vec3::new(1.07, 1.0, 1.04) * r, hair), 0.9));
    for k in 0..7 {
        let a = (k as f32 - 3.0) * 0.36;
        let base = h + Vec3::new(a * r * 0.9, 0.95 * r, (0.35 - a.abs() * 0.5) * r);
        let up = if k % 2 == 0 { 1.55 } else { 1.25 };
        p.push(lock(base, base + Vec3::new(a * 0.5 * r, up * r, -0.5 * r - sway * 2.0 * r), 0.3 * r, if k % 2 == 0 { hair_l } else { hair }));
    }
    for sg in [-1.0f32, 1.0] {
        p.push(lock(h + Vec3::new(sg * 0.96 * r, 0.3 * r, 0.0), h + Vec3::new(sg * 1.25 * r, -0.4 * r, 0.2 * r), 0.24 * r, hair));
    }
    p.push(cone(h + Vec3::Y * 0.46 * r, h + Vec3::Y * 0.6 * r, 0.95 * r, 0.9 * r, 0xd03030));
    p.push(panel(h + Vec3::new(0.7 * r, 0.52 * r, -0.7 * r), h + Vec3::new((1.0 + sway * 3.0) * r, -0.5 * r, (-1.3 - trail * 3.0) * r), 0.24 * r, 0.01, 0xd03030));
    p.push(panel(h + Vec3::new(0.6 * r, 0.52 * r, -0.75 * r), h + Vec3::new((0.4 + sway * 3.0) * r, -0.7 * r, (-1.5 - trail * 3.0) * r), 0.22 * r, 0.01, 0xa82020));
    // Lunettes de forge relevées, verres lumineux.
    p.push(shiny(cone(h + Vec3::Y * 0.7 * r, h + Vec3::Y * 0.9 * r, 0.9 * r, 0.82 * r, STEEL_D), 0.6));
    for sg in [-1.0f32, 1.0] {
        p.push(shiny(cone(h + Vec3::new(sg * 0.38 * r, 0.8 * r, 0.62 * r), h + Vec3::new(sg * 0.38 * r, 0.8 * r, 0.9 * r), 0.3 * r, 0.3 * r, STEEL_D), 0.6));
        p.push(glow(ball(h + Vec3::new(sg * 0.38 * r, 0.8 * r, 0.92 * r), 0.21 * r, CYAN)));
    }
    p.push(ellipsoid(h + Vec3::new(0.35 * r, -0.2 * r, 0.9 * r), Vec3::new(0.18, 0.08, 0.04) * r, 0x4a3a3a));
    // Marteau runique dans la main droite.
    let rot = held(j, 0, 0.25);
    let hand = j.hand[0];
    p.push(limb(hand - rot * Vec3::Y * 0.4, hand + rot * Vec3::Y * 0.55, 0.036, LEATHER));
    let head_c = hand + rot * Vec3::Y * 0.68;
    p.push(shiny(block(head_c, Vec3::new(0.2, 0.12, 0.12), rot, STEEL), 0.7));
    p.push(block(head_c + rot * Vec3::new(0.2, 0.0, 0.0), Vec3::new(0.035, 0.14, 0.14), rot, STEEL_D));
    p.push(block(head_c - rot * Vec3::new(0.2, 0.0, 0.0), Vec3::new(0.035, 0.14, 0.14), rot, STEEL_D));
    p.push(glow(block(head_c + rot * Vec3::new(0.0, 0.0, 0.125), Vec3::new(0.06, 0.04, 0.008), rot, 0xff8a2a)));
    for (a, b, r) in &j.fx.trail {
        p.push(glow(limb(*a, *b, *r, 0xffa040)));
    }
    if let Some((center, radius)) = j.fx.shock {
        // Onde de choc au sol: anneau de braises et gerbes d'étincelles.
        for k in 0..18 {
            let a = k as f32 * TAU / 18.0;
            let at = center + Vec3::new(a.cos() * radius, 0.0, a.sin() * radius);
            p.push(glow(ball(at, 0.05, 0xff9a30)));
            if k % 3 == 0 {
                p.push(glow(cone(at, at + Vec3::new(a.cos() * 0.08, 0.28, a.sin() * 0.08), 0.03, 0.006, 0xffe080)));
            }
        }
    }
    // Bombe à la hanche gauche, mèche allumée.
    let bomb = c + Vec3::new(d.torso_rx + 0.06, 0.02, 0.1);
    p.push(shiny(ball(bomb, 0.11, 0x3a3a48), 0.6));
    p.push(limb(bomb + Vec3::Y * 0.1, bomb + Vec3::new(0.02, 0.2, 0.0), 0.012, LEATHER));
    p.push(glow(ball(bomb + Vec3::new(0.02, 0.21, 0.0), 0.028, 0xffd040)));
    p
}

pub fn build(class: Class, pose: Pose, theme: Theme) -> Vec<Part> {
    match theme {
        Theme::Manga => build_manga(class, pose),
        Theme::Gore => crate::heroes_gore::build(class, pose),
        Theme::Medieval => crate::heroes_medieval::build(class, pose),
    }
}

fn build_manga(class: Class, pose: Pose) -> Vec<Part> {
    let dims = Dims::of(class);
    let joints = joints(&dims, pose, Some(class));
    match class {
        Class::Knight => knight(&dims, &joints),
        Class::Sorcerer => sorcerer(&dims, &joints),
        Class::Ranger => ranger(&dims, &joints),
        Class::Forgebraise => forgebraise(&dims, &joints),
    }
}

/// Hauteur du centre de la tête d'un héros au repos, selon le thème (sert à cadrer les vignettes).
pub fn head_height(class: Class, theme: Theme) -> f32 {
    let dims = match theme {
        Theme::Manga => Dims::of(class),
        Theme::Gore => crate::heroes_gore::dims(class),
        Theme::Medieval => crate::heroes_medieval::dims(class),
    };
    joints(&dims, Pose::Idle(0.0), None).head.y
}
