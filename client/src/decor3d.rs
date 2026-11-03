//! Le monde est une vraie scène 3D, vue de biais par une caméra qui suit la caméra 2D.
//!
//! - Le sol: les blocs de décor déjà assemblés (`art::rebuild_world`) servent de texture, un maillage par bloc.
//! - Les murs: des cubes complets (six faces, la matière projetée sur chacune, calculée par le shader selon la région: briques de
//!   pierre, ou tronc et feuillage). Les faces cachées le sont par la profondeur, pas par la géométrie.
//! - Les sprites du monde (personnages, objets, grands motifs, taches au sol...): dessinés dans la scène, debout ou à plat, pour que la
//!   profondeur règle les masquages entre eux et avec les murs. Seuls la météo et l'interface restent en 2D, par-dessus.
//!
//! Les deux caméras doivent avoir le même réglage d'antialiasing (`Msaa`): sinon Bevy ne leur fait pas partager l'image de la fenêtre,
//! et la dernière écrase l'autre.
//!
//! La vue est inclinée de `ELEVATION`. Elle est orthographique, ou en perspective avec `CASTLE_FOV=<degrés>` (les côtés des cubes
//! deviennent alors visibles). Lumières: un soleil, et jusqu'à `MAX_LIGHTS` lampes.

use bevy::{
    asset::RenderAssetUsages,
    camera::{ClearColorConfig, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    image::ImageSampler,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat},
    shader::ShaderRef,
    sprite::Anchor,
};
use castle_shared::tileart::TILE_CELL;
use bevy::window::PrimaryWindow;
use castle_shared::{Expedition, PartyId, Player, Projectile, Region, Room, TILE_SIZE, Tile};

use crate::LocalPlayer;
use crate::art::{Prop, TileMark, WorldTile, object};

/// Hauteur de la caméra au-dessus de l'horizon, en radians (le sol est vu d'en haut quand elle vaut π/2). Le stick droit de la manette principale la règle.
fn elevation() -> f32 {
    crate::settings::view_tilt().to_radians()
}

/// Angle de la caméra autour du décor, en radians (0: elle est au sud et regarde vers le nord). Le stick droit de la manette principale le règle.
fn yaw() -> f32 {
    crate::settings::view_yaw()
}

/// Part de terrain montrée, 1 pour le cadrage de la version 2D.
fn zoom() -> f32 {
    crate::settings::view_zoom()
}
/// Hauteur d'un mur, en unités du monde.
fn wall_height() -> f32 {
    crate::settings::VIEW_WALL_TILES * TILE_SIZE * crate::firstperson::wall_factor()
}

/// Champ de vision de la caméra en perspective, en degrés (`CASTLE_FOV`), ou `None` pour la vue orthographique.
fn field_of_view() -> Option<f32> {
    static FOV: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    *FOV.get_or_init(|| std::env::var("CASTLE_FOV").ok().and_then(|value| value.parse::<f32>().ok()).filter(|fov| *fov > 1.0 && *fov < 120.0))
}

/// Nombre maximal de lampes par bloc.
const MAX_LIGHTS: usize = 8;
/// Lanterne du héros dirigé: hauteur, rayon, couleur (linéaire).
const LANTERN_HEIGHT: f32 = 22.0;
/// Hauteur de la lumière d'une torche (un peu au-dessus de sa flamme).
const TORCH_LIGHT_HEIGHT: f32 = 21.0;
const LANTERN_COLOR: Vec3 = Vec3::new(1.0, 0.87, 0.68);

mod flame;
mod foliage;
pub(crate) mod water;
pub(crate) mod look;
#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
pub mod preview;
#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
pub mod tuner;

use flame::{FlameMaterial, TorchMark, face_flames, spawn_torches};
use look::{ActiveLight, LightTuning, Lights, Mood, Palette, Torches};

/// Couche de rendu du décor: la caméra 3D des personnages (`live3d`, couche 0) ne doit pas le dessiner dans son atlas, ni la
/// caméra du décor les personnages.
pub const LAYER: usize = 1;

pub struct Decor3dPlugin;

impl Plugin for Decor3dPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((MaterialPlugin::<DecorMaterial>::default(), MaterialPlugin::<BillboardMaterial>::default(), MaterialPlugin::<FlameMaterial>::default(), MaterialPlugin::<water::WaterMaterial>::default()))
            .insert_resource(LightTuning::load())
            .init_resource::<ActiveLight>()
            .init_resource::<Torches>()
            .init_resource::<Lights>()
            .init_resource::<Mood>()
            .add_observer(forget_walls)
            .add_observer(water::forget_water)
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (keep_the_image, build_chunks.after(crate::art::rebuild_world), water::build_water, rebuild_walls_for_view, follow_camera, update_materials, face_flames, apply_mood, drive_lamps).chain(),
            )
            .add_systems(
                Update,
                diag_hide,
            )
            .add_systems(
                Update,
                mirror_sprites.after(crate::art::AnimateCharacters).after(crate::art::animate_props).after(follow_camera).after(update_materials),
            );
    }
}

/// Diagnostic de performance: `CASTLE_HIDE=walls,ground,models,mist` masque ces éléments (pour savoir lesquels coûtent au GPU).
pub(crate) fn hidden_by_diag(what: &str) -> bool {
    static HIDE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HIDE.get_or_init(|| std::env::var("CASTLE_HIDE").unwrap_or_default()).split(',').any(|w| w == what)
}

fn diag_hide(
    mut walls: Query<&mut Visibility, With<Decor3dWalls>>,
    mut ground: Query<&mut Visibility, (With<Decor3d>, Without<Decor3dWalls>, Without<crate::live3d::Live3d>)>,
    mut models: Query<&mut Visibility, (With<crate::live3d::Live3d>, Without<Decor3d>, Without<Decor3dWalls>)>,
) {
    for (what, visibilities) in [("walls", 0), ("ground", 1), ("models", 2)] {
        if !hidden_by_diag(what) {
            continue;
        }
        match visibilities {
            0 => walls.iter_mut().for_each(|mut v| *v = Visibility::Hidden),
            1 => ground.iter_mut().for_each(|mut v| *v = Visibility::Hidden),
            _ => models.iter_mut().for_each(|mut v| *v = Visibility::Hidden),
        }
    }
}

#[derive(Component)]
pub(crate) struct DecorCamera;

/// Un bloc de décor en 3D, et le sprite à plat dont il reprend l'image.
#[derive(Component)]
struct Decor3d {
    /// Le sprite à plat dont le bloc reprend l'image (aucun pour les murs).
    source: Option<Entity>,
    material: Handle<DecorMaterial>,
}

/// Les murs du niveau.
#[derive(Component)]
pub(crate) struct Decor3dWalls;

/// Au retour au titre, le décor de la partie quittée n'est plus là: on n'attend pas la prochaine partie pour le savoir (l'interface de la
/// partie suivante ne se construit qu'une fois ses murs construits).
fn forget_walls(_: On<crate::title::ReturnToTitle>, mut commands: Commands, walls: Query<Entity, With<Decor3dWalls>>) {
    for entity in &walls {
        commands.entity(entity).despawn();
    }
}


/// Les blocs de sprite déjà transformés.
#[derive(Component)]
struct Built;

#[derive(ShaderType, Clone, Debug)]
struct DecorData {
    /// Couleur de teinte du décor (celle du héros dirigé).
    tint: Vec4,
    /// (1 / sin de l'élévation: l'étirement du sol, nombre de lampes, hauteur des murs, côté d'une tuile).
    params: Vec4,
    /// (largeur et hauteur de la salle en tuiles, pixels d'une tuile dans l'art, -).
    room: Vec4,
    /// (0: image du bloc, ou 1 pour les murs (la matière est lue tuile par tuile dans le masque des murs); 2 si le niveau a des murs de
    /// feuillage; lumière ambiante; force du soleil).
    style: Vec4,
    /// Couleurs des murs (linéaires): la première et la seconde variantes de la matière, puis les joints (pierre) ou l'écorce (feuillage).
    color_a: Vec4,
    color_b: Vec4,
    color_c: Vec4,
    /// La seconde matière des murs (le feuillage), pour les niveaux qui ont de la pierre et des arbres.
    color_d: Vec4,
    color_e: Vec4,
    color_f: Vec4,
    /// Lampes: position (x, hauteur, z étiré) et rayon; couleur.
    light_pos: [Vec4; MAX_LIGHTS],
    light_color: [Vec4; MAX_LIGHTS],
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct DecorMaterial {
    #[texture(0)]
    #[sampler(1)]
    image: Handle<Image>,
    #[uniform(2)]
    data: DecorData,
    /// Où sont les murs dans tout le niveau (255 = mur, une tuile par pixel, la ligne 0 est la plus au sud): le shader y cherche
    /// l'ombre des murs sur le sol.
    #[texture(3)]
    #[sampler(4)]
    walls: Handle<Image>,
    /// Le motif du feuillage, cuit une fois (voir `foliage`).
    #[texture(5, dimension = "3d")]
    foliage: Handle<Image>,
}

impl Material for DecorMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/decor3d.wesl".into()
    }

    // Les blocs se recouvrent (lisières, clairières: des blocs aux bords transparents posés sur d'autres): les pixels transparents
    // sont écartés par le shader, et chaque bloc est un peu plus haut que le précédent pour que le plus récent passe devant.
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Mask(0.5)
    }
}

/// L'antialiasing des deux caméras. Coupé sur Apple TV: le rendu y est déjà à moitié de la définition de l'écran, et le MSAA ×4 y
/// coûte trop de bande passante.
pub(crate) const MSAA: Msaa = if cfg!(feature = "tvos") { Msaa::Off } else { Msaa::Sample4 };

/// De combien chaque bloc est plus haut que le précédent, en unités du monde.
const LAYER_LIFT: f32 = 0.01;

/// La caméra 2D dessine par-dessus la 3D: elle ne doit plus effacer l'image. (Elle est créée après le démarrage de ce module.)
fn keep_the_image(mut cameras: Query<&mut Camera, (With<Camera2d>, Changed<Camera>)>) {
    for mut camera in &mut cameras {
        if !matches!(camera.clear_color, ClearColorConfig::None) {
            camera.clear_color = ClearColorConfig::None;
        }
    }
}

/// La direction du soleil du décor (celle du shader `decor3d.wesl`), pour que les modèles éclairés par Bevy aient la même lumière.
const SUN_FROM: Vec3 = Vec3::new(-0.45, 0.75, 0.5);

fn setup(mut commands: Commands) {
    spawn_sun_and_lamps(&mut commands);
    commands.spawn((
        DecorCamera,
        Camera3d::default(),
        MSAA,
        Camera { order: -1, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        Projection::from(OrthographicProjection { near: -2000.0, far: 2000.0, ..OrthographicProjection::default_2d() }),
        Tonemapping::None,
        RenderLayers::layer(LAYER),
        Transform::default(),
    ));
}

/// Le soleil, la lumière ambiante et les lampes des modèles 3D (les héros, les monstres, les objets).
fn spawn_sun_and_lamps(commands: &mut Commands) {
    // Le soleil des matériaux de Bevy, sur la seule couche des modèles (le sol et les murs ont leur propre éclairage, dans leur shader),
    // sans ombres portées (elles coûteraient une passe de plus).
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: false, ..default() },
        Transform::from_translation(SUN_FROM * 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(LAYER),
    ));
    commands.insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 450.0, ..default() });
    // Les lampes des modèles 3D (lanternes des héros, torches): `MAX_LIGHTS` lumières ponctuelles, réglées chaque image par `drive_lamps`
    // avec les mêmes lampes que celles du shader du décor, pour que le sol, les murs et les modèles aient la même lumière.
    for index in 0..MAX_LIGHTS {
        commands.spawn((
            Lamp(index),
            PointLight { intensity: 0.0, shadow_maps_enabled: false, ..default() },
            Transform::default(),
            RenderLayers::layer(LAYER),
        ));
    }
}

/// Position 3D d'un point du monde 2D (x vers la droite, y vers le nord), à la hauteur `height`: le nord est -z.
pub(super) fn to_3d(point: Vec2, height: f32) -> Vec3 {
    Vec3::new(point.x, height, -point.y)
}

/// Transforme chaque nouveau bloc de décor en maillage 3D, et cache le sprite à plat.
fn build_chunks(
    mut commands: Commands,
    room: Option<Res<Room>>,
    mut tiles: Query<(Entity, &mut Visibility, &Sprite, &Transform), (With<TileMark>, Without<Built>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DecorMaterial>>,
    mut images: ResMut<Assets<Image>>,
    built: Query<(Entity, &Decor3d)>,
    walls_entities: Query<Entity, With<Decor3dWalls>>,
    torch_entities: Query<Entity, With<TorchMark>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut flames: ResMut<Assets<FlameMaterial>>,
    tuning: Res<LightTuning>,
    expeditions: Query<&Expedition>,
    alive: Query<(), With<TileMark>>,
) {
    // Les blocs d'un ancien niveau disparaissent avec leurs sprites.
    for (entity, chunk) in &built {
        if chunk.source.is_some_and(|source| !alive.contains(source)) {
            commands.entity(entity).despawn();
        }
    }
    if alive.is_empty() {
        walls_entities.iter().for_each(|entity| commands.entity(entity).despawn());
        torch_entities.iter().for_each(|entity| commands.entity(entity).despawn());
    }
    let Some(room) = room else {
        return;
    };
    if tiles.is_empty() {
        return;
    }
    let expedition = expeditions.iter().last();
    let region = expedition.map(|expedition| expedition.region).unwrap_or(Region::Crypts);
    // Chaque tuile de mur a sa matière (pierre ou arbres): un niveau en mêle quand son fichier change le décor de certaines tuiles.
    let def = expedition.and_then(|expedition| look::level_of(&room, expedition.level));
    let kinds = look::wall_kinds(&room, region, def);
    let walls = images.add(wall_mask(&room, &kinds));
    let foliage_pattern = foliage::pattern(&mut images);
    let palette = look::palette(region, &room, def);
    // Les réglages de lumière de ce niveau (les siens, sinon ceux de sa région).
    let light = tuning.light(expedition.map_or(1, |expedition| expedition.level), region);
    let mood = light.mood();
    let foliage = if kinds.contains(&2) { 2.0 } else { 0.0 };
    let base = |wall: f32| decor_data(&room, wall, foliage, &palette, mood, yaw());
    // La version à plat dessine les blocs dans l'ordre de leurs entités: le dernier est devant.
    let mut order: Vec<Entity> = tiles.iter().map(|(entity, ..)| entity).collect();
    order.sort();
    for (rank, entity) in order.into_iter().enumerate() {
        let Ok((entity, mut visibility, sprite, transform)) = tiles.get_mut(entity) else {
            continue;
        };
        let lift = rank as f32 * LAYER_LIFT;
        let Some(size) = sprite.custom_size else {
            continue;
        };
        let (cols, rows) = ((size.x / TILE_SIZE).round() as u32, (size.y / TILE_SIZE).round() as u32);
        let half = Vec2::new(cols as f32 - 1.0, rows as f32 - 1.0) * TILE_SIZE / 2.0;
        let origin = room.world_to_tile(transform.translation.truncate() - half);
        let mesh = meshes.add(ground_mesh(&room, origin, cols, rows, lift));
        let material = materials.add(DecorMaterial {
            image: sprite.image.clone(),
            // Le sol sait s'il y a des arbres (y du style): il s'assombrit alors au pied des troncs.
            data: base(0.0),
            walls: walls.clone(),
            foliage: foliage_pattern.clone(),
        });
        commands.spawn((Decor3d { source: Some(entity), material: material.clone() }, Mesh3d(mesh), MeshMaterial3d(material), RenderLayers::layer(LAYER), Transform::default()));
        *visibility = Visibility::Hidden;
        commands.entity(entity).insert(Built);
    }
    // Les murs: un seul maillage pour tout le niveau.
    walls_entities.iter().for_each(|entity| commands.entity(entity).despawn());
    torch_entities.iter().for_each(|entity| commands.entity(entity).despawn());
    let material = materials.add(DecorMaterial { image: images.add(Image::default()), data: base(1.0), walls, foliage: foliage_pattern });
    commands.spawn((
        Decor3dWalls,
        Decor3d { source: None, material: material.clone() },
        Mesh3d(meshes.add(walls_mesh(&room))),
        MeshMaterial3d(material),
        RenderLayers::layer(LAYER),
        Transform::default(),
    ));
    // Dans le château, des torches sur les murs de pierre: elles éclairent, et se voient.
    let torches = if mood.darkness() > 0.3 { look::torch_sites(&room, &kinds) } else { Vec::new() };
    spawn_torches(&mut commands, &mut meshes, &mut standard, &mut flames, &torches, yaw());
    commands.insert_resource(mood);
    commands.insert_resource(ActiveLight(light));
    commands.insert_resource(Torches(torches));
}

/// La matière des murs d'une région: (1 pierre ou 2 feuillage, [première variante, seconde variante, joints ou écorce]), en linéaire.
fn wall_style(region: Region) -> (f32, [Vec4; 3]) {
    let linear = |r: f32, g: f32, b: f32| {
        let color = Color::srgb(r, g, b).to_linear();
        Vec4::new(color.red, color.green, color.blue, 1.0)
    };
    match region {
        Region::Forest => (2.0, [linear(0.15, 0.29, 0.14), linear(0.07, 0.16, 0.08), linear(0.22, 0.15, 0.09)]),
        Region::Marsh => (2.0, [linear(0.16, 0.26, 0.15), linear(0.08, 0.15, 0.10), linear(0.17, 0.13, 0.09)]),
        Region::Gardens => (2.0, [linear(0.26, 0.42, 0.20), linear(0.13, 0.27, 0.13), linear(0.22, 0.15, 0.11)]),
        Region::Crypts => (1.0, [linear(0.42, 0.42, 0.46), linear(0.30, 0.30, 0.35), linear(0.15, 0.15, 0.19)]),
        Region::Library => (1.0, [linear(0.48, 0.36, 0.25), linear(0.36, 0.27, 0.19), linear(0.17, 0.12, 0.09)]),
        Region::Forge => (1.0, [linear(0.40, 0.27, 0.24), linear(0.28, 0.19, 0.18), linear(0.12, 0.08, 0.08)]),
        Region::Tower => (1.0, [linear(0.42, 0.40, 0.55), linear(0.30, 0.28, 0.43), linear(0.15, 0.14, 0.22)]),
        Region::Royal => (1.0, [linear(0.64, 0.57, 0.44), linear(0.50, 0.44, 0.34), linear(0.27, 0.22, 0.16)]),
        Region::Approach => (1.0, [linear(0.52, 0.47, 0.41), linear(0.40, 0.36, 0.31), linear(0.21, 0.19, 0.16)]),
    }
}

/// Toute tuile de mur est un bloc de pleine hauteur, y compris celles qui sont loin du sol: leur dessus se voit au-dessus des premières
/// rangées, et il doit être à la bonne hauteur. (Hors de la salle, c'est un mur.)
fn is_raised(room: &Room, x: i32, y: i32) -> bool {
    room.tile(x, y) == Tile::Wall
}

/// Les blocs de mur du niveau, une tuile par pixel (voir `DecorMaterial::walls`): 0 pas de mur, 255 mur de pierre, 170 mur de feuillage.
fn wall_mask(room: &Room, kinds: &[u8]) -> Image {
    let size = room.size();
    let data = kinds.iter().map(|kind| match kind { 1 => 255, 2 => 170, _ => 0 }).collect();
    let mut image = Image::new(
        Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    image
}

/// Un maillage à remplir de rectangles.
#[derive(Default)]
struct Quads {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    uvs: Vec<Vec2>,
    indices: Vec<u32>,
}

impl Quads {
    fn add(&mut self, corners: [Vec3; 4], normal: Vec3, uv: [Vec2; 4]) {
        let base = self.positions.len() as u32;
        self.positions.extend(corners);
        self.normals.extend([normal; 4]);
        self.uvs.extend(uv);
        self.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn into_mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// Coin sud-ouest de la tuile (x, y), en coordonnées du monde.
fn tile_corner(room: &Room, x: i32, y: i32) -> Vec2 {
    room.tile_center(x.max(0) as u32, y.max(0) as u32) - Vec2::splat(TILE_SIZE / 2.0)
}

/// Le sol d'un bloc: un seul rectangle, texturé par l'image du bloc (murs compris: ils restent dessous, cachés par les murs en 3D).
fn ground_mesh(room: &Room, origin: IVec2, cols: u32, rows: u32, lift: f32) -> Mesh {
    let mut quads = Quads::default();
    // L'image du bloc: le coin haut-gauche est (0, 0) et v croît vers le bas; le nord (y croissant) est en haut. `ty` compte depuis le haut.
    let uv_at = |tx: f32, ty: f32| Vec2::new(tx / cols as f32, ty / rows as f32);
    let (min, max) = (tile_corner(room, origin.x, origin.y), tile_corner(room, origin.x + cols as i32, origin.y + rows as i32));
    // Sud-ouest, sud-est, nord-est, nord-ouest, vu de dessus (normale vers le haut).
    quads.add(
        [to_3d(min, lift), to_3d(Vec2::new(max.x, min.y), lift), to_3d(max, lift), to_3d(Vec2::new(min.x, max.y), lift)],
        Vec3::Y,
        [uv_at(0.0, rows as f32), uv_at(cols as f32, rows as f32), uv_at(cols as f32, 0.0), uv_at(0.0, 0.0)],
    );
    quads.into_mesh()
}

/// Hauteur du sol à l'intérieur des blocs de mur: juste au-dessus du sol, pour recouvrir son image.
const MASS_HEIGHT: f32 = 1.5;

/// Les murs de tout le niveau. Chaque tuile de mur est un cube de six faces (on ne génère pas celles qui touchent un autre cube). La matière est calculée par le shader (`decor3d.wesl`) à
/// partir de la position dans le monde et de la normale de la face: les six faces sont de la même matière.
fn walls_mesh(room: &Room) -> Mesh {
    let mut quads = Quads::default();
    let height = wall_height();
    let size = room.size();
    for y in 0..size.y as i32 {
        for x in 0..size.x as i32 {
            if room.tile(x, y) != Tile::Wall {
                continue;
            }
            let sw = tile_corner(room, x, y);
            let (se, ne, nw) = (sw + Vec2::new(TILE_SIZE, 0.0), sw + Vec2::splat(TILE_SIZE), sw + Vec2::new(0.0, TILE_SIZE));
            let none = [Vec2::ZERO; 4];
            // Les huit coins du cube: le pied (hauteur 0) et le haut.
            let (low, high) = (|at: Vec2| to_3d(at, 0.0), |at: Vec2| to_3d(at, height));
            // Le sol à l'intérieur du cube: on le voit à travers les vides du feuillage (entre les troncs).
            let flat = |at: Vec2| to_3d(at, MASS_HEIGHT);
            quads.add([flat(sw), flat(se), flat(ne), flat(nw)], Vec3::Y, none);
            // Dessus et dessous.
            quads.add([high(sw), high(se), high(ne), high(nw)], Vec3::Y, none);
            quads.add([low(nw), low(ne), low(se), low(sw)], Vec3::NEG_Y, none);
            // Les quatre côtés, tournés vers l'extérieur (sens trigonométrique vu de dehors), sauf contre un autre cube.
            if !is_raised(room, x, y - 1) {
                quads.add([low(sw), low(se), high(se), high(sw)], Vec3::Z, none);
            }
            if !is_raised(room, x, y + 1) {
                quads.add([low(ne), low(nw), high(nw), high(ne)], Vec3::NEG_Z, none);
            }
            if !is_raised(room, x + 1, y) {
                quads.add([low(se), low(ne), high(ne), high(se)], Vec3::X, none);
            }
            if !is_raised(room, x - 1, y) {
                quads.add([low(nw), low(sw), high(sw), high(nw)], Vec3::NEG_X, none);
            }
        }
    }
    quads.into_mesh()
}

/// Les murs changent de hauteur quand on passe à la première personne (ou qu'on en revient): le maillage est refait.
fn rebuild_walls_for_view(
    view: Res<crate::firstperson::FirstPerson>,
    mut last: Local<bool>,
    room: Option<Res<Room>>,
    walls: Query<&Mesh3d, With<Decor3dWalls>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if view.on == std::mem::replace(&mut *last, view.on) {
        return;
    }
    if let Some(room) = room {
        for mesh in &walls {
            let _ = meshes.insert(&mesh.0, walls_mesh(&room));
        }
    }
}

/// La caméra 3D suit la caméra 2D: même position, même zoom (en perspective, elle recule jusqu'à voir la même hauteur à l'écran). À la
/// première personne (F4), elle se place plutôt à hauteur des yeux du héros dirigé et regarde dans le sens de la vue.
pub(crate) fn follow_camera(
    flat: Single<(&Transform, &Projection), (With<Camera2d>, Without<DecorCamera>)>,
    mut deep: Single<(&mut Transform, &mut Projection), With<DecorCamera>>,
    window: Single<&Window, With<PrimaryWindow>>,
    first_person: Res<crate::firstperson::FirstPerson>,
    hero: Option<Single<&Transform, (With<LocalPlayer>, Without<DecorCamera>, Without<Camera2d>)>>,
    editor: Option<Res<crate::editor_view::EditorCamera>>,
    room: Option<Res<Room>>,
) {
    let (flat_transform, flat_projection) = *flat;
    // Aperçu de l'éditeur de niveaux: la caméra libre qu'il demande, en perspective.
    if let (Some(camera), Some(room)) = (editor.as_deref(), room.as_deref()) {
        let world = (camera.target - room.size().as_vec2() / 2.0) * TILE_SIZE;
        let target = to_3d(world, 0.0);
        let towards = Vec3::new(camera.yaw.sin() * camera.pitch.cos(), camera.pitch.sin(), camera.yaw.cos() * camera.pitch.cos());
        *deep.0 = Transform::from_translation(target + towards * camera.dist * TILE_SIZE).looking_at(target, Vec3::Y);
        let wanted = PerspectiveProjection { fov: crate::editor_view::FIELD_OF_VIEW.to_radians(), near: 1.0, far: 30000.0, ..default() };
        match &mut *deep.1 {
            Projection::Perspective(perspective) => {
                perspective.fov = wanted.fov;
                perspective.near = wanted.near;
                perspective.far = wanted.far;
            }
            projection => *projection = Projection::Perspective(wanted),
        }
        return;
    }
    // À la première personne: aux yeux du héros, le regard dans le sens de la vue (l'angle de la vue tourne la direction des touches).
    if let (true, Some(hero)) = (first_person.on, &hero) {
        let yaw = yaw();
        let pitch = first_person.pitch.to_radians();
        let forward = Vec3::new(-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
        let eye = to_3d(hero.translation.truncate(), crate::firstperson::EYE_HEIGHT);
        *deep.0 = Transform::from_translation(eye).looking_to(forward, Vec3::Y);
        let wanted = PerspectiveProjection { fov: crate::firstperson::FIELD_OF_VIEW.to_radians(), near: 0.5, far: 6000.0, ..default() };
        match &mut *deep.1 {
            Projection::Perspective(perspective) => {
                perspective.fov = wanted.fov;
                perspective.near = wanted.near;
                perspective.far = wanted.far;
            }
            projection => *projection = Projection::Perspective(wanted),
        }
        return;
    }
    let target = to_3d(flat_transform.translation.truncate(), 0.0);
    let (elevation, yaw) = (elevation(), yaw());
    // La direction de la cible vers la caméra: au sud pour un angle nul, puis tournée autour de la verticale.
    let towards_camera = Vec3::new(yaw.sin() * elevation.cos(), elevation.sin(), yaw.cos() * elevation.cos());
    let Projection::Orthographic(from) = flat_projection else {
        return;
    };
    let scale = from.scale * zoom();
    let distance = match field_of_view() {
        // La hauteur de la vue à l'écran, en unités du monde, à la distance de la cible.
        Some(fov) => window.height() * scale / (2.0 * (fov.to_radians() / 2.0).tan()),
        None => 1000.0,
    };
    *deep.0 = Transform::from_translation(target + towards_camera * distance).looking_at(target, Vec3::Y);
    match (field_of_view(), &mut *deep.1) {
        (Some(fov), projection @ Projection::Orthographic(_)) => {
            *projection = Projection::Perspective(PerspectiveProjection { fov: fov.to_radians(), near: 1.0, far: 6000.0, ..default() });
        }
        (Some(fov), Projection::Perspective(perspective)) => {
            perspective.fov = fov.to_radians();
            perspective.near = 1.0;
        }
        // De retour de la première personne: la vue orthographique reprend.
        (None, projection @ Projection::Perspective(_)) => {
            *projection = Projection::from(OrthographicProjection { near: -2000.0, far: 2000.0, ..OrthographicProjection::default_2d() });
            if let Projection::Orthographic(to) = projection {
                to.scale = scale;
                to.scaling_mode = from.scaling_mode;
            }
        }
        (None, Projection::Orthographic(to)) => {
            to.scale = scale;
            to.scaling_mode = from.scaling_mode;
        }
        _ => {}
    }
}

/// Les paramètres du shader du décor pour une salle. `wall` est 0 pour le sol, 1 pour les murs (la matière de chaque tuile est lue dans le
/// masque des murs); `foliage` vaut 2 si le niveau a des murs de feuillage.
fn decor_data(room: &Room, wall: f32, foliage: f32, palette: &Palette, mood: Mood, yaw: f32) -> DecorData {
    DecorData {
        tint: Vec4::ONE,
        params: Vec4::new(1.0, 0.0, wall_height(), TILE_SIZE),
        // (largeur et hauteur de la salle en tuiles, pixels d'une tuile dans l'art, angle de la vue en radians).
        room: Vec4::new(room.size().x as f32, room.size().y as f32, TILE_CELL as f32, yaw),
        style: Vec4::new(wall, foliage, mood.ambient, mood.sun),
        color_a: palette.stone[0],
        color_b: palette.stone[1],
        color_c: palette.stone[2],
        color_d: palette.foliage[0],
        color_e: palette.foliage[1],
        color_f: palette.foliage[2],
        light_pos: [Vec4::ZERO; MAX_LIGHTS],
        light_color: [Vec4::ZERO; MAX_LIGHTS],
    }
}

/// Teinte du décor (celle des sprites à plat) et lampes (les lanternes des héros de l'équipe, et les torches les plus proches).
fn update_materials(
    mut materials: ResMut<Assets<DecorMaterial>>,
    chunks: Query<&Decor3d>,
    sprites: Query<&Sprite, With<TileMark>>,
    local: Option<Single<(Entity, &Transform), With<LocalPlayer>>>,
    heroes: Query<(Entity, &Transform, &PartyId), With<Player>>,
    party: Option<Res<crate::LocalParty>>,
    torches: Res<Torches>,
    active: Res<ActiveLight>,
    mut scene: ResMut<Lights>,
    mood: Res<Mood>,
    mut waters: ResMut<Assets<water::WaterMaterial>>,
    water_surfaces: Query<&MeshMaterial3d<water::WaterMaterial>, With<water::WaterSurface>>,
) {
    // La teinte du décor (celle du héros dirigé) est la même pour tous les blocs, murs compris.
    let tint = sprites.iter().next().map_or(Vec4::ONE, |sprite| {
        let tint = sprite.color.to_linear();
        Vec4::new(tint.red, tint.green, tint.blue, 1.0)
    });
    let wall = wall_height();
    let angle = yaw();
    // Les lampes sont données dans le monde 2D (x, hauteur, y): le shader a besoin de distances non étirées. D'abord le héros que l'on
    // dirige, puis le reste de son équipe (dans l'ordre des entités, pour que les lampes ne changent pas de place d'une image à l'autre).
    let mut positions: Vec<Vec2> = Vec::new();
    if let Some(local) = &local {
        positions.push(local.1.translation.truncate());
    }
    if let Some(party) = party {
        let mut team: Vec<(Entity, Vec2)> = heroes
            .iter()
            .filter(|(entity, _, id)| id.0 == party.0 && local.as_ref().is_none_or(|local| local.0 != *entity))
            .map(|(entity, transform, _)| (entity, transform.translation.truncate()))
            .collect();
        team.sort_by_key(|(entity, _)| *entity);
        positions.extend(team.into_iter().map(|(_, at)| at));
    }
    let focus = positions.first().copied().unwrap_or(Vec2::ZERO);
    let lights = look::lights(&positions, &torches.0, focus, &active.0);
    let (light_pos, light_color, count) = (lights.pos, lights.color, lights.count);
    *scene = lights;
    // L'eau reçoit la même lumière que le sol: teinte, ambiance, soleil et lampes.
    for surface in &water_surfaces {
        let wanted = water::WaterData {
            tint,
            params: Vec4::new(count, 0.0, mood.ambient, mood.sun),
            light_pos,
            light_color,
        };
        if waters.get(&surface.0).is_some_and(|current| current.data != wanted) {
            if let Some(mut material) = waters.get_mut(&surface.0) {
                material.data = wanted;
            }
        }
    }
    for chunk in &chunks {
        // `get_mut` marque la matière modifiée, donc renvoyée au GPU à chaque image: on n'y touche que si quelque chose change.
        let Some(current) = materials.get(&chunk.material) else {
            continue;
        };
        let data = &current.data;
        if data.tint == tint
            && data.params.y == count
            && data.params.z == wall
            && data.room.w == angle
            && data.light_pos == light_pos
            && data.light_color == light_color
        {
            continue;
        }
        let Some(mut material) = materials.get_mut(&chunk.material) else {
            continue;
        };
        material.data.tint = tint;
        material.data.params.y = count;
        material.data.params.z = wall;
        material.data.room.w = angle;
        material.data.light_pos = light_pos;
        material.data.light_color = light_color;
    }
}

/// Une lampe des modèles 3D: l'indice de la lampe du shader dont elle reprend la position et la couleur.
#[derive(Component)]
struct Lamp(usize);

/// Rapport entre l'intensité d'une lampe du shader et la puissance de la lumière de Bevy (lumens): à mi-rayon, une lampe d'intensité `w`
/// éclaire un modèle comme sa lampe du shader éclaire le sol, avec des matières de modèles plus claires que le sol (mesuré sur l'aperçu):
/// un ton de 1 sur le sol vaut environ 1100 lux sur un modèle.
const LUMENS_PER_UNIT: f32 = 150.0 * std::f32::consts::PI;

/// Le soleil et la lumière ambiante des modèles 3D suivent l'ambiance du niveau (très sombre dans les bois à la nuit tombante et dans le
/// château), comme le décor.
fn apply_mood(mood: Res<Mood>, mut sun: Query<&mut DirectionalLight>, mut ambient: ResMut<GlobalAmbientLight>) {
    if !mood.is_changed() {
        return;
    }
    // Le ton d'une surface horizontale au soleil, comme sur le sol (1 en plein jour); de nuit, les modèles s'alignent sur le sol (leurs
    // matières sont plus claires que le sol: sans ce facteur, ils resteraient bien plus vifs que lui).
    let tone = mood.ambient + (1.0 - mood.ambient) * mood.sun;
    let day = ((tone - 0.5) / 0.5).clamp(0.0, 1.0);
    let night_factor = 0.08 + 0.92 * day * day * (3.0 - 2.0 * day);
    for mut light in &mut sun {
        light.illuminance = 9000.0 * tone * night_factor;
    }
    ambient.brightness = 450.0 * tone * night_factor;
}

/// Les lampes de Bevy reprennent celles du shader: lanternes des héros, torches proches.
fn drive_lamps(scene: Res<Lights>, mut lamps: Query<(&Lamp, &mut PointLight, &mut Transform)>) {
    for (Lamp(index), mut light, mut transform) in &mut lamps {
        let (intensity, range, at, color) = if (*index as f32) < scene.count {
            let (pos, color) = (scene.pos[*index], scene.color[*index]);
            (LUMENS_PER_UNIT * color.w * pos.w * pos.w, pos.w * 1.8, to_3d(Vec2::new(pos.x, pos.z), pos.y), Color::linear_rgb(color.x, color.y, color.z))
        } else {
            (0.0, 1.0, Vec3::ZERO, Color::BLACK)
        };
        if light.intensity != intensity || transform.translation != at {
            light.intensity = intensity;
            light.range = range;
            light.color = color;
            transform.translation = at;
        }
    }
}

/// Un sprite trié (héros, monstre, grand motif) dont le panneau 3D remplace l'affichage à plat.
#[derive(Component)]
struct Mirror {
    billboard: Entity,
    /// Taille d'origine du sprite (on lui donne une taille nulle pour ne plus le dessiner à plat).
    size: Vec2,
}

/// Le panneau 3D d'un sprite.
#[derive(Component)]
struct Billboard {
    source: Entity,
}

#[derive(ShaderType, Clone, Debug, PartialEq)]
struct BillboardData {
    rect: Vec4,
    tint: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct BillboardMaterial {
    #[texture(0)]
    #[sampler(1)]
    image: Handle<Image>,
    #[uniform(2)]
    data: BillboardData,
}

impl Material for BillboardMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/billboard3d.wesl".into()
    }

    // Les bords des sprites sont adoucis (MSAA des personnages en 3D): ils se mélangent au décor derrière.
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// Zone de texture (u0, v0, u1, v1) que montre un sprite, ou `None` tant que son image n'est pas chargée.
fn sprite_rect(sprite: &Sprite, images: &Assets<Image>, layouts: &Assets<TextureAtlasLayout>) -> Option<Vec4> {
    let (area, whole) = match &sprite.texture_atlas {
        Some(atlas) => {
            let layout = layouts.get(&atlas.layout)?;
            (layout.textures.get(atlas.index)?.as_rect(), layout.size.as_vec2())
        }
        None => {
            let whole = images.get(&sprite.image)?.size().as_vec2();
            (sprite.rect.unwrap_or(Rect::from_corners(Vec2::ZERO, whole)), whole)
        }
    };
    let (min, max) = (area.min / whole, area.max / whole);
    let (u0, u1) = if sprite.flip_x { (max.x, min.x) } else { (min.x, max.x) };
    let (v0, v1) = if sprite.flip_y { (max.y, min.y) } else { (min.y, max.y) };
    Some(Vec4::new(u0, v0, u1, v1))
}

/// Taille d'un sprite sans taille imposée (`custom_size`): celle de sa zone de texture, un pixel valant une unité du monde.
fn natural_size(sprite: &Sprite, images: &Assets<Image>, layouts: &Assets<TextureAtlasLayout>) -> Option<Vec2> {
    match &sprite.texture_atlas {
        Some(atlas) => Some(layouts.get(&atlas.layout)?.textures.get(atlas.index)?.size().as_vec2()),
        None => Some(sprite.rect.map_or_else(|| images.get(&sprite.image).map(|image| image.size().as_vec2()), |rect| Some(rect.size()))?),
    }
}

/// Dessine tous les sprites du monde dans la scène 3D, pour que la profondeur règle ce qui en cache quoi (un mur devant un héros,
/// un héros devant un rocher...): la caméra 2D dessine après la 3D, sans profondeur commune, donc un sprite à plat passerait toujours
/// par-dessus un mur. Seuls les sprites sans parent (la météo est rattachée à la caméra) sont repris.
///
/// - Debout (par défaut): un panneau vertical de la hauteur du sprite à l'écran (une verticale est raccourcie de `cos(élévation)`),
///   dont le bas touche le sol à la position du sprite.
/// - À plat (taches et fissures du sol, marques de gore): un rectangle posé sur le sol.
/// - Contre un mur (les panneaux des énigmes): collé sur la face sud du mur où ils sont posés.
/// - En vol (flèches, orbes, carreaux): un rectangle face à la caméra, tourné pour que le projectile pointe, à l'écran, dans le sens
///   où il avance.
#[allow(clippy::too_many_arguments)]
fn mirror_sprites(
    mut commands: Commands,
    mut sources: Query<
        (
            Entity,
            &mut Sprite,
            &Transform,
            Option<&Anchor>,
            &InheritedVisibility,
            Option<&mut Mirror>,
            Has<WorldTile>,
            (Option<&Prop>, Option<&Projectile>, Option<&crate::Lift>),
        ),
        (Without<TileMark>, Without<ChildOf>, Without<Billboard>),
    >,
    camera: Single<&Transform, (With<DecorCamera>, Without<Billboard>, Without<Mirror>)>,
    room: Option<Res<Room>>,
    billboards: Query<(Entity, &Billboard)>,
    mut placed: Query<(&mut Transform, &mut Visibility, &MeshMaterial3d<BillboardMaterial>), (With<Billboard>, Without<Mirror>)>,
    images: Res<Assets<Image>>,
    layouts: Res<Assets<TextureAtlasLayout>>,
    mut materials: ResMut<Assets<BillboardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut quad: Local<Option<Handle<Mesh>>>,
    lights: Res<Lights>,
    mood: Res<Mood>,
) {
    // Les panneaux dont le sprite a disparu.
    for (entity, billboard) in &billboards {
        if sources.get(billboard.source).is_err() {
            commands.entity(entity).despawn();
        }
    }
    let quad = quad.get_or_insert_with(|| meshes.add(Rectangle::new(1.0, 1.0))).clone();
    for (entity, mut sprite, transform, anchor, visible, mirror, decal, (prop, projectile, lift)) in &mut sources {
        // La taille d'origine: celle que le sprite porte, ou celle que nous avons gardée quand nous l'avons mis à zéro.
        let size = match (sprite.custom_size, mirror.as_deref()) {
            (Some(size), _) if size != Vec2::ZERO => size,
            (_, Some(mirror)) if sprite.custom_size == Some(Vec2::ZERO) => mirror.size,
            (None, _) => match natural_size(&sprite, &images, &layouts) {
                Some(size) => size,
                None => continue,
            },
            _ => continue,
        };
        let Some(rect) = sprite_rect(&sprite, &images, &layouts) else {
            continue;
        };
        let tint = sprite.color.to_linear();
        // Le centre du sprite dans le monde 2D, et son bas.
        let center = transform.translation.truncate() - anchor.map_or(Vec2::ZERO, |anchor| anchor.as_vec()) * size;
        // Un sprite ne brille pas de lui-même: il n'a que la lumière de l'ambiance et des lampes (un tir en vol, lui, est une lumière).
        let lit = if projectile.is_some() {
            Vec3::ONE
        } else {
            look::light_on(Vec3::new(center.x, if decal { 0.3 } else { size.y * 0.5 }, center.y), decal, &lights, *mood)
        };
        let data = BillboardData { rect, tint: Vec4::new(tint.red * lit.x, tint.green * lit.y, tint.blue * lit.z, tint.alpha) };
        let place = if decal {
            // À plat sur le sol, un peu au-dessus (plus haut pour les marques que le jeu dessine plus tard).
            Transform {
                translation: Vec3::new(center.x, 0.3 + transform.translation.z * 0.05, -center.y),
                rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2) * Quat::from_rotation_z(transform.rotation.to_euler(EulerRot::ZYX).0),
                scale: Vec3::new(size.x, size.y, 1.0),
            }
        } else if let Some(shot) = projectile {
            // En vol: au-dessus du sol d'autant que le jeu le soulève à l'écran, et face à la caméra. Le sprite est dessiné vers la
            // droite: on le tourne de l'angle que fait, à l'écran, la direction du tir (le sol est raccourci de sin(élévation)).
            let raised = lift.map_or(0.0, |lift| lift.0);
            let ground = transform.translation.truncate() - Vec2::new(0.0, raised);
            let (right, up) = (Vec2::new(yaw().cos(), yaw().sin()), Vec2::new(-yaw().sin(), yaw().cos()));
            let on_screen = Vec2::new(shot.dir.dot(right), shot.dir.dot(up) * elevation().sin());
            Transform {
                translation: Vec3::new(ground.x, raised / elevation().cos(), -ground.y),
                rotation: camera.rotation * Quat::from_rotation_z(on_screen.to_angle()),
                scale: Vec3::new(size.x, size.y, 1.0),
            }
        } else if let Some(mount) = prop
            .filter(|prop| (object::WALL_RUNE..=object::WALL_TARGET_HIT).contains(&prop.0))
            .and_then(|_| room.as_deref())
            .and_then(|room| {
                let tile = room.world_to_tile(transform.translation.truncate());
                (room.tile(tile.x, tile.y) == Tile::Wall).then(|| tile_corner(room, tile.x, tile.y).y)
            })
        {
            // Posé dans un mur: collé sur sa face sud, à mi-hauteur, comme une plaque. Il tourne avec le mur, pas avec la caméra.
            Transform {
                translation: Vec3::new(center.x, wall_height() / 2.0, -mount + 0.3),
                scale: Vec3::new(size.x, size.y, 1.0),
                ..default()
            }
        } else {
            // Debout sur le sol à la position du sprite dans le monde (celle des collisions): c'est elle qui se compare aux murs. Le panneau
            // fait face à la caméra, quel que soit son angle.
            let height = size.y / elevation().cos();
            Transform {
                translation: Vec3::new(center.x, height / 2.0, -transform.translation.y),
                rotation: Quat::from_rotation_y(yaw()),
                scale: Vec3::new(size.x, height, 1.0),
            }
        };
        let visibility = if visible.get() { Visibility::Inherited } else { Visibility::Hidden };
        match mirror {
            Some(mut mirror) => {
                mirror.size = size;
                if let Ok((mut at, mut shown, handle)) = placed.get_mut(mirror.billboard) {
                    *at = place;
                    if *shown != visibility {
                        *shown = visibility;
                    }
                    let current = materials.get(&handle.0).map(|material| (material.data.clone(), material.image.clone()));
                    if current.as_ref().is_none_or(|(old, image)| *old != data || *image != sprite.image) {
                        if let Some(mut material) = materials.get_mut(&handle.0) {
                            material.data = data;
                            material.image = sprite.image.clone();
                        }
                    }
                }
            }
            None => {
                let material = materials.add(BillboardMaterial { image: sprite.image.clone(), data });
                let billboard = commands
                    .spawn((Billboard { source: entity }, Mesh3d(quad.clone()), MeshMaterial3d(material), RenderLayers::layer(LAYER), place, visibility))
                    .id();
                commands.entity(entity).insert(Mirror { billboard, size });
            }
        }
        if sprite.custom_size != Some(Vec2::ZERO) {
            sprite.custom_size = Some(Vec2::ZERO);
        }
    }
}
