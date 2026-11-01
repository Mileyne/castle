//! Visionneuse des modèles 3D des personnages (héros, monstres, objets): un exécutable à part du jeu, pour voir comment ils sont
//! rendus et comment le rendu évolue, étape après étape, sous tous les angles.
//!
//! `cargo run --release -p model_viewer` (ou `scripts/models.sh`).
//!
//! Chaque étape est une façon de dessiner le même modèle; on en ajoute une à chaque avancée du travail. Le mode comparaison en montre
//! deux côte à côte, avec leur coût (entités, appels de dessin, sommets).
//!
//! Manette: stick droit = tourner autour; gâchettes L / R (ou ZL / ZR) = éloigner / rapprocher; croix gauche / droite = modèle précédent
//! / suivant; croix haut / bas = animation (repos, marche, coup); A = étape suivante (à droite); X = comparer deux étapes; Y = rotation
//! automatique; Select = recentrer la vue.
//! Clavier: flèches = tourner; Q / E = zoom; A / D = modèle; W / S = animation; Tab = étape; C = comparer; Espace = rotation; R = recentrer.

use std::collections::HashMap;
use std::time::Instant;

use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    mesh::VertexAttributeValues,
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
    window::PresentMode,
};
use sprite_forge::{Anim, LivePart, LiveShape, Theme, live_parts, models};

// Le code de fusion est celui du jeu: la visionneuse montre ce que le jeu dessine.
#[path = "../../../client/src/merged.rs"]
mod merged;

/// Les étapes du rendu, de la première à la plus récente: (nom, ce que ça change).
const STAGES: [(&str, &str); 2] = [
    ("1. Primitives superposées", "une entité par pièce (sphères, cubes, capsules, cônes), repositionnées à chaque image"),
    ("2. Maillage fusionné", "toutes les pièces d'une pose dans un seul maillage, gardé en cache: une entité par personnage"),
];

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

    /// Nombre d'images de la boucle de sa planche: le rendu fusionné n'en construit pas davantage.
    fn frames(self) -> usize {
        match self {
            Self::Idle => 25,
            Self::Walk => 16,
            Self::Attack => 10,
        }
    }
}

/// Ce que montre la visionneuse, et la caméra.
#[derive(Resource)]
struct Viewer {
    keys: Vec<String>,
    model: usize,
    motion: Motion,
    compare: bool,
    left: usize,
    right: usize,
    yaw: f32,
    pitch: f32,
    distance: f32,
    auto_rotate: bool,
}

const DEFAULT_YAW: f32 = 0.5;
const DEFAULT_PITCH: f32 = 0.35;
const DEFAULT_DISTANCE: f32 = 7.5;

impl Viewer {
    fn reset_camera(&mut self) {
        self.yaw = DEFAULT_YAW;
        self.pitch = DEFAULT_PITCH;
        self.distance = DEFAULT_DISTANCE;
    }
}

/// Un exemplaire du modèle dans la scène: à gauche ou à droite, avec l'étape qui le dessine.
#[derive(Component)]
struct Instance {
    side: usize,
    /// Ce qui est construit en ce moment: (modèle, étape, mouvement).
    built: Option<(usize, usize, usize)>,
    /// Les entités de pièces (étape 1) ou le corps (étape 2).
    children: Vec<Entity>,
    /// La pose montrée par le maillage fusionné, pour ne le changer que quand elle change.
    pose: Option<usize>,
    /// Coût de cette étape, pour l'affichage.
    stats: Stats,
}

#[derive(Default, Clone, Copy)]
struct Stats {
    parts: usize,
    entities: usize,
    vertices: usize,
    /// Dernier temps de construction d'une pose, en microsecondes.
    build_micros: f64,
}

#[derive(Component)]
struct Hud;

#[derive(ShaderType, Clone, Debug)]
struct PartData {
    color: Vec4,
    params: Vec4,
}

/// Étape 1: un matériau par couleur de pièce.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct PartMaterial {
    #[uniform(0)]
    data: PartData,
}

impl Material for PartMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/toon_parts.wesl".into()
    }
}

/// Étape 2: un seul matériau, la couleur est dans les sommets.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
struct MergedMaterial {}

impl Material for MergedMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/toon_merged.wesl".into()
    }
}

#[derive(Resource, Default)]
struct Caches {
    shapes: HashMap<(u8, i32, i32), Handle<Mesh>>,
    parts: HashMap<[u8; 6], Handle<PartMaterial>>,
    merged_material: Option<Handle<MergedMaterial>>,
    /// Les maillages fusionnés déjà construits: (modèle, mouvement, image).
    poses: HashMap<(usize, usize, usize), (Handle<Mesh>, usize)>,
}

fn main() {
    let keys: Vec<String> = models().into_iter().map(|model| model.key).collect();
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
            MaterialPlugin::<PartMaterial>::default(),
            MaterialPlugin::<MergedMaterial>::default(),
            FrameTimeDiagnosticsPlugin::default(),
        ))
        .insert_resource(ClearColor(Color::srgb(0.16, 0.2, 0.17)))
        .insert_resource(Viewer {
            keys,
            model: 0,
            motion: Motion::Idle,
            compare: false,
            left: 0,
            right: STAGES.len() - 1,
            yaw: DEFAULT_YAW,
            pitch: DEFAULT_PITCH,
            distance: DEFAULT_DISTANCE,
            auto_rotate: false,
        })
        .init_resource::<Caches>()
        .add_systems(Startup, setup)
        .add_systems(Update, (input, place_camera, update_instances, update_hud).chain())
        .run();
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut standard: ResMut<Assets<StandardMaterial>>, assets: Res<AssetServer>) {
    commands.spawn((Camera3d::default(), Projection::from(PerspectiveProjection { fov: 0.5, ..default() }), Transform::default()));
    // Un sol pour se repérer: un disque et un repère de 1 mètre.
    let ground = standard.add(StandardMaterial { base_color: Color::srgb(0.24, 0.3, 0.24), unlit: true, ..default() });
    commands.spawn((Mesh3d(meshes.add(Circle::new(3.5))), MeshMaterial3d(ground), Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))));
    for side in 0..2 {
        commands.spawn((Instance { side, built: None, children: Vec::new(), pose: None, stats: Stats::default() }, Transform::default(), Visibility::default()));
    }
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont { font: assets.load("fonts/PressStart2P-Regular.ttf").into(), font_size: 11.0.into(), ..default() },
        TextColor(Color::WHITE),
        BackgroundColor(Color::BLACK.with_alpha(0.55)),
        Node { position_type: PositionType::Absolute, bottom: Val::Px(8.0), left: Val::Px(8.0), padding: UiRect::all(Val::Px(8.0)), ..default() },
    ));
}

/// Manette et clavier.
fn input(mut viewer: ResMut<Viewer>, keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, pads: Query<&Gamepad>) {
    let dt = time.delta_secs();
    let count = viewer.keys.len();
    let (mut turn, mut tilt, mut zoom) = (0.0f32, 0.0f32, 0.0f32);
    let (mut next, mut motion, mut stage, mut compare, mut auto, mut reset) = (0i32, 0i32, false, false, false, false);
    // Le clavier.
    turn += f32::from(keys.pressed(KeyCode::ArrowRight)) - f32::from(keys.pressed(KeyCode::ArrowLeft));
    tilt += f32::from(keys.pressed(KeyCode::ArrowUp)) - f32::from(keys.pressed(KeyCode::ArrowDown));
    zoom += f32::from(keys.pressed(KeyCode::KeyQ)) - f32::from(keys.pressed(KeyCode::KeyE));
    next += i32::from(keys.just_pressed(KeyCode::KeyD)) - i32::from(keys.just_pressed(KeyCode::KeyA));
    motion += i32::from(keys.just_pressed(KeyCode::KeyS)) - i32::from(keys.just_pressed(KeyCode::KeyW));
    stage |= keys.just_pressed(KeyCode::Tab);
    compare |= keys.just_pressed(KeyCode::KeyC);
    auto |= keys.just_pressed(KeyCode::Space);
    reset |= keys.just_pressed(KeyCode::KeyR);
    // La manette (toutes celles qui sont branchées).
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
        stage |= pad.just_pressed(GamepadButton::South);
        compare |= pad.just_pressed(GamepadButton::West);
        auto |= pad.just_pressed(GamepadButton::North);
        reset |= pad.just_pressed(GamepadButton::Select);
    }
    viewer.yaw += turn.clamp(-1.0, 1.0) * 1.8 * dt;
    viewer.pitch = (viewer.pitch + tilt.clamp(-1.0, 1.0) * 1.0 * dt).clamp(-0.15, 1.45);
    viewer.distance = (viewer.distance * (zoom.clamp(-1.0, 1.0) * 0.9 * dt).exp()).clamp(1.5, 14.0);
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
    if stage {
        // L'étape de droite passe à la suivante; en comparaison, celle de gauche est la précédente.
        viewer.right = (viewer.right + 1) % STAGES.len();
        viewer.left = (viewer.right + STAGES.len() - 1) % STAGES.len();
    }
    if compare {
        viewer.compare = !viewer.compare;
        viewer.left = (viewer.right + STAGES.len() - 1) % STAGES.len();
    }
    if auto {
        viewer.auto_rotate = !viewer.auto_rotate;
    }
    if reset {
        viewer.reset_camera();
    }
}

/// La caméra tourne autour du personnage (ou des deux), à hauteur de poitrine.
fn place_camera(viewer: Res<Viewer>, mut camera: Single<&mut Transform, With<Camera3d>>) {
    let target = Vec3::new(0.0, 0.75, 0.0);
    let offset = Vec3::new(viewer.yaw.sin() * viewer.pitch.cos(), viewer.pitch.sin(), viewer.yaw.cos() * viewer.pitch.cos());
    **camera = Transform::from_translation(target + offset * viewer.distance).looking_at(target, Vec3::Y);
}

fn shape_key(shape: LiveShape) -> (u8, i32, i32) {
    match shape {
        LiveShape::Sphere => (0, 0, 0),
        LiveShape::Cube => (1, 0, 0),
        LiveShape::Capsule(_) => (2, 0, 0),
        LiveShape::Frustum(a, b) => (3, (a * 50.0).round() as i32, (b * 50.0).round() as i32),
    }
}

/// Étape 1: la forme d'une pièce, avec la finesse d'origine.
fn shape_mesh(caches: &mut Caches, meshes: &mut Assets<Mesh>, shape: LiveShape) -> Handle<Mesh> {
    caches
        .shapes
        .entry(shape_key(shape))
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

fn part_material(caches: &mut Caches, materials: &mut Assets<PartMaterial>, part: &LivePart) -> Handle<PartMaterial> {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let key = [byte(part.color[0]), byte(part.color[1]), byte(part.color[2]), part.glow as u8, byte(part.shine), 0];
    caches
        .parts
        .entry(key)
        .or_insert_with(|| {
            materials.add(PartMaterial {
                data: PartData {
                    color: Vec4::new(part.color[0], part.color[1], part.color[2], 1.0),
                    params: Vec4::new(if part.glow { 1.0 } else { 0.0 }, part.shine, 1.0, 0.0),
                },
            })
        })
        .clone()
}

fn vertex_count(mesh: &Mesh) -> usize {
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(values)) => values.len(),
        _ => 0,
    }
}

/// Construit ou met à jour ce que montre chaque exemplaire.
fn update_instances(
    mut commands: Commands,
    time: Res<Time>,
    viewer: Res<Viewer>,
    mut instances: Query<(Entity, &mut Instance, &mut Transform, &mut Visibility)>,
    mut transforms: Query<&mut Transform, Without<Instance>>,
    mut caches: ResMut<Caches>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut part_materials: ResMut<Assets<PartMaterial>>,
    mut merged_materials: ResMut<Assets<MergedMaterial>>,
) {
    let merged_material = caches.merged_material.get_or_insert_with(|| merged_materials.add(MergedMaterial::default())).clone();
    let phase = (time.elapsed_secs() / viewer.motion.period()).fract();
    let motion_index = Motion::ALL.iter().position(|m| *m == viewer.motion).unwrap_or(0);
    for (entity, mut instance, mut transform, mut visibility) in &mut instances {
        // Une seule étape en vue normale (à droite, au centre); deux en comparaison.
        let shown = viewer.compare || instance.side == 1;
        let wanted = if shown { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != wanted {
            *visibility = wanted;
        }
        transform.translation = Vec3::new(if !viewer.compare { 0.0 } else if instance.side == 0 { -1.3 } else { 1.3 }, 0.0, 0.0);
        if !shown {
            continue;
        }
        let stage = if instance.side == 0 { viewer.left } else { viewer.right };
        let key = &viewer.keys[viewer.model];
        let want = (viewer.model, stage, motion_index);
        if instance.built != Some(want) {
            for child in instance.children.drain(..) {
                commands.entity(child).despawn();
            }
            instance.built = Some(want);
            instance.pose = None;
            instance.stats = Stats::default();
        }
        let anim = viewer.motion.anim(phase);
        if stage == 0 {
            // Étape 1: des pièces, repositionnées à chaque image.
            let started = Instant::now();
            let parts = live_parts(key, Theme::Medieval, anim);
            instance.stats.build_micros = started.elapsed().as_secs_f64() * 1e6;
            if instance.children.len() != parts.len() {
                for child in instance.children.drain(..) {
                    commands.entity(child).despawn();
                }
                instance.children = parts
                    .iter()
                    .map(|part| {
                        commands
                            .spawn((
                                Mesh3d(shape_mesh(&mut caches, &mut meshes, part.shape)),
                                MeshMaterial3d(part_material(&mut caches, &mut part_materials, part)),
                                merged::part_transform(part),
                                ChildOf(entity),
                            ))
                            .id()
                    })
                    .collect();
                let handles: Vec<Handle<Mesh>> = parts.iter().map(|part| shape_mesh(&mut caches, &mut meshes, part.shape)).collect();
                let vertices: usize = handles.iter().map(|handle| meshes.get(handle).map_or(0, vertex_count)).sum();
                instance.stats.vertices = vertices;
            } else {
                for (child, part) in instance.children.iter().zip(&parts) {
                    if let Ok(mut place) = transforms.get_mut(*child) {
                        *place = merged::part_transform(part);
                    }
                }
            }
            instance.stats.parts = parts.len();
            instance.stats.entities = parts.len();
        } else {
            // Étape 2: un maillage par image de la boucle, construit une fois.
            let frames = viewer.motion.frames();
            let frame = ((phase * frames as f32) as usize).min(frames - 1);
            if instance.pose != Some(frame) {
                let cache_key = (viewer.model, motion_index, frame);
                let (handle, vertices) = match caches.poses.get(&cache_key) {
                    Some(found) => found.clone(),
                    None => {
                        let started = Instant::now();
                        let t = frame as f32 / frames as f32;
                        let parts = live_parts(key, Theme::Medieval, viewer.motion.anim(t));
                        let mesh = merged::merge(&parts);
                        instance.stats.build_micros = started.elapsed().as_secs_f64() * 1e6;
                        let vertices = vertex_count(&mesh);
                        instance.stats.parts = parts.len();
                        let handle = meshes.add(mesh);
                        caches.poses.insert(cache_key, (handle.clone(), vertices));
                        (handle, vertices)
                    }
                };
                instance.stats.vertices = vertices;
                instance.stats.entities = 1;
                match instance.children.first().copied() {
                    Some(body) => {
                        commands.entity(body).insert(Mesh3d(handle));
                    }
                    None => {
                        let body = commands.spawn((Mesh3d(handle), MeshMaterial3d(merged_material.clone()), ChildOf(entity))).id();
                        instance.children.push(body);
                    }
                }
                instance.pose = Some(frame);
            }
        }
    }
}

fn update_hud(
    viewer: Res<Viewer>,
    diagnostics: Res<DiagnosticsStore>,
    instances: Query<&Instance>,
    mut hud: Single<&mut Text, With<Hud>>,
) {
    let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let mut text = format!(
        "{}   [{}]   {} / {}   {fps:.0} FPS\n",
        viewer.keys[viewer.model],
        viewer.motion.label(),
        viewer.model + 1,
        viewer.keys.len()
    );
    let mut sides: Vec<&Instance> = instances.iter().collect();
    sides.sort_by_key(|instance| instance.side);
    for instance in sides {
        if !viewer.compare && instance.side == 0 {
            continue;
        }
        let stage = if instance.side == 0 { viewer.left } else { viewer.right };
        let stats = instance.stats;
        let (name, what) = STAGES[stage];
        text += &format!(
            "\n{name}\n   {what}\n   {} pièces, {} entités, {} sommets, mise en pose {:.0} µs\n",
            stats.parts, stats.entities, stats.vertices, stats.build_micros
        );
    }
    text += "\nStick droit: tourner   L / R: zoom   croix: modèle / animation   A: étape   X: comparer   Y: rotation   Select: recentrer";
    if text != hud.0 {
        hud.0 = text;
    }
}
