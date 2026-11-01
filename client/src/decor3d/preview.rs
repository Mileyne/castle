//! Aperçu du décor 3D sans lancer le jeu: une petite carte d'essai (un bois dense, une haie, un arbre seul, un bosquet, une clairière) rendue
//! avec le vrai shader `decor3d.wesl`, puis enregistrée en image. Le shader se relit au lancement: pour le régler, on le modifie et on
//! relance l'aperçu, sans recompiler.
//!
//! ```sh
//! cargo run -p castle_client --bin decor_preview -- [--region forest|crypts|...] [--out aperçu.png] [--zoom 0.85] [--tilt 55] [--yaw 0]
//!     [--focus X,Y] [--map bois|pierre|arbres|mixte]
//!     [--sol terre|herbe|dense]
//! ```
//! La caméra est celle du jeu par défaut. La taille de la fenêtre fixe celle de l'image.

use bevy::{
    app::AppExit,
    camera::visibility::RenderLayers,
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::view::window::screenshot::{Screenshot, save_to_disk},
    window::WindowResolution,
};
use castle_shared::{Region, Room, TILE_SIZE};

use super::look::{self, Mood, Palette};
use super::flame::{FlameMaterial, spawn_torches};
use super::{DecorMaterial, LAYER, decor_data, ground_mesh, wall_mask, wall_style, walls_mesh};

/// Hauteur de la vue à zoom 1, en unités du monde (celle du jeu, voir `camera.rs`).
const VIEW_HEIGHT: f32 = 240.0;

#[derive(Resource, Clone)]
struct Options {
    region: Option<Region>,
    out: String,
    zoom: f32,
    tilt: f32,
    yaw: f32,
    focus: Option<Vec2>,
    map: String,
    floor: String,
}

pub(super) fn parse_region(name: &str) -> Region {
    match name {
        "marsh" => Region::Marsh,
        "gardens" => Region::Gardens,
        "crypts" => Region::Crypts,
        "library" => Region::Library,
        "forge" => Region::Forge,
        "tower" => Region::Tower,
        "royal" => Region::Royal,
        "approach" => Region::Approach,
        _ => Region::Forest,
    }
}

fn options() -> Options {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let number = |name: &str, default: f32| value(name).and_then(|v| v.parse().ok()).unwrap_or(default);
    let region = value("--region").map(|name| parse_region(&name));
    Options {
        region,
        out: value("--out").unwrap_or_else(|| "decor_preview.png".into()),
        zoom: number("--zoom", crate::settings::VIEW_ZOOM_DEFAULT),
        tilt: number("--tilt", crate::settings::VIEW_TILT_DEFAULT),
        yaw: number("--yaw", crate::settings::VIEW_ANGLE_DEFAULT),
        focus: value("--focus").and_then(|v| {
            let (x, y) = v.split_once(',')?;
            Some(Vec2::new(x.parse().ok()?, y.parse().ok()?))
        }),
        map: value("--map").unwrap_or_default(),
        floor: value("--sol").unwrap_or_else(|| "terre".into()),
    }
}

/// La carte d'essai, du nord (en haut) au sud. `#` est un mur.
pub(super) const MAP: [&str; 24] = [
    "########################################",
    "########################################",
    "#########################...############",
    "###############..........................",
    "#####################.......#####.....###",
    "####################........#####.....###",
    "####..###############....................",
    "###....#############.....###.....########",
    "###....#####################.....########",
    "####..####################..............#",
    "#######################...............###",
    "#.....#################.........##.....##",
    "#.....#################.........##.....##",
    "#.....###########.....................####",
    "#.....##########.......................###",
    "#######........................#####.....#",
    "#######..........###...........#####.....#",
    "#######..........###...........#####.....#",
    "#######........................###########",
    "################.......#################",
    "#################.....##################",
    "######################################## ",
    "########################################",
    "########################################",
];

/// Les arbres un par un, dans une clairière: un seul, une paire, un trio, un bosquet 2 x 2, deux en diagonale, un en L.
const TREES: [&str; 20] = [
    "########################################",
    "#......................................#",
    "#......................................#",
    "#...#.........##.........###.....##....#",
    "#.............................#...#....#",
    "#......................................#",
    "#......................................#",
    "#......................................#",
    "#.....#.........##.........#...........#",
    "#.......................................",
    "#..........................#..#........#",
    "#...........................#..........#",
    "#......................................#",
    "#......................................#",
    "#......................................#",
    "#......................................#",
    "#......................................#",
    "#......................................#",
    "#......................................#",
    "########################################",
];

pub fn run() {
    let options = options();
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
    App::new()
        .insert_resource(options)
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin { file_path: assets.into(), ..default() })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Aperçu du décor".into(),
                        resolution: WindowResolution::new(1280, 800).with_scale_factor_override(1.0),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins((MaterialPlugin::<DecorMaterial>::default(), MaterialPlugin::<FlameMaterial>::default()))
        .add_systems(Startup, setup)
        .add_systems(Update, (capture, super::apply_mood, super::drive_lamps).chain())
        .init_resource::<look::Lights>()
        .init_resource::<Mood>()
        .run();
}

/// Un sol d'essai étiré sur toute la carte: `terre` (brun), `herbe` (vert moyen) ou `dense` (herbe épaisse, vert sombre rayé).
pub(super) fn floor_image(size: UVec2, kind: &str) -> Image {
    let (w, h) = (size.x * 16, size.y * 16);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let n = ((x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663)) % 100) as f32 / 100.0;
            let rgb = match kind {
                "herbe" => {
                    let tone = 0.8 + 0.4 * n;
                    [0.16 * tone, 0.33 * tone, 0.09 * tone]
                }
                // Des traits verticaux serrés: la colonne décide de la longueur du brin.
                "dense" => {
                    let column = (x.wrapping_mul(2654435761) >> 7) % 100;
                    let tone = 0.55 + 0.7 * (((column + y / 3) % 7) as f32 / 7.0) * (0.6 + 0.4 * n);
                    [0.07 * tone, 0.22 * tone, 0.04 * tone]
                }
                _ => {
                    let tone = 0.85 + 0.3 * n;
                    [0.30 * tone, 0.26 * tone, 0.17 * tone]
                }
            };
            data.extend_from_slice(&[(rgb[0] * 255.0).min(255.0) as u8, (rgb[1] * 255.0).min(255.0) as u8, (rgb[2] * 255.0).min(255.0) as u8, 255]);
        }
    }
    let mut image = Image::new(
        bevy::render::render_resource::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = bevy::image::ImageSampler::nearest();
    image
}

fn setup(
    mut commands: Commands,
    options: Res<Options>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DecorMaterial>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut flames: ResMut<Assets<FlameMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let rows: Vec<&str> = if options.map == "arbres" { TREES.to_vec() } else { MAP.iter().map(|row| row.trim_end()).collect() };
    let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
    let padded: Vec<String> = rows.iter().map(|r| format!("{r:#<width$}")).collect();
    let padded: Vec<&str> = padded.iter().map(String::as_str).collect();
    let room = Room::from_rows(&padded);
    let size = room.size();

    // La région du niveau fixe l'ambiance (sombre dans le château), et la matière des murs sauf pour la carte `mixte`: la pierre du chemin
    // du château à gauche, les arbres du marais à droite, comme le niveau 3.
    let region = options.region.unwrap_or(match options.map.as_str() {
        "pierre" => Region::Crypts,
        "mixte" => Region::Approach,
        _ => Region::Forest,
    });
    let (stone_region, foliage_region) = if options.map == "mixte" { (region, Region::Marsh) } else { (region, region) };
    let palette = Palette { stone: wall_style(stone_region).1, foliage: wall_style(foliage_region).1 };
    let uniform = wall_style(region).0 as u8;
    let kinds: Vec<u8> = (0..size.y as i32)
        .flat_map(|y| (0..size.x as i32).map(move |x| (x, y)))
        .map(|(x, y)| match room.tile(x, y) {
            castle_shared::Tile::Wall if options.map == "mixte" => if x * 2 < size.x as i32 { 1 } else { 2 },
            castle_shared::Tile::Wall => uniform,
            _ => 0,
        })
        .collect();
    let mood = Mood::of(region);
    let foliage = if kinds.contains(&2) { 2.0 } else { 0.0 };
    let yaw = options.yaw.to_radians();
    let walls = images.add(wall_mask(&room, &kinds));

    // Le sol, puis les murs: comme dans le jeu.
    let ground = materials.add(DecorMaterial {
        image: images.add(floor_image(size, &options.floor)),
        data: decor_data(&room, 0.0, foliage, &palette, mood, yaw),
        walls: walls.clone(),
    });
    let ground_entity = commands.spawn((Mesh3d(meshes.add(ground_mesh(&room, IVec2::ZERO, size.x, size.y, 0.0))), MeshMaterial3d(ground.clone()), RenderLayers::layer(LAYER))).id();
    let _ = ground_entity;
    let wall_material = materials.add(DecorMaterial {
        image: images.add(Image::default()),
        data: decor_data(&room, 1.0, foliage, &palette, mood, yaw),
        walls,
    });
    commands.spawn((Mesh3d(meshes.add(walls_mesh(&room))), MeshMaterial3d(wall_material.clone()), RenderLayers::layer(LAYER)));

    // Les torches (dans le château seulement), et un repère de la taille d'un héros (8 x 20) dans la clairière, qui porte une lanterne.
    let torches = if mood.darkness() > 0.3 { look::torch_sites(&room, &kinds) } else { Vec::new() };
    spawn_torches(&mut commands, &mut meshes, &mut standard, &mut flames, &torches, yaw);
    let hero = standard.add(StandardMaterial { base_color: Color::srgb(0.9, 0.2, 0.15), unlit: true, ..default() });
    let at = room.tile_center(30, 13);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(8.0, 20.0, 4.0))),
        MeshMaterial3d(hero),
        Transform::from_xyz(at.x, 10.0, -at.y),
        RenderLayers::layer(LAYER),
    ));

    // La caméra du jeu (voir `follow_camera`): orthographique, inclinée, tournée.
    let focus = options.focus.unwrap_or_else(|| room.tile_center(width as u32 / 2, size.y / 2 - 1));
    let lights = look::lights(&[at, at + Vec2::new(-14.0, -8.0), at + Vec2::new(16.0, -10.0)], &torches, focus, &look::LevelLight::default());
    for handle in [ground, wall_material] {
        if let Some(mut material) = materials.get_mut(&handle) {
            material.data.params.y = lights.count;
            material.data.light_pos = lights.pos;
            material.data.light_color = lights.color;
        }
    }
    // Des modèles éclairés par Bevy, comme les ennemis, les pierres et les objets du jeu: un cube gris près du héros et d'autres plus loin.
    // Ils ne reçoivent que l'ambiance du niveau et les lampes (lanternes, torches), comme dans le jeu.
    super::spawn_sun_and_lamps(&mut commands);
    commands.insert_resource(mood);
    commands.insert_resource(look::Lights { pos: lights.pos, color: lights.color, count: lights.count });
    let stone = standard.add(StandardMaterial { base_color: Color::srgb(0.7, 0.7, 0.72), perceptual_roughness: 0.9, ..default() });
    for (dx, dy) in [(-22.0f32, -6.0f32), (-70.0, 20.0), (-130.0, -10.0), (-190.0, 30.0), (60.0, 40.0)] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(10.0, 10.0, 10.0))),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(at.x + dx, 5.0, -(at.y + dy)),
            RenderLayers::layer(LAYER),
        ));
    }
    // Des héros d'essai, loin des lampes: sans lueur propre (un ennemi), avec celle des héros (`HERO_GLOW`), et avec le double.
    for (i, glow) in [0.0f32, 0.07, 0.14].into_iter().enumerate() {
        let emitting = standard.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.72),
            perceptual_roughness: 0.9,
            emissive: LinearRgba::gray(glow),
            ..default()
        });
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(10.0, 10.0, 10.0))),
            MeshMaterial3d(emitting),
            Transform::from_xyz(at.x - 120.0 + i as f32 * 26.0, 5.0, -(at.y + 70.0)),
            RenderLayers::layer(LAYER),
        ));
    }
    let (elevation, yaw) = (options.tilt.to_radians(), options.yaw.to_radians());
    let target = Vec3::new(focus.x, 0.0, -focus.y);
    let towards = Vec3::new(yaw.sin() * elevation.cos(), elevation.sin(), yaw.cos() * elevation.cos());
    let mut projection = OrthographicProjection { near: -2000.0, far: 2000.0, ..OrthographicProjection::default_2d() };
    projection.scale = VIEW_HEIGHT * options.zoom / 800.0;
    commands.spawn((
        Camera3d::default(),
        Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        Projection::from(projection),
        Tonemapping::None,
        RenderLayers::layer(LAYER),
        Transform::from_translation(target + towards * 1000.0).looking_at(target, Vec3::Y),
    ));
    let _ = TILE_SIZE;
}

/// On laisse 6 secondes à tout pour se charger (les shaders se compilent au premier affichage, parfois lentement), puis on prend la
/// capture, et on quitte une seconde après.
fn capture(mut commands: Commands, options: Res<Options>, time: Res<Time>, mut taken: Local<bool>, mut exit: MessageWriter<AppExit>) {
    if !*taken && time.elapsed_secs() > 6.0 {
        *taken = true;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(options.out.clone()));
    }
    if time.elapsed_secs() > 7.5 {
        exit.write(AppExit::Success);
    }
}
