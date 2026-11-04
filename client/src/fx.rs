//! Les effets des pouvoirs des héros: des lumières et des particules posées dans la scène 3D (et non plus des cercles tracés au sol).
//!
//! - **Égide** (Chevalier): une onde dorée, un pilier de lumière, puis une aura et des étincelles qui montent autour de chaque héros
//!   protégé, tant que l'immunité dure; elle clignote dans les dernières secondes.
//! - **Gel** (Ensorceleur): une onde de givre qui s'étend jusqu'à la portée exacte du pouvoir, une tempête de flocons; chaque monstre
//!   figé se couvre de cristaux de glace qui scintillent, et devient bleuté.
//! - **Salve** (Rôdeur): un éclair de lumière et des traits de vent le long de chaque flèche; chaque flèche laisse une traînée dorée.
//! - **Frappe titanesque** (Forgebraise): un flash, une onde de choc qui s'arrête au rayon de l'impact, des fissures incandescentes dans
//!   le sol, des braises et de la poussière.
//!
//! Chaque effet est un carré texturé (lueur, anneau, étincelle, flocon, cristal, fissures: dessinés ici, au démarrage), sans éclairage
//! et ajouté à l'image, qui grandit, bouge et s'éteint.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use castle_shared::{Frozen, HeroClass, Position, Shielded, SpecialFx};

use crate::decor3d::{DecorCamera, LAYER};
use crate::live3d::ModelTint;
use crate::sim::{Hurtbox, ShieldTimer, Volley};

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxRng>()
            .init_resource::<FxClock>()
            .add_systems(Startup, setup)
            .add_observer(on_special)
            .add_observer(|added: On<Add<Frozen>>, mut commands: Commands| {
                commands.entity(added.entity).insert(ModelTint(Color::srgb(0.62, 0.85, 1.12)));
            })
            .add_observer(|removed: On<Remove<Frozen>>, mut commands: Commands| {
                commands.entity(removed.entity).remove::<ModelTint>();
            })
            .add_systems(Update, (aegis_start, freeze_start, ambient_sparks, volley_trails, update_fx).chain());
    }
}

/// Les images et le maillage communs à tous les effets.
#[derive(Resource)]
struct FxAssets {
    quad: Handle<Mesh>,
    glow: Handle<Image>,
    ring: Handle<Image>,
    spark: Handle<Image>,
    flake: Handle<Image>,
    shard: Handle<Image>,
    streak: Handle<Image>,
    cracks: Handle<Image>,
}

#[derive(Resource)]
struct FxRng(u32);

impl Default for FxRng {
    fn default() -> Self {
        Self(0x2545_F491)
    }
}

impl FxRng {
    /// Un nombre dans [0, 1[.
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }

    fn between(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next()
    }
}

/// Les minuteries des étincelles qui accompagnent les effets durables.
#[derive(Resource, Default)]
struct FxClock {
    motes: f32,
    glints: f32,
    trails: f32,
}

// ---------------------------------------------------------------------------------------------------------------------
// Les images
// ---------------------------------------------------------------------------------------------------------------------

/// Une image blanche dont l'opacité vient de `shape(x, y)`, avec x et y dans [-1, 1] (y vers le haut).
fn texture(images: &mut Assets<Image>, size: u32, shape: impl Fn(f32, f32) -> f32) -> Handle<Image> {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for row in 0..size {
        for col in 0..size {
            let x = (col as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let y = 1.0 - (row as f32 + 0.5) / size as f32 * 2.0;
            let alpha = shape(x, y).clamp(0.0, 1.0);
            data.extend_from_slice(&[255, 255, 255, (alpha * 255.0) as u8]);
        }
    }
    let mut image = Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    images.add(image)
}

/// Un bruit de valeur lisse en une dimension (pour les fissures).
fn noise(x: f32, seed: f32) -> f32 {
    let value = |i: f32| ((i * 12.9898 + seed * 78.233).sin() * 43758.547).fract().abs();
    let (i, f) = (x.floor(), x.fract());
    let f = f * f * (3.0 - 2.0 * f);
    value(i) * (1.0 - f) + value(i + 1.0) * f
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut images: ResMut<Assets<Image>>) {
    let glow = texture(&mut images, 64, |x, y| {
        let r2 = x * x + y * y;
        if r2 >= 1.0 { 0.0 } else { (1.0 - r2).powi(2) }
    });
    // Un anneau net, avec un halo doux de part et d'autre et un voile léger à l'intérieur.
    let ring = texture(&mut images, 128, |x, y| {
        let r = (x * x + y * y).sqrt();
        if r >= 1.0 {
            return 0.0;
        }
        let line = (-((r - 0.9) / 0.045).powi(2)).exp();
        let halo = 0.35 * (-((r - 0.9) / 0.16).powi(2)).exp();
        let veil = 0.1 * r * r;
        (line + halo + veil) * (1.0 - ((r - 0.9) / 0.1).max(0.0))
    });
    // Une étoile à quatre branches.
    let spark = texture(&mut images, 64, |x, y| {
        let horizontal = (-14.0 * y.abs()).exp() * (-1.6 * x.abs()).exp();
        let vertical = (-14.0 * x.abs()).exp() * (-1.6 * y.abs()).exp();
        let core = (-18.0 * (x * x + y * y)).exp();
        let fade = (1.0 - (x * x + y * y)).max(0.0);
        (horizontal + vertical + core) * fade.sqrt()
    });
    // Un flocon à six branches.
    let flake = texture(&mut images, 96, |x, y| {
        let r = (x * x + y * y).sqrt();
        if r >= 1.0 {
            return 0.0;
        }
        let mut best: f32 = 0.0;
        for k in 0..3 {
            let a = k as f32 * std::f32::consts::FRAC_PI_3;
            let (s, c) = a.sin_cos();
            let along = (x * c + y * s).abs();
            let across = (x * s - y * c).abs();
            best = best.max((-(across / 0.055).powi(2)).exp() * (1.0 - along).max(0.0).sqrt());
            // Les petites branches, vers les extrémités.
            for sign in [-1.0f32, 1.0] {
                for (start, length) in [(0.45f32, 0.28f32), (0.7, 0.18)] {
                    let (u, v) = (along - start, across);
                    let slant = (v - u * sign * 1.1).abs();
                    if u > 0.0 && u < length {
                        best = best.max((-(slant / 0.05).powi(2)).exp() * 0.8 * (1.0 - u / length));
                    }
                }
            }
        }
        let core = (-30.0 * r * r).exp();
        best + core
    });
    // Un cristal de glace: un losange allongé, plus clair en son centre.
    let shard = texture(&mut images, 64, |x, y| {
        let s = x.abs() / 0.42 + y.abs() / 0.96;
        if s >= 1.0 {
            return 0.0;
        }
        let edge = (1.0 - s).powf(0.35);
        let ridge = 0.65 + 0.35 * (1.0 - x.abs() / 0.42).max(0.0);
        let facet = if x > 0.0 { 1.0 } else { 0.8 };
        edge * ridge * facet
    });
    // Un trait de lumière, plus vif vers l'avant (la droite).
    let streak = texture(&mut images, 128, |x, y| {
        let body = (-(y / 0.13).powi(2)).exp() * (1.0 - x.abs()).max(0.0);
        body * (0.15 + 0.85 * (x + 1.0) / 2.0)
    });
    // Des fissures qui partent du centre.
    let cracks = texture(&mut images, 192, |x, y| {
        let r = (x * x + y * y).sqrt();
        if !(0.04..1.0).contains(&r) {
            return 0.0;
        }
        let theta = y.atan2(x);
        let mut best: f32 = 0.0;
        let count = 9;
        for i in 0..count {
            let base = (i as f32 + 0.35 * noise(i as f32, 1.0)) / count as f32 * std::f32::consts::TAU - std::f32::consts::PI;
            let wander = (noise(r * 6.0 + i as f32 * 3.1, i as f32) - 0.5) * 0.55;
            let mut delta = theta - (base + wander);
            while delta > std::f32::consts::PI {
                delta -= std::f32::consts::TAU;
            }
            while delta < -std::f32::consts::PI {
                delta += std::f32::consts::TAU;
            }
            let distance = delta.abs() * r;
            let width = 0.012 + 0.03 * (1.0 - r);
            // Chaque fissure s'arrête à une distance différente du centre.
            let reach = 0.55 + 0.45 * noise(i as f32 * 1.7, 5.0);
            let strength = (1.0 - r / reach).max(0.0).sqrt();
            best = best.max((-(distance / width).powi(2)).exp() * strength);
        }
        best + 0.5 * (-14.0 * r * r).exp()
    });
    commands.insert_resource(FxAssets { quad: meshes.add(Rectangle::new(1.0, 1.0)), glow, ring, spark, flake, shard, streak, cracks });
}

// ---------------------------------------------------------------------------------------------------------------------
// Un effet
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Plane {
    /// Face à la caméra.
    Billboard,
    /// À plat sur le sol.
    Ground,
}

/// Ce qui retient un effet durable: il disparaît quand son porteur n'a plus cet état.
#[derive(Clone, Copy, PartialEq)]
enum Needs {
    Frozen,
    Shielded,
}

#[derive(Component)]
struct Fx {
    plane: Plane,
    age: f32,
    /// Durée de vie (infinie pour un effet durable).
    life: f32,
    /// Attente avant de paraître.
    delay: f32,
    /// Largeur au début et à la fin, et rapport hauteur / largeur.
    size: (f32, f32),
    aspect: f32,
    /// Opacité maximale.
    alpha: f32,
    color: Color,
    /// Position dans la scène 3D, vitesse, gravité (vers le bas) et freinage.
    at: Vec3,
    vel: Vec3,
    gravity: f32,
    drag: f32,
    angle: f32,
    spin: f32,
    /// Part de la vie passée à apparaître.
    fade_in: f32,
    /// Palpitation de l'opacité: amplitude et vitesse.
    pulse: (f32, f32),
    /// Porteur: l'effet le suit, et dure autant que son état.
    owner: Option<(Entity, Needs, Vec3)>,
    /// Clignote quand l'immunité du porteur touche à sa fin.
    warns: bool,
    material: Handle<StandardMaterial>,
}

/// Les réglages d'un effet à poser.
#[derive(Clone)]
struct Spec {
    texture: Handle<Image>,
    plane: Plane,
    at: Vec3,
    size: (f32, f32),
    life: f32,
    color: Color,
    alpha: f32,
    blend: bool,
    aspect: f32,
    delay: f32,
    vel: Vec3,
    gravity: f32,
    drag: f32,
    angle: f32,
    spin: f32,
    fade_in: f32,
    pulse: (f32, f32),
    owner: Option<(Entity, Needs, Vec3)>,
    warns: bool,
}

impl Spec {
    fn new(texture: &Handle<Image>, plane: Plane, at: Vec3, size: (f32, f32), life: f32, color: Color) -> Self {
        Self {
            texture: texture.clone(),
            plane,
            at,
            size,
            life,
            color,
            alpha: 1.0,
            blend: false,
            aspect: 1.0,
            delay: 0.0,
            vel: Vec3::ZERO,
            gravity: 0.0,
            drag: 0.0,
            angle: 0.0,
            spin: 0.0,
            fade_in: 0.12,
            pulse: (0.0, 0.0),
            owner: None,
            warns: false,
        }
    }
}

/// Pose un effet.
fn emit(commands: &mut Commands, materials: &mut Assets<StandardMaterial>, assets: &FxAssets, spec: Spec) {
    let material = materials.add(StandardMaterial {
        base_color: spec.color.with_alpha(0.0),
        base_color_texture: Some(spec.texture.clone()),
        unlit: true,
        alpha_mode: if spec.blend { AlphaMode::Blend } else { AlphaMode::Add },
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Fx {
            plane: spec.plane,
            age: 0.0,
            life: spec.life,
            delay: spec.delay,
            size: spec.size,
            aspect: spec.aspect,
            alpha: spec.alpha,
            color: spec.color,
            at: spec.at,
            vel: spec.vel,
            gravity: spec.gravity,
            drag: spec.drag,
            angle: spec.angle,
            spin: spec.spin,
            fade_in: spec.fade_in,
            pulse: spec.pulse,
            owner: spec.owner,
            warns: spec.warns,
            material: material.clone(),
        },
        Mesh3d(assets.quad.clone()),
        MeshMaterial3d(material),
        Transform::from_scale(Vec3::ZERO),
        Visibility::default(),
        RenderLayers::layer(LAYER),
    ));
}

/// Position 3D d'un point du monde 2D, à la hauteur `height`.
fn at3(point: Vec2, height: f32) -> Vec3 {
    Vec3::new(point.x, height, -point.y)
}

fn update_fx(
    mut commands: Commands,
    time: Res<Time>,
    camera: Query<&Transform, (With<DecorCamera>, Without<Fx>)>,
    frozen: Query<&Position, With<Frozen>>,
    shielded: Query<(&Position, Option<&ShieldTimer>), With<Shielded>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut effects: Query<(Entity, &mut Fx, &mut Transform)>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let dt = time.delta_secs();
    for (entity, mut fx, mut transform) in &mut effects {
        // Un effet durable suit son porteur et s'éteint avec son état.
        let mut warning = 1.0;
        if let Some((owner, needs, offset)) = fx.owner {
            let place = match needs {
                Needs::Frozen => frozen.get(owner).ok().map(|p| p.0),
                Needs::Shielded => shielded.get(owner).ok().map(|(p, timer)| {
                    if fx.warns && timer.is_some_and(|t| t.0 < 2.5) {
                        warning = 0.55 + 0.45 * (fx.age * 16.0).sin();
                    }
                    p.0
                }),
            };
            let Some(place) = place else {
                commands.entity(entity).despawn();
                continue;
            };
            fx.at = at3(place, 0.0) + offset;
        }
        if fx.delay > 0.0 {
            fx.delay -= dt;
            continue;
        }
        fx.age += dt;
        let t = if fx.life.is_finite() { fx.age / fx.life } else { 0.0 };
        if t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if fx.owner.is_none() {
            let (vel, drag, gravity) = (fx.vel, fx.drag, fx.gravity);
            fx.at += vel * dt;
            fx.vel = vel * (1.0 - (drag * dt).min(1.0)) - Vec3::Y * gravity * dt;
            if fx.plane == Plane::Billboard && fx.at.y < 0.5 {
                fx.at.y = 0.5;
                fx.vel.y = 0.0;
            }
        }
        fx.angle += fx.spin * dt;
        let eased = 1.0 - (1.0 - t).powi(2);
        let width = fx.size.0 + (fx.size.1 - fx.size.0) * eased;
        let rise = if fx.fade_in > 0.0 { (t / fx.fade_in).min(1.0) } else { 1.0 };
        let fall = if fx.life.is_finite() { (1.0 - t).powf(1.4) } else { 1.0 };
        let pulse = 1.0 + fx.pulse.0 * (fx.age * fx.pulse.1).sin();
        let alpha = (fx.alpha * rise.min(1.0) * fall * pulse * warning).clamp(0.0, 1.0);
        if let Some(mut material) = materials.get_mut(&fx.material) {
            let color = fx.color.with_alpha(alpha);
            if material.base_color != color {
                material.base_color = color;
            }
        }
        let scale = Vec3::new(width, width * fx.aspect, 1.0);
        *transform = match fx.plane {
            Plane::Billboard => Transform {
                translation: fx.at,
                rotation: camera.rotation * Quat::from_rotation_z(fx.angle),
                scale,
            },
            Plane::Ground => Transform {
                translation: Vec3::new(fx.at.x, 0.45 + fx.at.y, fx.at.z),
                rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2) * Quat::from_rotation_z(fx.angle),
                scale,
            },
        };
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Les pouvoirs
// ---------------------------------------------------------------------------------------------------------------------

const GOLD: Color = Color::srgb(1.0, 0.8, 0.3);
const ICE: Color = Color::srgb(0.55, 0.85, 1.0);
const FROST: Color = Color::srgb(0.85, 0.96, 1.0);
const EMBER: Color = Color::srgb(1.0, 0.5, 0.12);
const HOT: Color = Color::srgb(1.0, 0.85, 0.55);
const LEAF: Color = Color::srgb(0.7, 1.0, 0.4);

fn on_special(
    fx: On<SpecialFx>,
    mut commands: Commands,
    assets: Res<FxAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
) {
    let (commands, materials, assets, rng) = (&mut commands, &mut *materials, &*assets, &mut *rng);
    let event = *fx;
    match event.class {
        // Le Chevalier lève son bouclier: un pilier de lumière (l'onde et l'aura sont posées sur chaque héros protégé).
        HeroClass::Knight => {
            // Un trait horizontal tourné d'un quart de tour, et plus haut que large.
            let mut pillar = Spec::new(&assets.streak, Plane::Billboard, at3(event.from, 26.0), (80.0, 70.0), 0.7, GOLD);
            pillar.angle = std::f32::consts::FRAC_PI_2;
            pillar.aspect = 0.32;
            pillar.alpha = 0.7;
            emit(commands, materials, assets, pillar);
        }
        // L'Ensorceleur fige les monstres: une onde de givre jusqu'à la portée du pouvoir, et une tempête de flocons.
        HeroClass::Sorcerer => {
            let r = event.radius;
            let centre = event.from;
            let mut wave = Spec::new(&assets.ring, Plane::Ground, at3(centre, 0.0), (24.0, 2.0 * r), 0.75, ICE);
            wave.alpha = 1.0;
            wave.fade_in = 0.05;
            emit(commands, materials, assets, wave.clone());
            let mut second = wave.clone();
            second.size = (10.0, 1.7 * r);
            second.delay = 0.1;
            second.alpha = 0.55;
            emit(commands, materials, assets, second);
            let mut frost = Spec::new(&assets.glow, Plane::Ground, at3(centre, 0.0), (30.0, 2.0 * r), 1.5, ICE);
            frost.alpha = 0.3;
            emit(commands, materials, assets, frost);
            let mut flash = Spec::new(&assets.glow, Plane::Billboard, at3(centre, 12.0), (20.0, 120.0), 0.4, FROST);
            flash.alpha = 0.9;
            emit(commands, materials, assets, flash);
            for _ in 0..42 {
                let angle = rng.next() * std::f32::consts::TAU;
                let speed = rng.between(70.0, r * 1.15);
                let size = rng.between(6.0, 12.0);
                let mut flake = Spec::new(&assets.flake, Plane::Billboard, at3(centre, 12.0 + rng.between(-4.0, 8.0)), (size, size * 0.6), rng.between(0.9, 1.5), FROST);
                flake.vel = Vec3::new(angle.cos() * speed, rng.between(-4.0, 22.0), -angle.sin() * speed);
                flake.drag = 2.4;
                flake.gravity = 6.0;
                flake.spin = rng.between(-3.0, 3.0);
                flake.fade_in = 0.05;
                flake.alpha = 0.95;
                emit(commands, materials, assets, flake);
            }
        }
        // Le Rôdeur tire une salve: un éclair à l'encoche et un trait de vent derrière chaque flèche.
        HeroClass::Ranger => {
            let muzzle = event.from + event.dir * 10.0;
            let mut flash = Spec::new(&assets.glow, Plane::Billboard, at3(muzzle, 12.0), (10.0, 52.0), 0.22, LEAF);
            flash.alpha = 0.9;
            emit(commands, materials, assets, flash);
            let mut star = Spec::new(&assets.spark, Plane::Billboard, at3(muzzle, 12.0), (16.0, 46.0), 0.25, HOT);
            star.alpha = 0.9;
            star.spin = 2.0;
            emit(commands, materials, assets, star);
            let count = crate::balance_arrows();
            let spread = crate::balance_spread();
            for k in 0..count {
                let offset = if count > 1 { (k as f32 / (count - 1) as f32 - 0.5) * spread } else { 0.0 };
                let direction = Vec2::from_angle(offset).rotate(event.dir);
                let mut wind = Spec::new(&assets.streak, Plane::Ground, at3(muzzle + direction * 34.0, 0.0), (74.0, 120.0), 0.32, LEAF);
                wind.aspect = 0.14;
                wind.angle = direction.to_angle();
                wind.alpha = 0.8;
                emit(commands, materials, assets, wind);
            }
            for _ in 0..8 {
                let direction = Vec2::from_angle(rng.between(-0.35, 0.35)).rotate(event.dir);
                let speed = rng.between(40.0, 120.0);
                let size = rng.between(4.0, 8.0);
                let mut spark = Spec::new(&assets.spark, Plane::Billboard, at3(muzzle, 12.0), (size, size * 0.4), rng.between(0.3, 0.55), GOLD);
                spark.vel = Vec3::new(direction.x * speed, rng.between(-5.0, 15.0), -direction.y * speed);
                spark.drag = 3.0;
                emit(commands, materials, assets, spark);
            }
        }
        // Le Forgebraise écrase le sol: flash, onde de choc jusqu'au rayon de l'impact, fissures, braises et poussière.
        HeroClass::Forgebraise => {
            let (centre, r) = (event.at, event.radius);
            let mut flash = Spec::new(&assets.glow, Plane::Billboard, at3(centre, 10.0), (30.0, 130.0), 0.3, HOT);
            flash.alpha = 1.0;
            flash.fade_in = 0.03;
            emit(commands, materials, assets, flash);
            let mut disc = Spec::new(&assets.glow, Plane::Ground, at3(centre, 0.0), (2.0 * r, 2.2 * r), 0.45, EMBER);
            disc.alpha = 0.7;
            disc.fade_in = 0.03;
            emit(commands, materials, assets, disc);
            let mut shock = Spec::new(&assets.ring, Plane::Ground, at3(centre, 0.0), (0.5 * r, 2.3 * r), 0.5, HOT);
            shock.alpha = 1.0;
            shock.fade_in = 0.04;
            emit(commands, materials, assets, shock);
            let mut dust_ring = Spec::new(&assets.ring, Plane::Ground, at3(centre, 0.0), (0.3 * r, 3.0 * r), 0.8, Color::srgb(0.6, 0.5, 0.42));
            dust_ring.delay = 0.05;
            dust_ring.alpha = 0.5;
            dust_ring.blend = true;
            emit(commands, materials, assets, dust_ring);
            // Les fissures: sombres et durables, avec leur lueur qui s'éteint vite.
            let mut scar = Spec::new(&assets.cracks, Plane::Ground, at3(centre, 0.0), (2.4 * r, 2.6 * r), 3.0, Color::srgb(0.05, 0.03, 0.02));
            scar.blend = true;
            scar.alpha = 0.85;
            scar.angle = rng.next() * std::f32::consts::TAU;
            scar.fade_in = 0.02;
            let mut glow_cracks = Spec::new(&assets.cracks, Plane::Ground, at3(centre, 0.0), (2.4 * r, 2.6 * r), 1.1, EMBER);
            glow_cracks.alpha = 1.0;
            glow_cracks.angle = scar.angle;
            glow_cracks.fade_in = 0.02;
            emit(commands, materials, assets, scar);
            emit(commands, materials, assets, glow_cracks);
            for _ in 0..30 {
                let angle = rng.next() * std::f32::consts::TAU;
                let speed = rng.between(40.0, 150.0);
                let size = rng.between(3.0, 7.0);
                let mut ember = Spec::new(&assets.spark, Plane::Billboard, at3(centre, 3.0), (size, size * 0.5), rng.between(0.6, 1.3), if rng.next() < 0.5 { EMBER } else { HOT });
                ember.vel = Vec3::new(angle.cos() * speed, rng.between(30.0, 130.0), -angle.sin() * speed);
                ember.gravity = 190.0;
                ember.drag = 1.0;
                ember.spin = rng.between(-6.0, 6.0);
                ember.fade_in = 0.03;
                emit(commands, materials, assets, ember);
            }
            for _ in 0..10 {
                let angle = rng.next() * std::f32::consts::TAU;
                let speed = rng.between(15.0, 45.0);
                let size = rng.between(14.0, 26.0);
                let mut puff = Spec::new(&assets.glow, Plane::Billboard, at3(centre + Vec2::from_angle(angle) * r * 0.5, 4.0), (size * 0.5, size * 1.4), rng.between(0.7, 1.2), Color::srgb(0.4, 0.34, 0.3));
                puff.blend = true;
                puff.alpha = 0.45;
                puff.vel = Vec3::new(angle.cos() * speed, rng.between(8.0, 24.0), -angle.sin() * speed);
                puff.drag = 1.5;
                puff.delay = rng.between(0.0, 0.1);
                emit(commands, materials, assets, puff);
            }
        }
    }
}

/// Un héros vient d'être protégé par l'Égide: onde dorée sous lui, pilier de lumière, aura qui le suit jusqu'à la fin de l'immunité.
fn aegis_start(
    mut commands: Commands,
    assets: Option<Res<FxAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    fresh: Query<(Entity, &Position), Added<Shielded>>,
) {
    let Some(assets) = assets else {
        return;
    };
    let (commands, materials, assets, rng) = (&mut commands, &mut *materials, &*assets, &mut *rng);
    for (hero, pos) in &fresh {
        let here = pos.0;
        let mut ring = Spec::new(&assets.ring, Plane::Ground, at3(here, 0.0), (14.0, 120.0), 0.6, GOLD);
        ring.alpha = 1.0;
        ring.fade_in = 0.04;
        emit(commands, materials, assets, ring.clone());
        let mut second = ring;
        second.size = (8.0, 84.0);
        second.delay = 0.12;
        second.alpha = 0.6;
        emit(commands, materials, assets, second);
        let mut pillar = Spec::new(&assets.streak, Plane::Billboard, at3(here, 26.0), (80.0, 70.0), 0.7, Color::srgb(1.0, 0.92, 0.6));
        pillar.angle = std::f32::consts::FRAC_PI_2;
        pillar.aspect = 0.32;
        pillar.alpha = 0.85;
        emit(commands, materials, assets, pillar);
        for _ in 0..14 {
            let angle = rng.next() * std::f32::consts::TAU;
            let size = rng.between(4.0, 8.0);
            let mut mote = Spec::new(&assets.spark, Plane::Billboard, at3(here, 3.0), (size, size * 0.5), rng.between(0.7, 1.2), GOLD);
            mote.vel = Vec3::new(angle.cos() * rng.between(8.0, 28.0), rng.between(35.0, 90.0), -angle.sin() * rng.between(8.0, 28.0));
            mote.drag = 1.8;
            mote.spin = rng.between(-4.0, 4.0);
            emit(commands, materials, assets, mote);
        }
        // L'aura: une lueur autour du héros, et une autre au sol; elles palpitent et clignotent avant de s'éteindre.
        let mut aura = Spec::new(&assets.glow, Plane::Billboard, at3(here, 11.0), (46.0, 46.0), f32::INFINITY, GOLD);
        aura.alpha = 0.28;
        aura.pulse = (0.25, 3.0);
        aura.owner = Some((hero, Needs::Shielded, Vec3::new(0.0, 11.0, 0.0)));
        aura.warns = true;
        emit(commands, materials, assets, aura);
        let mut base = Spec::new(&assets.glow, Plane::Ground, at3(here, 0.0), (44.0, 44.0), f32::INFINITY, GOLD);
        base.alpha = 0.4;
        base.pulse = (0.2, 3.0);
        base.owner = Some((hero, Needs::Shielded, Vec3::ZERO));
        base.warns = true;
        emit(commands, materials, assets, base);
    }
}

/// Un monstre vient d'être figé: éclat de givre, cristaux de glace autour de lui et lueur froide au sol, tant qu'il reste figé.
fn freeze_start(
    mut commands: Commands,
    assets: Option<Res<FxAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    fresh: Query<(Entity, &Position, Option<&Hurtbox>), Added<Frozen>>,
) {
    let Some(assets) = assets else {
        return;
    };
    let (commands, materials, assets, rng) = (&mut commands, &mut *materials, &*assets, &mut *rng);
    for (monster, pos, hurtbox) in &fresh {
        let half = hurtbox.map_or(8.0, |h| h.0);
        let here = pos.0;
        let mut flash = Spec::new(&assets.glow, Plane::Billboard, at3(here, half), (half * 2.0, half * 6.0), 0.35, FROST);
        flash.alpha = 0.9;
        emit(commands, materials, assets, flash);
        for _ in 0..8 {
            let angle = rng.next() * std::f32::consts::TAU;
            let size = rng.between(5.0, 9.0);
            let mut chip = Spec::new(&assets.shard, Plane::Billboard, at3(here, half), (size * 0.5, size), rng.between(0.5, 0.9), FROST);
            chip.aspect = 2.2;
            chip.vel = Vec3::new(angle.cos() * rng.between(20.0, 60.0), rng.between(10.0, 50.0), -angle.sin() * rng.between(20.0, 60.0));
            chip.gravity = 90.0;
            chip.drag = 1.0;
            chip.spin = rng.between(-5.0, 5.0);
            emit(commands, materials, assets, chip);
        }
        // Les cristaux qui enveloppent le monstre.
        for k in 0..4 {
            let angle = (k as f32 + rng.next() * 0.5) / 4.0 * std::f32::consts::TAU;
            let offset = Vec3::new(angle.cos() * half * 0.9, half * rng.between(0.7, 1.3), -angle.sin() * half * 0.9);
            let width = half * rng.between(0.7, 1.0);
            let mut crystal = Spec::new(&assets.shard, Plane::Billboard, at3(here, 0.0) + offset, (width, width), f32::INFINITY, Color::srgb(0.7, 0.92, 1.0));
            crystal.aspect = 2.3;
            crystal.blend = true;
            crystal.alpha = 0.8;
            crystal.angle = rng.between(-0.35, 0.35);
            crystal.pulse = (0.12, rng.between(2.0, 4.0));
            crystal.owner = Some((monster, Needs::Frozen, offset));
            emit(commands, materials, assets, crystal);
        }
        let mut chill = Spec::new(&assets.glow, Plane::Billboard, at3(here, half), (half * 3.4, half * 3.4), f32::INFINITY, ICE);
        chill.alpha = 0.3;
        chill.pulse = (0.25, 2.5);
        chill.owner = Some((monster, Needs::Frozen, Vec3::new(0.0, half, 0.0)));
        emit(commands, materials, assets, chill);
        let mut floor = Spec::new(&assets.glow, Plane::Ground, at3(here, 0.0), (half * 4.0, half * 4.0), f32::INFINITY, ICE);
        floor.alpha = 0.4;
        floor.owner = Some((monster, Needs::Frozen, Vec3::ZERO));
        emit(commands, materials, assets, floor);
    }
}

/// Les étincelles des effets durables: des éclats d'or qui montent autour des héros protégés, des scintillements sur les monstres figés.
fn ambient_sparks(
    mut commands: Commands,
    time: Res<Time>,
    assets: Option<Res<FxAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    mut clock: ResMut<FxClock>,
    shielded: Query<&Position, With<Shielded>>,
    frozen: Query<(&Position, Option<&Hurtbox>), With<Frozen>>,
) {
    let Some(assets) = assets else {
        return;
    };
    let (commands, materials, assets, rng) = (&mut commands, &mut *materials, &*assets, &mut *rng);
    let dt = time.delta_secs();
    clock.motes -= dt;
    clock.glints -= dt;
    if clock.motes <= 0.0 {
        clock.motes = 0.09;
        for pos in &shielded {
            let angle = rng.next() * std::f32::consts::TAU;
            let size = rng.between(3.0, 6.0);
            let at = at3(pos.0, 1.0) + Vec3::new(angle.cos() * 9.0, 0.0, -angle.sin() * 9.0);
            let mut mote = Spec::new(&assets.spark, Plane::Billboard, at, (size, size * 0.5), rng.between(0.7, 1.1), GOLD);
            mote.vel = Vec3::new(0.0, rng.between(18.0, 34.0), 0.0);
            mote.spin = rng.between(-3.0, 3.0);
            emit(commands, materials, assets, mote);
        }
    }
    if clock.glints <= 0.0 {
        clock.glints = 0.16;
        for (pos, hurtbox) in &frozen {
            let half = hurtbox.map_or(8.0, |h| h.0);
            let at = at3(pos.0, 0.0) + Vec3::new(rng.between(-half, half), rng.between(0.2, 2.0) * half, rng.between(-half, half) * 0.6);
            let size = rng.between(5.0, 10.0);
            let mut glint = Spec::new(&assets.spark, Plane::Billboard, at, (size * 0.3, size), rng.between(0.3, 0.6), FROST);
            glint.spin = rng.between(-2.0, 2.0);
            glint.fade_in = 0.4;
            emit(commands, materials, assets, glint);
        }
    }
}

/// Chaque flèche d'une salve laisse une traînée dorée.
fn volley_trails(
    mut commands: Commands,
    time: Res<Time>,
    assets: Option<Res<FxAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<FxRng>,
    mut clock: ResMut<FxClock>,
    arrows: Query<&Position, With<Volley>>,
) {
    let Some(assets) = assets else {
        return;
    };
    clock.trails -= time.delta_secs();
    if clock.trails > 0.0 {
        return;
    }
    clock.trails = 0.025;
    let (commands, materials, assets, rng) = (&mut commands, &mut *materials, &*assets, &mut *rng);
    for pos in &arrows {
        let at = at3(pos.0, 8.0);
        let mut glow = Spec::new(&assets.glow, Plane::Billboard, at, (9.0, 2.0), 0.4, LEAF);
        glow.alpha = 0.7;
        emit(commands, materials, assets, glow);
        if rng.next() < 0.5 {
            let size = rng.between(3.0, 6.0);
            let mut spark = Spec::new(&assets.spark, Plane::Billboard, at + Vec3::new(rng.between(-2.0, 2.0), rng.between(-2.0, 2.0), 0.0), (size, size * 0.5), rng.between(0.3, 0.5), GOLD);
            spark.vel = Vec3::new(rng.between(-8.0, 8.0), rng.between(2.0, 14.0), rng.between(-8.0, 8.0));
            emit(commands, materials, assets, spark);
        }
    }
}
