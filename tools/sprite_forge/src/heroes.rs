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
    /// Noms et valeurs de toutes les proportions réglables.
    pub(crate) fn fields(&self) -> [(&'static str, f32); 13] {
        [
            ("thigh", self.thigh),
            ("shin", self.shin),
            ("torso", self.torso),
            ("hip_x", self.hip_x),
            ("shoulder_x", self.shoulder_x),
            ("upper", self.upper),
            ("fore", self.fore),
            ("head_r", self.head_r),
            ("head_rise", self.head_rise),
            ("leg_r", self.leg_r),
            ("arm_r", self.arm_r),
            ("torso_rx", self.torso_rx),
            ("torso_rz", self.torso_rz),
        ]
    }

    /// Applique les proportions réglées dans l'éditeur au modèle `model`.
    pub(crate) fn tweak(mut self, model: &str) -> Self {
        use crate::tuning::param;
        let p = |name: &str, value: f32| param(model, &format!("dims.{name}"), value);
        self.thigh = p("thigh", self.thigh);
        self.shin = p("shin", self.shin);
        self.torso = p("torso", self.torso);
        self.hip_x = p("hip_x", self.hip_x);
        self.shoulder_x = p("shoulder_x", self.shoulder_x);
        self.upper = p("upper", self.upper);
        self.fore = p("fore", self.fore);
        self.head_r = p("head_r", self.head_r);
        self.head_rise = p("head_rise", self.head_rise);
        self.leg_r = p("leg_r", self.leg_r);
        self.arm_r = p("arm_r", self.arm_r);
        self.torso_rx = p("torso_rx", self.torso_rx);
        self.torso_rz = p("torso_rz", self.torso_rz);
        self
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
    // Le cou est le seul morceau de peau du corps habillé.
    let neck = parts.len();
    parts.push(cone(Vec3::new(0.0, j.shoulder_y - 0.02, 0.0), j.head - Vec3::Y * d.head_r * 0.7, 0.065, 0.055, SKIN));
    mark(&mut parts, neck, crate::render::Layer::Body);
    parts
}

/// Rotation d'une arme tenue en main: elle suit un peu l'avant-bras, inclinée vers l'avant.
pub(crate) fn held(j: &Joints, side: usize, tilt: f32) -> Mat3 {
    match (side, j.weapon) {
        (0, Some(rot)) => rot,
        _ => Mat3::from_rotation_x(j.forearm_angle[side] * 0.5 + tilt),
    }
}

pub fn build(class: Class, pose: Pose, _theme: Theme) -> Vec<Part> {
    let mut parts = crate::heroes_medieval::build(class, pose);
    crate::tuning::apply(&crate::tuning::hero_key(class), &mut parts);
    parts
}

/// Proportions d'un héros, avec les réglages en vigueur.
pub(crate) fn dims_of(class: Class, _theme: Theme) -> Dims {
    crate::heroes_medieval::dims(class)
}

/// Hauteur du centre de la tête d'un héros au repos, selon le thème (sert à cadrer les vignettes).
#[allow(dead_code)]
pub fn head_height(class: Class, theme: Theme) -> f32 {
    let dims = dims_of(class, theme);
    joints(&dims, Pose::Idle(0.0), None).head.y
}
