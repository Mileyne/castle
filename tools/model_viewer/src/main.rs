//! Visionneuse des modèles 3D du jeu (héros, monstres, objets, grands motifs, sorties): un exécutable à part, qui montre exactement ce que
//! le jeu dessine (le code de fusion est le sien, voir `client/src/merged.rs`), sous tous les angles.
//!
//! `scripts/models.sh` (ou `cargo run --release -p model_viewer`). Au lancement, comme le jeu au début d'un niveau, toutes les poses de
//! tous les modèles sont construites à l'avance, derrière un écran « Veuillez patienter » avec son avancement.
//!
//! Manette: stick droit = tourner autour; gâchettes L / R (ou ZL / ZR) = éloigner / rapprocher; croix gauche / droite = modèle précédent
//! / suivant; croix haut / bas = action (repos, marche, coup); X = fusion douce / primitives fusionnées / les deux côte à côte;
//! Y = rotation automatique; Select = recentrer la vue.
//! Clavier: flèches = tourner; Q / E = zoom; A / D = modèle; W / S = action; F = fusion; Espace = rotation; R = recentrer.

use std::time::Duration;

use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    window::PresentMode,
};
use sprite_forge::{Anim, models};

// Le code de fusion est celui du jeu: la visionneuse montre ce que le jeu dessine.
#[allow(dead_code)]
#[path = "../../../client/src/merged.rs"]
mod merged;

// La fusion douce: les pièces se fondent en un seul corps continu (voir `client/src/smooth.rs`).
#[allow(dead_code)]
#[path = "../../../client/src/smooth.rs"]
mod smooth;

use merged::{Baking, ModelSource, PoseCache, PoseKey};

/// Les grands motifs du décor et les sorties de niveau, après les modèles réglables.
const LANDMARKS: [&str; 4] = ["rocher", "colonne", "statue", "fontaine"];
const EXITS: [&str; 7] = ["escalier", "barrière fermée", "barrière ouverte", "pont brisé", "pont réparé", "pont-levis levé", "pont-levis baissé"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Motion {
    Idle,
    Walk,
    Attack,
}

impl Motion {
    const ALL: [Motion; 3] = [Self::Idle, Self::Walk, Self::Attack];

    fn label(self) -> &'static str {
        match self {
            Self::Idle => "repos",
            Self::Walk => "marche",
            Self::Attack => "coup",
        }
    }

    /// La famille d'action du modèle fusionné (voir `ModelSource::families`).
    fn family(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Walk => 1,
            Self::Attack => 2,
        }
    }

    /// Durée d'une boucle, en secondes.
    fn period(self) -> f32 {
        match self {
            Self::Idle => 1.0,
            Self::Walk => 0.8,
            Self::Attack => 0.5,
        }
    }

    fn anim(self, t: f32) -> Anim {
        match self {
            Self::Idle => Anim::Idle(t),
            Self::Walk => Anim::Walk(t),
            Self::Attack => Anim::Attack(t),
        }
    }
}

/// Un modèle à montrer: son nom et d'où il vient.
struct Entry {
    label: String,
    source: ModelSource,
}

/// Quel rendu montrer: les primitives fusionnées en un seul maillage (celui du jeu), leur fusion douce, ou les deux côte à côte.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fusion {
    Primitives,
    Soft,
    Both,
}

impl Fusion {
    fn next(self) -> Self {
        match self {
            Self::Both => Self::Soft,
            Self::Soft => Self::Primitives,
            Self::Primitives => Self::Both,
        }
    }
}

/// Ce que montre la visionneuse, et la caméra.
#[derive(Resource)]
struct Viewer {
    fusion: Fusion,
    entries: Vec<Entry>,
    model: usize,
    motion: Motion,
    yaw: f32,
    pitch: f32,
    distance: f32,
    auto_rotate: bool,
}

const DEFAULT_YAW: f32 = 0.5;
const DEFAULT_PITCH: f32 = 0.35;
const DEFAULT_DISTANCE: f32 = 6.0;

/// Le maillage fusionné de chaque pose, construit à l'avance.
#[derive(Resource, Default)]
struct Poses {
    cache: PoseCache,
    material: Option<Handle<ToonMaterial>>,
    /// La construction à l'avance, tant qu'elle n'est pas finie, et son temps (en secondes).
    baking: Option<Baking>,
    baking_secs: f32,
    /// Les poses en fusion douce, construites à la demande: (maillage, sommets, millisecondes de construction).
    soft: std::collections::HashMap<PoseKey, (Handle<Mesh>, usize, f32)>,
    /// La fusion douce de la pose montrée: ses sommets et son temps de construction.
    soft_stats: Option<(usize, f32)>,
}

#[derive(Component)]
struct Hud;

#[derive(Component)]
struct WaitScreen;

#[derive(Component)]
struct WaitPercent;

/// Le modèle montré, et la pose qu'il affiche.
#[derive(Component, Default)]
struct Shown {
    /// La pose montrée à gauche (primitives fusionnées) et à droite (fusion douce).
    poses: [Option<PoseKey>; 2],
    fusion: Option<Fusion>,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct ToonMaterial {
    /// Multiplie la couleur (blanc: les couleurs du modèle).
    #[uniform(0)]
    tint: Vec4,
}

impl Material for ToonMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/toon.wesl".into()
    }
}

fn main() {
    let mut entries: Vec<Entry> = models().into_iter().map(|model| Entry { label: model.key.clone(), source: ModelSource::Key(model.key) }).collect();
    entries.extend(LANDMARKS.iter().enumerate().map(|(index, name)| Entry { label: format!("motif:{name}"), source: ModelSource::Landmark(index) }));
    entries.extend(EXITS.iter().enumerate().map(|(row, name)| Entry { label: format!("sortie:{name}"), source: ModelSource::Exit(row) }));
    let sources: Vec<ModelSource> = entries.iter().map(|entry| entry.source.clone()).collect();
    let baking = Baking::plan(sources, &PoseCache::default());
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Castle - modèles 3D".into(),
                        resolution: (1280, 720).into(),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin { file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(), ..default() }),
            MaterialPlugin::<ToonMaterial>::default(),
            FrameTimeDiagnosticsPlugin::default(),
        ))
        .insert_resource(ClearColor(Color::srgb(0.16, 0.2, 0.17)))
        .insert_resource(Viewer {
            entries,
            fusion: Fusion::Both,
            model: 0,
            motion: Motion::Idle,
            yaw: DEFAULT_YAW,
            pitch: DEFAULT_PITCH,
            distance: DEFAULT_DISTANCE,
            auto_rotate: false,
        })
        .insert_resource(Poses { baking: Some(baking), ..default() })
        .add_systems(Startup, setup)
        .add_systems(Update, (bake, input, place_camera, update_model, update_hud).chain())
        .run();
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut standard: ResMut<Assets<StandardMaterial>>, assets: Res<AssetServer>) {
    commands.spawn((Camera3d::default(), Projection::from(PerspectiveProjection { fov: 0.5, ..default() }), Transform::default()));
    // Un sol pour se repérer.
    let ground = standard.add(StandardMaterial { base_color: Color::srgb(0.24, 0.3, 0.24), unlit: true, ..default() });
    commands.spawn((Mesh3d(meshes.add(Circle::new(3.5))), MeshMaterial3d(ground), Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))));
    commands.spawn((Shown::default(), Transform::default(), Visibility::default()));
    let font = TextFont { font: assets.load("fonts/PressStart2P-Regular.ttf").into(), font_size: 11.0.into(), ..default() };
    commands.spawn((
        Hud,
        Text::new(""),
        font.clone(),
        TextColor(Color::WHITE),
        BackgroundColor(Color::BLACK.with_alpha(0.55)),
        Node { position_type: PositionType::Absolute, bottom: Val::Px(8.0), left: Val::Px(8.0), padding: UiRect::all(Val::Px(8.0)), ..default() },
    ));
    // L'écran d'attente, comme celui du jeu au début d'un niveau.
    commands
        .spawn((
            WaitScreen,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(10),
        ))
        .with_children(|screen| {
            screen.spawn((Text::new("Veuillez patienter"), TextFont { font_size: 24.0.into(), ..font.clone() }, TextColor(Color::WHITE)));
            screen.spawn((Text::new("0 %"), WaitPercent, TextFont { font_size: 24.0.into(), ..font }, TextColor(Color::srgb(1.0, 0.84, 0.0))));
        });
}

/// La construction à l'avance de toutes les poses, par morceaux, derrière l'écran d'attente.
fn bake(
    mut commands: Commands,
    time: Res<Time>,
    mut poses: ResMut<Poses>,
    mut meshes: ResMut<Assets<Mesh>>,
    screen: Query<Entity, With<WaitScreen>>,
    mut percent: Query<&mut Text, With<WaitPercent>>,
) {
    let poses = &mut *poses;
    let Some(baking) = poses.baking.as_mut() else {
        return;
    };
    baking.run(Duration::from_millis(10), &mut poses.cache, &mut meshes);
    poses.baking_secs += time.delta_secs();
    let shown = format!("{} %", baking.percent());
    for mut text in &mut percent {
        if text.0 != shown {
            text.0 = shown.clone();
        }
    }
    if baking.is_finished() {
        poses.baking = None;
        for entity in &screen {
            commands.entity(entity).despawn();
        }
    }
}

/// Manette et clavier.
fn input(mut viewer: ResMut<Viewer>, keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, pads: Query<&Gamepad>) {
    let dt = time.delta_secs();
    let count = viewer.entries.len();
    let (mut turn, mut tilt, mut zoom) = (0.0f32, 0.0f32, 0.0f32);
    let (mut next, mut motion, mut auto, mut reset, mut fusion) = (0i32, 0i32, false, false, false);
    turn += f32::from(keys.pressed(KeyCode::ArrowRight)) - f32::from(keys.pressed(KeyCode::ArrowLeft));
    tilt += f32::from(keys.pressed(KeyCode::ArrowUp)) - f32::from(keys.pressed(KeyCode::ArrowDown));
    zoom += f32::from(keys.pressed(KeyCode::KeyQ)) - f32::from(keys.pressed(KeyCode::KeyE));
    next += i32::from(keys.just_pressed(KeyCode::KeyD)) - i32::from(keys.just_pressed(KeyCode::KeyA));
    motion += i32::from(keys.just_pressed(KeyCode::KeyS)) - i32::from(keys.just_pressed(KeyCode::KeyW));
    auto |= keys.just_pressed(KeyCode::Space);
    fusion |= keys.just_pressed(KeyCode::KeyF);
    reset |= keys.just_pressed(KeyCode::KeyR);
    for pad in &pads {
        let right = pad.right_stick();
        if right.length() > 0.15 {
            turn += right.x;
            tilt += right.y;
        }
        let trigger = |shoulder: GamepadButton, analog: GamepadButton| if pad.pressed(shoulder) { 1.0 } else { pad.get(analog).unwrap_or(0.0) };
        zoom += trigger(GamepadButton::LeftTrigger, GamepadButton::LeftTrigger2) - trigger(GamepadButton::RightTrigger, GamepadButton::RightTrigger2);
        next += i32::from(pad.just_pressed(GamepadButton::DPadRight)) - i32::from(pad.just_pressed(GamepadButton::DPadLeft));
        motion += i32::from(pad.just_pressed(GamepadButton::DPadDown)) - i32::from(pad.just_pressed(GamepadButton::DPadUp));
        auto |= pad.just_pressed(GamepadButton::North);
        fusion |= pad.just_pressed(GamepadButton::West);
        reset |= pad.just_pressed(GamepadButton::Select);
    }
    viewer.yaw += turn.clamp(-1.0, 1.0) * 1.8 * dt;
    viewer.pitch = (viewer.pitch + tilt.clamp(-1.0, 1.0) * dt).clamp(-0.15, 1.45);
    viewer.distance = (viewer.distance * (zoom.clamp(-1.0, 1.0) * 0.9 * dt).exp()).clamp(1.2, 14.0);
    if viewer.auto_rotate {
        viewer.yaw += 0.6 * dt;
    }
    if next != 0 {
        viewer.model = (viewer.model as i32 + next).rem_euclid(count as i32) as usize;
    }
    if motion != 0 {
        let at = Motion::ALL.iter().position(|m| *m == viewer.motion).unwrap_or(0) as i32;
        viewer.motion = Motion::ALL[(at + motion).rem_euclid(Motion::ALL.len() as i32) as usize];
    }
    if auto {
        viewer.auto_rotate = !viewer.auto_rotate;
    }
    if fusion {
        viewer.fusion = viewer.fusion.next();
    }
    if reset {
        viewer.yaw = DEFAULT_YAW;
        viewer.pitch = DEFAULT_PITCH;
        viewer.distance = DEFAULT_DISTANCE;
    }
}

/// La caméra tourne autour du modèle, à hauteur de poitrine.
fn place_camera(viewer: Res<Viewer>, mut camera: Single<&mut Transform, With<Camera3d>>) {
    let target = Vec3::new(0.0, 0.8, 0.0);
    let offset = Vec3::new(viewer.yaw.sin() * viewer.pitch.cos(), viewer.pitch.sin(), viewer.yaw.cos() * viewer.pitch.cos());
    **camera = Transform::from_translation(target + offset * viewer.distance).looking_at(target, Vec3::Y);
}

/// Montre la pose du moment du modèle choisi: l'image de la boucle de son action. À gauche, le maillage des primitives fusionnées (déjà
/// construit); à droite, la fusion douce de la même pose, construite à la demande (une par image au plus).
fn update_model(
    mut commands: Commands,
    time: Res<Time>,
    viewer: Res<Viewer>,
    mut poses: ResMut<Poses>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ToonMaterial>>,
    mut shown: Single<(Entity, &mut Shown)>,
    mut bodies: Local<[Option<Entity>; 2]>,
) {
    if poses.baking.is_some() {
        return;
    }
    let poses = &mut *poses;
    let material = poses.material.get_or_insert_with(|| materials.add(ToonMaterial { tint: Vec4::ONE })).clone();
    let source = &viewer.entries[viewer.model].source;
    let phase = (time.elapsed_secs() / viewer.motion.period()).fract();
    // Un objet, un motif ou une sortie n'ont que leur repos.
    let (family, index) = source.pose(viewer.motion.anim(phase));
    let wanted = (source.clone(), family, index);
    let (root, shown) = &mut *shown;
    let sides = match viewer.fusion {
        Fusion::Both => [Some(-0.9), Some(0.9)],
        Fusion::Primitives => [Some(0.0), None],
        Fusion::Soft => [None, Some(0.0)],
    };
    for side in 0..2 {
        let Some(offset) = sides[side] else {
            if let Some(entity) = bodies[side].take() {
                commands.entity(entity).despawn();
                shown.poses[side] = None;
            }
            continue;
        };
        if shown.poses[side].as_ref() != Some(&wanted) || shown.fusion != Some(viewer.fusion) {
            let handle = if side == 0 {
                poses.cache.get_or_build(&wanted, &mut meshes)
            } else {
                let found = poses.soft.get(&wanted).map(|(handle, vertices, millis)| (handle.clone(), *vertices, *millis));
                let (handle, vertices, millis) = found.unwrap_or_else(|| {
                    let started = std::time::Instant::now();
                    let mesh = smooth::smooth_merge(&source.parts(source.anim(family, index)), smooth::Blend::default());
                    let vertices = mesh.count_vertices();
                    let millis = started.elapsed().as_secs_f32() * 1000.0;
                    let handle = meshes.add(mesh);
                    poses.soft.insert(wanted.clone(), (handle.clone(), vertices, millis));
                    (handle, vertices, millis)
                });
                poses.soft_stats = Some((vertices, millis));
                handle
            };
            match bodies[side] {
                Some(entity) => {
                    commands.entity(entity).insert((Mesh3d(handle), Transform::from_xyz(offset, 0.0, 0.0)));
                }
                None => {
                    bodies[side] = Some(
                        commands
                            .spawn((Mesh3d(handle), MeshMaterial3d(material.clone()), Transform::from_xyz(offset, 0.0, 0.0), ChildOf(*root)))
                            .id(),
                    );
                }
            }
            shown.poses[side] = Some(wanted.clone());
        }
    }
    shown.fusion = Some(viewer.fusion);
}

fn update_hud(viewer: Res<Viewer>, poses: Res<Poses>, diagnostics: Res<DiagnosticsStore>, mut hud: Single<&mut Text, With<Hud>>) {
    let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let entry = &viewer.entries[viewer.model];
    let own = entry.source.all_poses().len();
    let actions: Vec<&str> = Motion::ALL.iter().filter(|m| entry.source.families().contains(&m.family())).map(|m| m.label()).collect();
    let fusion = match viewer.fusion {
        Fusion::Both => "à gauche: primitives fusionnées (le jeu)   à droite: fusion douce",
        Fusion::Primitives => "primitives fusionnées (le jeu)",
        Fusion::Soft => "fusion douce",
    };
    let soft = poses.soft_stats.map_or(String::new(), |(vertices, millis)| format!("\nFusion douce de cette pose: {vertices} sommets, construite en {millis:.0} ms"));
    let text = format!(
        "{}   [{}]   {} / {}   {fps:.0} FPS\n{fusion}\n\n{} poses pour ce modèle (actions: {})\n{} poses construites au lancement en {:.2} s: {:.1} Mo de sommets{soft}\n\nStick droit: tourner   L / R: zoom   croix: modèle / action   X: fusion   Y: rotation   Select: recentrer",
        entry.label,
        viewer.motion.label(),
        viewer.model + 1,
        viewer.entries.len(),
        own,
        actions.join(", "),
        poses.cache.len(),
        poses.baking_secs,
        poses.cache.bytes() as f64 / 1e6,
    );
    if text != hud.0 {
        hud.0 = text;
    }
}
