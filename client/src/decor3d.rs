//! Prototype (`CASTLE_DECOR3D=1`): le monde est une vraie scène 3D, vue de biais par une caméra qui suit la caméra 2D.
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
use castle_shared::{Expedition, Region, Room, TILE_SIZE, Tile};

use crate::LocalPlayer;
use crate::live3d::Direct3d;
use crate::settings::View3d;
use crate::art::{Stain, TileMark, WorldTile};

/// Hauteur de la caméra au-dessus de l'horizon, en radians (le sol est vu d'en haut quand elle vaut π/2). Réglage du joueur.
fn elevation() -> f32 {
    crate::settings::view_value(View3d::Tilt).to_radians()
}

/// Angle de la caméra autour du décor, en radians (0: elle est au sud et regarde vers le nord). Réglage du joueur.
fn yaw() -> f32 {
    crate::settings::view_value(View3d::Angle).to_radians()
}

/// Part de terrain montrée, 1 pour le cadrage de la version 2D. Réglage du joueur.
fn zoom() -> f32 {
    crate::settings::view_value(View3d::Zoom) / 100.0
}
/// Hauteur d'un mur, en unités du monde. Réglage du joueur (en tuiles).
fn wall_height() -> f32 {
    crate::settings::view_value(View3d::Wall) * TILE_SIZE
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
const LANTERN_RADIUS: f32 = 150.0;
const LANTERN_COLOR: Vec3 = Vec3::new(1.0, 0.72, 0.4);

/// Couche de rendu du décor: la caméra 3D des personnages (`live3d`, couche 0) ne doit pas le dessiner dans son atlas, ni la
/// caméra du décor les personnages.
pub const LAYER: usize = 1;

/// Le prototype est-il demandé (`CASTLE_DECOR3D=1`)?
pub fn enabled() -> bool {
    std::env::var_os("CASTLE_DECOR3D").is_some()
}

pub struct Decor3dPlugin;

impl Plugin for Decor3dPlugin {
    fn build(&self, app: &mut App) {
        if !enabled() {
            return;
        }
        app.add_plugins((MaterialPlugin::<DecorMaterial>::default(), MaterialPlugin::<BillboardMaterial>::default()))
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (keep_the_image, apply_view, build_chunks.after(crate::art::rebuild_world), follow_camera, update_materials).chain(),
            )
            .add_systems(Update, mirror_sprites.after(crate::art::AnimateCharacters).after(crate::art::y_sort).after(crate::art::animate_props));
    }
}

#[derive(Component)]
struct DecorCamera;

/// Un bloc de décor en 3D, et le sprite à plat dont il reprend l'image.
#[derive(Component)]
struct Decor3d {
    /// Le sprite à plat dont le bloc reprend l'image (aucun pour les murs).
    source: Option<Entity>,
    material: Handle<DecorMaterial>,
}

/// Les murs du niveau.
#[derive(Component)]
struct Decor3dWalls;

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
    /// (0: image du bloc, 1: murs de pierre, 2: murs de feuillage, -, -).
    style: Vec4,
    /// Couleurs des murs (linéaires): la première et la seconde variantes de la matière, puis les joints (pierre) ou l'écorce (feuillage).
    color_a: Vec4,
    color_b: Vec4,
    color_c: Vec4,
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

fn setup(mut commands: Commands) {
    commands.spawn((
        DecorCamera,
        Camera3d::default(),
        Camera { order: -1, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        Projection::from(OrthographicProjection { near: -2000.0, far: 2000.0, ..OrthographicProjection::default_2d() }),
        Tonemapping::None,
        RenderLayers::layer(LAYER),
        Transform::default(),
    ));
}

/// Position 3D d'un point du monde 2D (x vers la droite, y vers le nord), à la hauteur `height`: le nord est -z.
fn to_3d(point: Vec2, height: f32) -> Vec3 {
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
    }
    let Some(room) = room else {
        return;
    };
    if tiles.is_empty() {
        return;
    }
    let walls = images.add(wall_mask(&room));
    let region = expeditions.iter().last().map(|expedition| expedition.region).unwrap_or(Region::Crypts);
    let base = |style: Vec4, colors: [Vec4; 3]| DecorData {
        tint: Vec4::ONE,
        params: Vec4::new(1.0, 0.0, wall_height(), TILE_SIZE),
        room: Vec4::new(room.size().x as f32, room.size().y as f32, TILE_CELL as f32, 0.0),
        style,
        color_a: colors[0],
        color_b: colors[1],
        color_c: colors[2],
        light_pos: [Vec4::ZERO; MAX_LIGHTS],
        light_color: [Vec4::ZERO; MAX_LIGHTS],
    };
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
            data: base(Vec4::ZERO, [Vec4::ZERO; 3]),
            walls: walls.clone(),
        });
        commands.spawn((Decor3d { source: Some(entity), material: material.clone() }, Mesh3d(mesh), MeshMaterial3d(material), RenderLayers::layer(LAYER), Transform::default()));
        *visibility = Visibility::Hidden;
        commands.entity(entity).insert(Built);
    }
    // Les murs: un seul maillage pour tout le niveau.
    walls_entities.iter().for_each(|entity| commands.entity(entity).despawn());
    let (kind, colors) = wall_style(region);
    let material = materials.add(DecorMaterial { image: images.add(Image::default()), data: base(Vec4::new(kind, 0.0, 0.0, 0.0), colors), walls });
    commands.spawn((
        Decor3dWalls,
        Decor3d { source: None, material: material.clone() },
        Mesh3d(meshes.add(walls_mesh(&room))),
        MeshMaterial3d(material),
        RenderLayers::layer(LAYER),
        Transform::default(),
    ));
}

/// La matière des murs d'une région: (1 pierre ou 2 feuillage, [première variante, seconde variante, joints ou écorce]), en linéaire.
fn wall_style(region: Region) -> (f32, [Vec4; 3]) {
    let linear = |r: f32, g: f32, b: f32| {
        let color = Color::srgb(r, g, b).to_linear();
        Vec4::new(color.red, color.green, color.blue, 1.0)
    };
    match region {
        Region::Forest => (2.0, [linear(0.20, 0.38, 0.18), linear(0.10, 0.22, 0.11), linear(0.24, 0.16, 0.10)]),
        Region::Marsh => (2.0, [linear(0.22, 0.34, 0.20), linear(0.11, 0.20, 0.14), linear(0.18, 0.14, 0.10)]),
        Region::Gardens => (2.0, [linear(0.26, 0.42, 0.20), linear(0.13, 0.27, 0.13), linear(0.22, 0.15, 0.11)]),
        Region::Crypts => (1.0, [linear(0.42, 0.42, 0.46), linear(0.30, 0.30, 0.35), linear(0.15, 0.15, 0.19)]),
        Region::Library => (1.0, [linear(0.48, 0.36, 0.25), linear(0.36, 0.27, 0.19), linear(0.17, 0.12, 0.09)]),
        Region::Forge => (1.0, [linear(0.40, 0.27, 0.24), linear(0.28, 0.19, 0.18), linear(0.12, 0.08, 0.08)]),
        Region::Tower => (1.0, [linear(0.42, 0.40, 0.55), linear(0.30, 0.28, 0.43), linear(0.15, 0.14, 0.22)]),
        Region::Royal => (1.0, [linear(0.64, 0.57, 0.44), linear(0.50, 0.44, 0.34), linear(0.27, 0.22, 0.16)]),
        Region::Approach => (1.0, [linear(0.52, 0.47, 0.41), linear(0.40, 0.36, 0.31), linear(0.21, 0.19, 0.16)]),
    }
}

/// Une tuile de mur qui touche du sol (même en diagonale) est un bloc surélevé; les tuiles plus profondes ne sont qu'un fond de roche.
fn is_raised(room: &Room, x: i32, y: i32) -> bool {
    room.tile(x, y) == Tile::Wall && (-1..=1).any(|dy| (-1..=1).any(|dx| room.tile(x + dx, y + dy) == Tile::Floor))
}

/// Les blocs de mur du niveau, une tuile par pixel (voir `DecorMaterial::walls`).
fn wall_mask(room: &Room) -> Image {
    let size = room.size();
    let data = (0..size.y as i32)
        .flat_map(|y| (0..size.x as i32).map(move |x| (x, y)))
        .map(|(x, y)| if is_raised(room, x, y) { 255 } else { 0 })
        .collect();
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

/// Hauteur du dessus plat des masses de mur (au-delà de la première rangée): juste au-dessus du sol, pour recouvrir son image.
const MASS_HEIGHT: f32 = 1.5;

/// Les murs de tout le niveau. Une tuile de mur qui touche du sol est un cube de six faces (on ne génère pas celles qui touchent un
/// autre cube); les tuiles plus profondes ne sont qu'un fond de roche, à plat. La matière est calculée par le shader (`decor3d.wesl`) à
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
            if !is_raised(room, x, y) {
                let flat = |at: Vec2| to_3d(at, MASS_HEIGHT);
                quads.add([flat(sw), flat(se), flat(ne), flat(nw)], Vec3::Y, none);
                continue;
            }
            // Les huit coins du cube: le pied (hauteur 0) et le haut.
            let (low, high) = (|at: Vec2| to_3d(at, 0.0), |at: Vec2| to_3d(at, height));
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

/// La caméra 3D suit la caméra 2D: même position, même zoom (en perspective, elle recule jusqu'à voir la même hauteur à l'écran).
fn follow_camera(
    flat: Single<(&Transform, &Projection), (With<Camera2d>, Without<DecorCamera>)>,
    mut deep: Single<(&mut Transform, &mut Projection), With<DecorCamera>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let (flat_transform, flat_projection) = *flat;
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
        (Some(fov), Projection::Perspective(perspective)) => perspective.fov = fov.to_radians(),
        (None, Projection::Orthographic(to)) => {
            to.scale = scale;
            to.scaling_mode = from.scaling_mode;
        }
        _ => {}
    }
}

/// Un changement de la hauteur des murs oblige à refaire le décor: les maillages des murs en dépendent.
fn apply_view(
    mut commands: Commands,
    chunks: Query<Entity, With<Decor3d>>,
    built: Query<Entity, (With<TileMark>, With<Built>)>,
    mut last: Local<Option<f32>>,
) {
    let wall = wall_height();
    if last.replace(wall).is_some_and(|before| before != wall) {
        chunks.iter().for_each(|entity| commands.entity(entity).despawn());
        built.iter().for_each(|entity| {
            commands.entity(entity).remove::<Built>();
        });
    }
}

/// Teinte du décor (celle des sprites à plat) et lampes (la lanterne du héros dirigé).
fn update_materials(
    mut materials: ResMut<Assets<DecorMaterial>>,
    chunks: Query<&Decor3d>,
    sprites: Query<&Sprite, With<TileMark>>,
    hero: Option<Single<&Transform, With<LocalPlayer>>>,
) {
    // La teinte du décor (celle du héros dirigé) est la même pour tous les blocs, murs compris.
    let tint = sprites.iter().next().map_or(Vec4::ONE, |sprite| {
        let tint = sprite.color.to_linear();
        Vec4::new(tint.red, tint.green, tint.blue, 1.0)
    });
    let lantern = hero.map(|hero| (to_3d(hero.translation.truncate(), LANTERN_HEIGHT), hero.translation.truncate()));
    for chunk in &chunks {
        let Some(mut material) = materials.get_mut(&chunk.material) else {
            continue;
        };
        material.data.tint = tint;
        material.data.params.z = wall_height();
        // Les lampes sont données dans le monde 2D (x, hauteur, y): le shader a besoin de distances non étirées.
        material.data.light_pos = [Vec4::ZERO; MAX_LIGHTS];
        material.data.light_color = [Vec4::ZERO; MAX_LIGHTS];
        let mut count = 0.0;
        if let Some((_, flat)) = lantern {
            material.data.light_pos[0] = Vec4::new(flat.x, LANTERN_HEIGHT, flat.y, LANTERN_RADIUS);
            material.data.light_color[0] = LANTERN_COLOR.extend(1.0);
            count = 1.0;
        }
        material.data.params.y = count;
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
fn mirror_sprites(
    mut commands: Commands,
    mut sources: Query<
        (Entity, &mut Sprite, &Transform, Option<&Anchor>, &InheritedVisibility, Option<&mut Mirror>, Has<WorldTile>, Has<Stain>, Has<Direct3d>),
        (Without<TileMark>, Without<ChildOf>, Without<Billboard>),
    >,
    billboards: Query<(Entity, &Billboard)>,
    mut placed: Query<(&mut Transform, &mut Visibility, &MeshMaterial3d<BillboardMaterial>), (With<Billboard>, Without<Mirror>)>,
    images: Res<Assets<Image>>,
    layouts: Res<Assets<TextureAtlasLayout>>,
    mut materials: ResMut<Assets<BillboardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut quad: Local<Option<Handle<Mesh>>>,
) {
    // Les panneaux dont le sprite a disparu.
    for (entity, billboard) in &billboards {
        if sources.get(billboard.source).is_err() {
            commands.entity(entity).despawn();
        }
    }
    let quad = quad.get_or_insert_with(|| meshes.add(Rectangle::new(1.0, 1.0))).clone();
    for (entity, mut sprite, transform, anchor, visible, mirror, decal, stain, direct) in &mut sources {
        // Son modèle 3D est dans la scène: le sprite à plat ne doit plus être dessiné, et n'a pas de panneau.
        if direct {
            if sprite.custom_size != Some(Vec2::ZERO) {
                sprite.custom_size = Some(Vec2::ZERO);
            }
            continue;
        }
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
        let data = BillboardData { rect, tint: Vec4::new(tint.red, tint.green, tint.blue, tint.alpha) };
        // Le centre du sprite dans le monde 2D, et son bas.
        let center = transform.translation.truncate() - anchor.map_or(Vec2::ZERO, |anchor| anchor.as_vec()) * size;
        let place = if decal || stain {
            // À plat sur le sol, un peu au-dessus (plus haut pour les marques que le jeu dessine plus tard).
            Transform {
                translation: Vec3::new(center.x, 0.3 + transform.translation.z * 0.05, -center.y),
                rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2) * Quat::from_rotation_z(transform.rotation.to_euler(EulerRot::ZYX).0),
                scale: Vec3::new(size.x, size.y, 1.0),
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
