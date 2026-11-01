//! Essai de rendu en direct: les modèles de `sprite_forge` (héros et monstres) dessinés par la carte graphique en 3D, pièce par pièce,
//! au lieu d'être lus dans des planches de sprites.
//!
//! `cargo run --release -p live3d` ouvre une fenêtre; `--bench` fait varier le nombre de personnages à l'écran, relève la cadence
//! et le coût de la mise en pose (en µs par personnage), enregistre une image (`/tmp/live3d.png`) et quitte.

use std::collections::HashMap;
use std::time::Instant;

use bevy::{
    camera::{RenderTarget, ScalingMode},
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::{
        render_resource::{AsBindGroup, ShaderType, TextureFormat},
        view::screenshot::{Screenshot, save_to_disk},
    },
    shader::ShaderRef,
    window::PresentMode,
};
use sprite_forge::{Anim, LivePart, LiveShape, Theme, live_parts, models};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
/// Hauteur de la vue en mètres: un personnage de 1,8 m fait alors 108 pixels, comme à l'écran dans le jeu.
const VIEW_HEIGHT: f32 = 12.0;
/// Personnages par ligne de la grille (à la hauteur de vue de base).
const COLUMNS: usize = 11;
const ELEVATION: f32 = 0.42;

#[derive(ShaderType, Clone, Debug)]
struct ToonData {
    color: Vec4,
    params: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct ToonMaterial {
    #[uniform(0)]
    data: ToonData,
}

impl Material for ToonMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/toon.wesl".into()
    }
}

/// Un personnage: son modèle, sa façon de bouger, et ses pièces une fois créées.
#[derive(Component)]
struct Character {
    key: String,
    index: usize,
    parts: Vec<Entity>,
}

#[derive(Component)]
struct Hud;

/// Ce que l'essai fait en ce moment.
#[derive(Resource)]
struct Bench {
    /// Nombres de personnages à essayer, dans l'ordre; `None` hors du mode `--bench`.
    plan: Option<Vec<usize>>,
    step: usize,
    frames: u32,
    started: Option<Instant>,
    pose_micros: f64,
    pose_calls: u64,
    shot_taken: bool,
    results: Vec<String>,
}

#[derive(Resource)]
struct Scene {
    count: usize,
    dirty: bool,
    /// Hauteur de la vue en mètres: elle grandit avec le nombre de personnages, pour que tous restent à l'écran (plus petits).
    view: f32,
}

#[derive(Resource, Default)]
struct Caches {
    meshes: HashMap<(u8, i32, i32), Handle<Mesh>>,
    materials: HashMap<[u8; 6], Handle<ToonMaterial>>,
}

#[derive(Resource)]
struct Target(Handle<Image>);

fn main() {
    let bench = std::env::args().any(|a| a == "--bench");
    let count = std::env::args().skip_while(|a| a != "--count").nth(1).and_then(|n| n.parse().ok()).unwrap_or(24);
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window { title: "Castle - rendu en direct".into(), resolution: (WIDTH, HEIGHT).into(), present_mode: PresentMode::AutoNoVsync, ..default() }),
                ..default()
            }),
            MaterialPlugin::<ToonMaterial>::default(),
            FrameTimeDiagnosticsPlugin::default(),
        ))
        .insert_resource(ClearColor(Color::srgb(0.2, 0.28, 0.2)))
        .insert_resource(Scene { count, dirty: true, view: VIEW_HEIGHT })
        .insert_resource(Caches::default())
        .insert_resource(Bench {
            plan: bench.then(|| vec![12, 66, 130, 260, 520, 1040]),
            step: 0,
            frames: 0,
            started: None,
            pose_micros: 0.0,
            pose_calls: 0,
            shot_taken: false,
            results: Vec::new(),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (rebuild_scene, fit_view, animate, drive_bench, update_hud, shot_mode).chain())
        .run();
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut meshes: ResMut<Assets<Mesh>>, mut standard: ResMut<Assets<StandardMaterial>>) {
    let target = images.add(Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None));
    commands.insert_resource(Target(target.clone()));
    let aim = Vec3::new(0.0, 0.9, 0.0);
    commands.spawn((
        Camera3d::default(),
        RenderTarget::Image(target.clone().into()),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical { viewport_height: VIEW_HEIGHT },
            ..OrthographicProjection::default_3d()
        }),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Msaa::Sample4,
        Transform::from_translation(aim + Vec3::new(0.0, ELEVATION.sin(), ELEVATION.cos()) * 60.0).looking_at(aim, Vec3::Y),
    ));
    // Le sol.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(80.0, 0.2, 80.0))),
        MeshMaterial3d(standard.add(StandardMaterial { base_color: Color::srgb(0.32, 0.42, 0.28), unlit: true, ..default() })),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    // L'image de la caméra, plein écran, puis le texte par-dessus.
    commands.spawn((Camera2d, Camera { order: 1, ..default() }));
    commands.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, ImageNode::new(target)));
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont { font_size: 16.0_f32.into(), ..default() },
        TextColor(Color::WHITE),
        Node { position_type: PositionType::Absolute, left: Val::Px(8.0), top: Val::Px(6.0), ..default() },
    ));
}

/// La liste des modèles à montrer (héros et monstres), dans l'ordre.
fn keys() -> Vec<String> {
    // `--only <modèle>` (par exemple `monster:Wolf`) ne montre que ce modèle.
    let only = std::env::args().skip_while(|a| a != "--only").nth(1);
    models().into_iter().filter(|m| m.group == "Héros" || m.group == "Monstres")
        .map(|m| m.key)
        .filter(|k| only.as_ref().is_none_or(|o| k == o))
        .collect()
}

fn rebuild_scene(mut commands: Commands, mut scene: ResMut<Scene>, characters: Query<Entity, With<Character>>) {
    if !scene.dirty {
        return;
    }
    scene.dirty = false;
    for entity in &characters {
        commands.entity(entity).despawn();
    }
    let keys = keys();
    // Au-delà de ce qui tient à l'écran, on agrandit la vue (et la grille) en gardant la même densité de personnages.
    let zoom = (scene.count as f32 / 66.0).sqrt().max(1.0);
    scene.view = std::env::args().skip_while(|a| a != "--view").nth(1).and_then(|v| v.parse().ok()).unwrap_or(VIEW_HEIGHT * zoom);
    let columns = (COLUMNS as f32 * zoom).ceil() as usize;
    let rows = scene.count.div_ceil(columns);
    for i in 0..scene.count {
        let (column, row) = (i % columns, i / columns);
        let x = (column as f32 - (columns as f32 - 1.0) / 2.0) * 2.1;
        let z = (row as f32 - rows as f32 / 2.0) * 1.5;
        commands.spawn((
            Character { key: keys[i % keys.len()].clone(), index: i, parts: Vec::new() },
            Transform::from_xyz(x, 0.0, z),
            Visibility::default(),
        ));
    }
}

fn mesh_for(caches: &mut Caches, meshes: &mut Assets<Mesh>, shape: LiveShape) -> Handle<Mesh> {
    let key = match shape {
        LiveShape::Sphere => (0, 0, 0),
        LiveShape::Cube => (1, 0, 0),
        LiveShape::Capsule(_) => (2, 0, 0),
        LiveShape::Frustum(a, b) => (3, (a * 50.0).round() as i32, (b * 50.0).round() as i32),
    };
    caches
        .meshes
        .entry(key)
        .or_insert_with(|| {
            meshes.add(match shape {
                LiveShape::Sphere => Sphere::new(1.0).mesh().uv(20, 12),
                LiveShape::Cube => Cuboid::new(2.0, 2.0, 2.0).into(),
                LiveShape::Capsule(_) => Capsule3d::new(1.0, 2.0).mesh().rings(4).latitudes(8).longitudes(16).build(),
                LiveShape::Frustum(a, b) => ConicalFrustum { radius_top: b, radius_bottom: a, height: 2.0 }.mesh().resolution(20).build(),
            })
        })
        .clone()
}

fn material_for(caches: &mut Caches, materials: &mut Assets<ToonMaterial>, part: &LivePart) -> Handle<ToonMaterial> {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let key = [byte(part.color[0]), byte(part.color[1]), byte(part.color[2]), part.glow as u8, byte(part.shine), 0];
    caches
        .materials
        .entry(key)
        .or_insert_with(|| {
            materials.add(ToonMaterial {
                data: ToonData {
                    color: Vec4::new(part.color[0], part.color[1], part.color[2], 1.0),
                    params: Vec4::new(if part.glow { 1.0 } else { 0.0 }, part.shine, 0.0, 0.0),
                },
            })
        })
        .clone()
}

/// Place d'une pièce dans le repère du personnage.
fn part_transform(part: &LivePart) -> Transform {
    let r = Mat3::from_cols_array_2d(&part.rot);
    let mut scale = Vec3::from(part.scale);
    if let LiveShape::Capsule(h) = part.shape {
        // La capsule de référence a un demi-corps de 2 (rayon 1): on l'étire pour que le total fasse `rayon * (h + 1)` de demi-longueur.
        scale.y = part.scale[0] * (h + 1.0) / 2.0;
    }
    Transform { translation: Vec3::from(part.center), rotation: Quat::from_mat3(&r), scale }
}

/// Garde la hauteur de la vue à la valeur voulue par la scène.
fn fit_view(scene: Res<Scene>, mut camera: Single<&mut Projection, With<Camera3d>>) {
    if let Projection::Orthographic(o) = &mut **camera {
        o.scaling_mode = ScalingMode::FixedVertical { viewport_height: scene.view };
    }
}

fn animate(
    mut commands: Commands,
    time: Res<Time>,
    mut characters: Query<(Entity, &mut Character, &mut Transform)>,
    mut parts: Query<&mut Transform, Without<Character>>,
    mut caches: ResMut<Caches>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ToonMaterial>>,
    mut bench: ResMut<Bench>,
) {
    let t = time.elapsed_secs();
    let started = Instant::now();
    let mut calls = 0u64;
    for (entity, mut character, mut transform) in &mut characters {
        let i = character.index as f32;
        // Un tiers marche, un tiers respire, un tiers frappe; chacun tourne lentement sur lui-même.
        let anim = match character.index % 3 {
            0 => Anim::Walk(((t * 1.6 + i * 0.13) % 1.0).abs()),
            1 => Anim::Idle(((t * 1.0 + i * 0.07) % 1.0).abs()),
            _ => Anim::Attack(((t * 2.5 + i * 0.11) % 1.0).abs()),
        };
        transform.rotation = Quat::from_rotation_y(if std::env::args().any(|a| a == "--still") { i * std::f32::consts::FRAC_PI_4 } else { t * 0.5 + i * 0.8 });
        let list = live_parts(&character.key, Theme::Medieval, anim);
        calls += 1;
        if character.parts.len() != list.len() {
            for old in character.parts.drain(..) {
                commands.entity(old).despawn();
            }
            let children: Vec<Entity> = list
                .iter()
                .map(|part| {
                    commands
                        .spawn((
                            Mesh3d(mesh_for(&mut caches, &mut meshes, part.shape)),
                            MeshMaterial3d(material_for(&mut caches, &mut materials, part)),
                            part_transform(part),
                            ChildOf(entity),
                        ))
                        .id()
                })
                .collect();
            character.parts = children;
            continue;
        }
        for (child, part) in character.parts.iter().zip(&list) {
            if let Ok(mut place) = parts.get_mut(*child) {
                *place = part_transform(part);
            }
        }
    }
    if calls > 0 {
        bench.pose_micros += started.elapsed().as_secs_f64() * 1e6;
        bench.pose_calls += calls;
    }
}

fn update_hud(mut hud: Single<&mut Text, With<Hud>>, diagnostics: Res<DiagnosticsStore>, scene: Res<Scene>, characters: Query<&Character>, entities: Query<Entity>) {
    let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let parts: usize = characters.iter().map(|c| c.parts.len()).sum();
    hud.0 = format!("{} personnages, {parts} pièces, {} entités - {fps:.0} images/s", scene.count, entities.iter().count());
}

fn drive_bench(
    mut commands: Commands,
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut bench: ResMut<Bench>,
    mut scene: ResMut<Scene>,
    target: Res<Target>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(plan) = bench.plan.clone() else {
        return;
    };
    bench.frames += 1;
    if bench.step >= plan.len() {
        for line in &bench.results {
            println!("{line}");
        }
        exit.write(AppExit::Success);
        return;
    }
    if scene.count != plan[bench.step] {
        scene.count = plan[bench.step];
        scene.dirty = true;
        bench.frames = 0;
        bench.started = None;
        bench.pose_micros = 0.0;
        bench.pose_calls = 0;
        return;
    }
    // Quelques secondes de chauffe (les pièces se créent), puis une mesure de quatre secondes.
    if bench.started.is_none() && bench.frames > 90 {
        bench.started = Some(Instant::now());
        bench.pose_micros = 0.0;
        bench.pose_calls = 0;
        bench.frames = 0;
        if bench.step == 1 && !bench.shot_taken {
            bench.shot_taken = true;
            commands.spawn(Screenshot::image(target.0.clone())).observe(save_to_disk("/tmp/live3d.png"));
        }
        return;
    }
    let Some(started) = bench.started else {
        return;
    };
    if started.elapsed().as_secs_f32() >= 4.0 {
        let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
        let measured = bench.frames as f32 / started.elapsed().as_secs_f32();
        let per_pose = bench.pose_micros / bench.pose_calls.max(1) as f64;
        let line = format!(
            "RESULTAT: {:3} personnages: {:6.1} images/s ({:5.2} ms par image), mise en pose {:6.1} µs par personnage",
            plan[bench.step],
            measured,
            1000.0 / measured.max(0.001),
            per_pose
        );
        println!("{line}");
        let _ = (fps, time.delta_secs());
        bench.results.push(line);
        bench.step += 1;
    }
}

/// `--shot <fichier.png>`: enregistre l'image de la caméra après quelques secondes, puis quitte.
fn shot_mode(mut commands: Commands, target: Res<Target>, mut frames: Local<u32>, mut exit: MessageWriter<AppExit>) {
    let Some(path) = std::env::args().skip_while(|a| a != "--shot").nth(1) else {
        return;
    };
    *frames += 1;
    if *frames == 150 {
        commands.spawn(Screenshot::image(target.0.clone())).observe(save_to_disk(path));
    }
    if *frames == 190 {
        exit.write(AppExit::Success);
    }
}
