//! `lumiere`: le programme de réglage de la lumière, niveau par niveau. Il montre un niveau de la campagne tel que le jeu le dessine
//! (son vrai plan, les matériaux de ses murs tuile par tuile, pierre et arbres mêlés quand le niveau en a, ses torches), avec les vrais
//! modèles du jeu (les quatre héros avec leur lanterne, des ennemis, des objets à ramasser, un prisonnier), éclairés exactement comme en
//! partie. On règle en direct, pour ce niveau:
//! - le soleil et la lumière ambiante;
//! - l'intensité et le rayon des lanternes des héros;
//! - pour les héros, les ennemis et les objets: la capacité à capter la lumière (un gain sur leurs couleurs) et à rayonner (la part de
//!   leurs propres couleurs qu'ils émettent, quelle que soit la lumière reçue).
//!
//! `./lumiere.sh [--niveau 1] [--sol terre|herbe|dense] [--yaw 10] [--tilt 55] [--zoom 0.9]`.
//! Tab / Maj+Tab (ou Page haut / Page bas) passent d'un niveau à l'autre; Entrée enregistre tous les niveaux dans
//! `client/assets/lighting.ron`, que le jeu lit au lancement.

use bevy::{
    app::AppExit,
    camera::visibility::RenderLayers,
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    window::WindowResolution,
};
use castle_shared::{Facing, HeroClass, LEVEL_COUNT, Monster, MonsterKind, Room, Tile, level_def};

use super::flame::{FlameMaterial, TorchMark, spawn_torches};
use super::look::{self, ActiveLight, LevelLight, LightTuning, Lights, Mood};
use super::preview::floor_image;
use super::{DecorMaterial, LAYER, decor_data, ground_mesh, wall_mask, walls_mesh};
use crate::art::{PoseKind, Prop, WalkCycle, object};

/// Hauteur de la vue à zoom 1, en unités du monde.
const VIEW_HEIGHT: f32 = 240.0;

#[derive(Resource, Clone)]
struct Options {
    floor: Option<String>,
    zoom: f32,
    tilt: f32,
    yaw: f32,
}

/// Le niveau que l'on règle (de 1 à `LEVEL_COUNT`).
#[derive(Resource)]
struct Level(u32);

/// Ce que le programme garde du niveau affiché.
#[derive(Resource)]
struct Scene {
    room: Room,
    ground: Handle<DecorMaterial>,
    walls: Handle<DecorMaterial>,
    torches: Vec<look::Torch>,
    /// Le héros que l'on déplace (les autres le suivent), et les trois autres avec leur place autour de lui.
    lead: Entity,
    followers: Vec<(Entity, Vec2)>,
    focus: Vec2,
}

/// Tout ce qui appartient au niveau affiché: il disparaît quand on change de niveau.
#[derive(Component)]
struct LevelEntity;

#[derive(Component)]
struct TunerCamera;

/// Le réglage sélectionné dans le panneau.
#[derive(Resource, Default)]
struct Selected(usize);

/// Les réglages tels qu'ils étaient à l'enregistrement précédent: le panneau signale ce qui a changé depuis.
#[derive(Resource)]
struct Saved(LightTuning);

#[derive(Component)]
struct Panel;

#[derive(Component)]
struct Pulse(f32);

/// Le dernier message du programme (enregistrement...), affiché en bas du panneau jusqu'au prochain réglage.
#[derive(Resource, Default)]
struct Message(String);

/// Un réglage: son nom, son pas (et son grand pas avec Maj), et comment le lire et le changer dans les réglages d'un niveau.
struct Setting {
    label: &'static str,
    step: f32,
    big: f32,
    get: fn(&LevelLight) -> f32,
    set: fn(&mut LevelLight, f32),
}

const SETTINGS: [Setting; 10] = [
    Setting { label: "Soleil", step: 0.01, big: 0.05, get: |l| l.sun, set: |l, v| l.sun = v },
    Setting { label: "Lumière ambiante", step: 0.01, big: 0.05, get: |l| l.ambient, set: |l, v| l.ambient = v },
    Setting { label: "Lanternes: intensité (x)", step: 0.05, big: 0.25, get: |l| l.lantern_intensity, set: |l, v| l.lantern_intensity = v },
    Setting { label: "Lanternes: rayon (x)", step: 0.05, big: 0.25, get: |l| l.lantern_radius, set: |l, v| l.lantern_radius = v },
    Setting { label: "Héros: capte la lumière", step: 0.1, big: 0.5, get: |l| l.hero.gain, set: |l, v| l.hero.gain = v },
    Setting { label: "Héros: rayonne (ses couleurs)", step: 0.01, big: 0.05, get: |l| l.hero.glow, set: |l, v| l.hero.glow = v },
    Setting { label: "Ennemis: captent la lumière", step: 0.1, big: 0.5, get: |l| l.enemy.gain, set: |l, v| l.enemy.gain = v },
    Setting { label: "Ennemis: rayonnent (leurs couleurs)", step: 0.01, big: 0.05, get: |l| l.enemy.glow, set: |l, v| l.enemy.glow = v },
    Setting { label: "Objets: captent la lumière", step: 0.1, big: 0.5, get: |l| l.object.gain, set: |l, v| l.object.gain = v },
    Setting { label: "Objets: rayonnent (leurs couleurs)", step: 0.01, big: 0.05, get: |l| l.object.glow, set: |l, v| l.object.glow = v },
];

pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let number = |name: &str, default: f32| value(name).and_then(|v| v.parse().ok()).unwrap_or(default);
    let options = Options {
        floor: value("--sol"),
        zoom: number("--zoom", 0.9),
        tilt: number("--tilt", crate::settings::VIEW_TILT_DEFAULT),
        yaw: number("--yaw", crate::settings::VIEW_ANGLE_DEFAULT),
    };
    let level = value("--niveau").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1).clamp(1, LEVEL_COUNT);
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
    let tuning = LightTuning::load();
    App::new()
        .insert_resource(options)
        .insert_resource(Level(level))
        .insert_resource(Saved(tuning.clone()))
        .insert_resource(tuning)
        .init_resource::<Selected>()
        .init_resource::<Message>()
        .init_resource::<Lights>()
        .init_resource::<Mood>()
        .init_resource::<ActiveLight>()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin { file_path: assets.into(), ..default() })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Castle: réglage de la lumière".into(),
                        resolution: WindowResolution::new(1400, 860),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins((MaterialPlugin::<DecorMaterial>::default(), MaterialPlugin::<FlameMaterial>::default(), crate::live3d::Live3dPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (input, rebuild, walk, pulse, apply, super::apply_mood, super::drive_lamps, panel, shot).chain())
        .run();
}

/// La caméra, le soleil et les lampes, et le panneau: ils restent d'un niveau à l'autre.
fn setup(mut commands: Commands, assets: Res<AssetServer>, options: Res<Options>) {
    let mut projection = OrthographicProjection { near: -2000.0, far: 2000.0, ..OrthographicProjection::default_2d() };
    projection.scale = VIEW_HEIGHT * options.zoom / 860.0;
    commands.spawn((
        TunerCamera,
        Camera3d::default(),
        Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        Projection::from(projection),
        Tonemapping::None,
        RenderLayers::layer(LAYER),
        Transform::default(),
    ));
    super::spawn_sun_and_lamps(&mut commands);
    commands.spawn((
        Panel,
        Text::default(),
        crate::ui::ui_font(&assets, 12.0),
        TextColor(Color::srgb(0.95, 0.92, 0.8)),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.62)),
        Node { position_type: PositionType::Absolute, left: Val::Px(10.0), top: Val::Px(10.0), padding: UiRect::all(Val::Px(10.0)), ..default() },
    ));
}

/// La tuile de sol la plus proche d'un point, en cherchant autour (pour poser un personnage ou un objet là où il y a du sol).
fn floor_near(room: &Room, target: Vec2) -> Vec2 {
    let tile = room.world_to_tile(target);
    for radius in 0..24i32 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let (x, y) = (tile.x + dx, tile.y + dy);
                if room.tile(x, y) == Tile::Floor {
                    return room.tile_center(x as u32, y as u32);
                }
            }
        }
    }
    target
}

/// Construit le niveau choisi: son plan, ses matériaux, ses torches, et les modèles du jeu posés près de son départ.
#[allow(clippy::too_many_arguments)]
fn rebuild(
    mut commands: Commands,
    level: Res<Level>,
    options: Res<Options>,
    tuning: Res<LightTuning>,
    old: Query<Entity, Or<(With<LevelEntity>, With<TorchMark>)>>,
    mut camera: Query<&mut Transform, With<TunerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DecorMaterial>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut flames: ResMut<Assets<FlameMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !level.is_changed() {
        return;
    }
    old.iter().for_each(|entity| commands.entity(entity).despawn());
    let def = level_def(level.0);
    let Ok(dungeon) = def.to_dungeon() else {
        return;
    };
    let room = dungeon.room;
    let size = room.size();
    let region = def.region;
    // Les matériaux de chaque tuile, comme le jeu: la région du niveau, et celles que son décor emprunte (pierre et arbres mêlés).
    let kinds = look::wall_kinds(&room, region, Some(def));
    let palette = look::palette(region, &room, Some(def));
    let light = tuning.light(level.0, region);
    let mood = light.mood();
    let foliage = if kinds.contains(&2) { 2.0 } else { 0.0 };
    let yaw = options.yaw.to_radians();
    let wall_texture = images.add(wall_mask(&room, &kinds));
    let floor = options.floor.clone().unwrap_or_else(|| match region {
        castle_shared::Region::Gardens => "herbe".into(),
        _ => "terre".into(),
    });

    let ground = materials.add(DecorMaterial {
        image: images.add(floor_image(size, &floor)),
        data: decor_data(&room, 0.0, foliage, &palette, mood, yaw),
        walls: wall_texture.clone(),
    });
    commands.spawn((
        LevelEntity,
        Mesh3d(meshes.add(ground_mesh(&room, IVec2::ZERO, size.x, size.y, 0.0))),
        MeshMaterial3d(ground.clone()),
        RenderLayers::layer(LAYER),
    ));
    let walls = materials.add(DecorMaterial {
        image: images.add(Image::default()),
        data: decor_data(&room, 1.0, foliage, &palette, mood, yaw),
        walls: wall_texture,
    });
    commands.spawn((LevelEntity, Mesh3d(meshes.add(walls_mesh(&room))), MeshMaterial3d(walls.clone()), RenderLayers::layer(LAYER)));
    // Les torches des niveaux sombres, sur les murs de pierre (comme le jeu).
    let torches = if mood.darkness() > 0.3 { look::torch_sites(&room, &kinds) } else { Vec::new() };
    spawn_torches(&mut commands, &mut meshes, &mut standard, &mut flames, &torches, yaw);

    // Les modèles du jeu, posés près du départ du niveau, sur du sol: les héros au milieu, des ennemis et des objets plus ou moins loin.
    let start = dungeon.layout.rooms[dungeon.layout.start].center();
    let origin = floor_near(&room, room.tile_center(start.x, start.y));
    let character = |commands: &mut Commands, at: Vec2, bundle: Box<dyn FnOnce(&mut EntityCommands)>| {
        let mut entity = commands.spawn((
            LevelEntity,
            Transform::from_translation(at.extend(0.5)),
            Visibility::default(),
            WalkCycle::default(),
            Pulse(at.x * 0.013),
        ));
        bundle(&mut entity);
        entity.id()
    };
    let hero = |commands: &mut Commands, class: HeroClass, at: Vec2| {
        character(commands, at, Box::new(move |entity| {
            entity.insert((class, Facing(Vec2::new(0.0, -1.0))));
        }))
    };
    let lead = hero(&mut commands, HeroClass::Knight, origin);
    let followers: Vec<(Entity, Vec2)> = [
        (HeroClass::Sorcerer, Vec2::new(-20.0, -6.0)),
        (HeroClass::Ranger, Vec2::new(20.0, -8.0)),
        (HeroClass::Forgebraise, Vec2::new(0.0, -22.0)),
    ]
    .into_iter()
    .map(|(class, offset)| (hero(&mut commands, class, origin + offset), offset))
    .collect();
    for (kind, offset) in [
        (MonsterKind::Wolf, Vec2::new(52.0, 6.0)),
        (MonsterKind::Goblin, Vec2::new(-60.0, 22.0)),
        (MonsterKind::Skeleton, Vec2::new(92.0, -20.0)),
        (MonsterKind::Ghoul, Vec2::new(-105.0, -12.0)),
        (MonsterKind::Orc, Vec2::new(140.0, 32.0)),
        (MonsterKind::Wolf, Vec2::new(-150.0, 30.0)),
    ] {
        let at = floor_near(&room, origin + offset);
        character(&mut commands, at, Box::new(move |entity| {
            entity.insert(Monster(kind));
        }));
    }
    for (row, offset) in [
        (object::COIN, Vec2::new(40.0, -26.0)),
        (object::POTION, Vec2::new(-44.0, -30.0)),
        (object::KEY, Vec2::new(70.0, 34.0)),
        (object::FOOD, Vec2::new(-80.0, -34.0)),
        (object::CAPTIVE, Vec2::new(110.0, 6.0)),
        (object::INGREDIENT, Vec2::new(-125.0, 12.0)),
    ] {
        let at = floor_near(&room, origin + offset);
        commands.spawn((LevelEntity, Prop(row), Transform::from_translation(at.extend(0.5)), Visibility::default()));
    }

    // La caméra du jeu: orthographique, inclinée, tournée; centrée sur les héros.
    let focus = origin + Vec2::new(0.0, 5.0);
    if let Ok(mut transform) = camera.single_mut() {
        let (elevation, camera_yaw) = (options.tilt.to_radians(), yaw);
        let target = Vec3::new(focus.x, 0.0, -focus.y);
        let towards = Vec3::new(camera_yaw.sin() * elevation.cos(), elevation.sin(), camera_yaw.cos() * elevation.cos());
        *transform = Transform::from_translation(target + towards * 1000.0).looking_at(target, Vec3::Y);
    }
    commands.insert_resource(Scene { room, ground, walls, torches, lead, followers, focus });
}

/// Les touches: Tab (Maj+Tab) change de niveau, haut et bas choisissent un réglage, gauche et droite le changent (Maj: plus vite), Entrée
/// enregistre, Retour arrière rend au niveau les valeurs du code, Échap quitte.
fn input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut level: ResMut<Level>,
    mut selected: ResMut<Selected>,
    mut tuning: ResMut<LightTuning>,
    mut saved: ResMut<Saved>,
    mut repeat: Local<f32>,
    mut message: ResMut<Message>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let next = keys.just_pressed(KeyCode::PageDown) || (keys.just_pressed(KeyCode::Tab) && !shift);
    let previous = keys.just_pressed(KeyCode::PageUp) || (keys.just_pressed(KeyCode::Tab) && shift);
    if next || previous {
        let count = LEVEL_COUNT;
        level.0 = if next { level.0 % count + 1 } else { (level.0 + count - 2) % count + 1 };
        message.0.clear();
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        selected.0 = (selected.0 + 1) % SETTINGS.len();
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        selected.0 = (selected.0 + SETTINGS.len() - 1) % SETTINGS.len();
    }
    let region = level_def(level.0).region;
    let direction = i32::from(keys.pressed(KeyCode::ArrowRight)) - i32::from(keys.pressed(KeyCode::ArrowLeft));
    *repeat -= time.delta_secs();
    let fresh = keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::ArrowLeft);
    if direction != 0 && (fresh || *repeat <= 0.0) {
        *repeat = if fresh { 0.35 } else { 0.06 };
        let setting = &SETTINGS[selected.0];
        let step = if shift { setting.big } else { setting.step };
        let mut light = tuning.light(level.0, region);
        let value = ((setting.get)(&light) + step * direction as f32).max(0.0);
        // Arrondi au pas, pour que les valeurs restent propres.
        (setting.set)(&mut light, (value / setting.step).round() * setting.step);
        tuning.set_level(level.0, light);
    }
    if keys.just_pressed(KeyCode::Enter) {
        message.0 = match tuning.save() {
            Ok(path) => {
                saved.0 = tuning.clone();
                format!("Enregistré (tous les niveaux): {}", path.display())
            }
            Err(error) => format!("Échec de l'enregistrement: {error}"),
        };
    }
    if keys.just_pressed(KeyCode::Backspace) {
        // Le niveau n'a plus de réglage propre: il reprend ceux de sa région (et du code).
        tuning.levels.remove(&level.0);
        message.0 = format!("Niveau {}: valeurs de sa région (Entrée pour enregistrer)", level.0);
    }
    if fresh {
        message.0.clear();
    }
}

/// W A S D (ZQSD sur un clavier français, mêmes touches): les héros se déplacent (sur du sol), leurs lanternes avec eux.
fn walk(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, scene: Option<Res<Scene>>, mut transforms: Query<&mut Transform>) {
    let Some(scene) = scene else {
        return;
    };
    let axis = |plus: KeyCode, minus: KeyCode| f32::from(u8::from(keys.pressed(plus))) - f32::from(u8::from(keys.pressed(minus)));
    let delta = Vec2::new(axis(KeyCode::KeyD, KeyCode::KeyA), axis(KeyCode::KeyW, KeyCode::KeyS));
    if delta == Vec2::ZERO {
        return;
    }
    let step = delta.normalize() * 70.0 * time.delta_secs();
    let Ok(here) = transforms.get(scene.lead).map(|t| t.translation.truncate()) else {
        return;
    };
    let lead = here + step;
    let tile = scene.room.world_to_tile(lead);
    if scene.room.tile(tile.x, tile.y) != Tile::Floor {
        return;
    }
    if let Ok(mut t) = transforms.get_mut(scene.lead) {
        t.translation = lead.extend(0.5);
    }
    for (entity, offset) in &scene.followers {
        if let Ok(mut t) = transforms.get_mut(*entity) {
            t.translation = (lead + *offset).extend(0.5);
        }
    }
}

/// Les personnages respirent: ils sont au repos, avec leur boucle d'animation.
fn pulse(time: Res<Time>, mut cycles: Query<(&mut WalkCycle, &Pulse)>) {
    for (mut cycle, Pulse(phase)) in &mut cycles {
        cycle.pose = (PoseKind::Idle, (time.elapsed_secs() * 0.4 + phase).fract());
    }
}

/// Donne au décor, aux modèles et aux lampes la lumière des réglages du niveau.
fn apply(
    level: Res<Level>,
    tuning: Res<LightTuning>,
    scene: Option<Res<Scene>>,
    transforms: Query<&Transform>,
    mut active: ResMut<ActiveLight>,
    mut mood: ResMut<Mood>,
    mut lights: ResMut<Lights>,
    mut materials: ResMut<Assets<DecorMaterial>>,
) {
    let wanted = tuning.light(level.0, level_def(level.0).region);
    if active.0 != wanted {
        active.0 = wanted;
    }
    let wanted_mood = wanted.mood();
    if mood.ambient != wanted_mood.ambient || mood.sun != wanted_mood.sun {
        *mood = wanted_mood;
    }
    let Some(scene) = scene else {
        return;
    };
    let mut positions: Vec<Vec2> = Vec::new();
    for entity in std::iter::once(scene.lead).chain(scene.followers.iter().map(|(entity, _)| *entity)) {
        if let Ok(transform) = transforms.get(entity) {
            positions.push(transform.translation.truncate());
        }
    }
    let focus = positions.first().copied().unwrap_or(scene.focus);
    let computed = look::lights(&positions, &scene.torches, focus, &wanted);
    for handle in [&scene.ground, &scene.walls] {
        let Some(current) = materials.get(handle) else {
            continue;
        };
        let data = &current.data;
        if data.params.y == computed.count
            && data.light_pos == computed.pos
            && data.light_color == computed.color
            && data.style.z == wanted_mood.ambient
            && data.style.w == wanted_mood.sun
        {
            continue;
        }
        if let Some(mut material) = materials.get_mut(handle) {
            material.data.params.y = computed.count;
            material.data.light_pos = computed.pos;
            material.data.light_color = computed.color;
            material.data.style.z = wanted_mood.ambient;
            material.data.style.w = wanted_mood.sun;
        }
    }
    *lights = computed;
}

/// Le texte du panneau.
fn panel(
    level: Res<Level>,
    tuning: Res<LightTuning>,
    saved: Res<Saved>,
    selected: Res<Selected>,
    message: Res<Message>,
    mut text: Query<&mut Text, With<Panel>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let def = level_def(level.0);
    let region = def.region;
    let light = tuning.light(level.0, region);
    let before = saved.0.light(level.0, region);
    let own = if tuning.levels.contains_key(&level.0) { "réglages propres" } else { "valeurs de la région" };
    // La liste des niveaux, celui qu'on règle entre crochets, et un point sur ceux qui ont leurs propres réglages.
    let strip: String = (1..=LEVEL_COUNT)
        .map(|n| {
            let dot = if tuning.levels.contains_key(&n) { "·" } else { "" };
            if n == level.0 { format!("[{n}]") } else { format!(" {n}{dot} ") }
        })
        .collect();
    let mut out = format!("NIVEAU {} / {}   {}\n{:?}  ({own})\n{strip}\n\n", level.0, LEVEL_COUNT, def.name, region);
    for (i, setting) in SETTINGS.iter().enumerate() {
        let marker = if i == selected.0 { ">" } else { " " };
        let now = (setting.get)(&light);
        let was = (setting.get)(&before);
        let changed = if (now - was).abs() > 1e-4 { " *" } else { "" };
        out.push_str(&format!("{marker} {:<38}{:>6.2}{changed}\n", setting.label, now));
    }
    out.push_str("\nTab / Maj+Tab: autre niveau\n↑↓ choisir   ←→ changer (Maj: vite)\nZQSD/WASD déplacer les héros\nEntrée enregistrer (tous les niveaux)\nRetour arrière: valeurs de la région\nÉchap quitter   (* = non enregistré, · = niveau réglé)");
    if !message.0.is_empty() {
        out.push_str(&format!("\n\n{}", message.0));
    }
    if text.0 != out {
        text.0 = out;
    }
}

/// `LUMIERE_SHOT=<fichier.png>`: prend une capture après quelques secondes et quitte (pour vérifier l'affichage sans clavier).
fn shot(mut commands: Commands, time: Res<Time>, mut taken: Local<bool>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("LUMIERE_SHOT") else {
        return;
    };
    if !*taken && time.elapsed_secs() > 8.0 {
        *taken = true;
        commands.spawn(bevy::render::view::window::screenshot::Screenshot::primary_window()).observe(bevy::render::view::window::screenshot::save_to_disk(path));
    }
    if time.elapsed_secs() > 9.5 {
        exit.write(AppExit::Success);
    }
}
