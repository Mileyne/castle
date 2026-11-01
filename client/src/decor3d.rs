//! Prototype (`CASTLE_DECOR3D=1`): le sol et les murs sont de vraies surfaces en 3D, éclairées par un petit shader, au lieu de blocs
//! d'image posés à plat.
//!
//! Les blocs de décor déjà assemblés (`art::rebuild_world`) servent de texture: chaque bloc devient un maillage (le sol, plus un
//! relief par tuile de mur), dessiné par une caméra 3D placée derrière la caméra 2D. La caméra 2D ne s'efface plus et dessine
//! par-dessus (personnages, objets, interface): le tri par hauteur et les collisions n'ont donc pas à changer.
//!
//! Les deux caméras doivent avoir le même réglage d'antialiasing (`Msaa`): sinon Bevy ne leur fait pas partager l'image de la fenêtre,
//! et la dernière écrase l'autre.
//!
//! La vue est orthographique et inclinée de `ELEVATION`; le sol est étiré en profondeur pour rester à l'échelle 1:1 de la carte 2D
//! (un point (x, y) du monde 2D tombe au même endroit de l'écran). Lumières: un soleil, et jusqu'à `MAX_LIGHTS` lampes.

use bevy::{
    asset::RenderAssetUsages,
    camera::{ClearColorConfig, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    image::ImageSampler,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat},
    shader::ShaderRef,
};
use castle_shared::{Room, TILE_SIZE, Tile};

use crate::LocalPlayer;
use crate::art::WorldTile;

/// Hauteur de la caméra au-dessus de l'horizon, en radians (le sol est vu d'en haut quand elle vaut π/2).
const ELEVATION: f32 = 0.9;
/// Hauteur des murs, en unités du monde. L'art d'une tuile de mur est déjà une face vue en 3/4, et elle doit remplir exactement une
/// tuile à l'écran: la hauteur à l'écran d'une verticale est `hauteur x cos(élévation)`.
fn wall_height() -> f32 {
    TILE_SIZE * 1.02 / ELEVATION.cos()
}
/// Nombre maximal de lampes par bloc.
const MAX_LIGHTS: usize = 8;
/// Lanterne du héros dirigé: hauteur, rayon, couleur (linéaire).
const LANTERN_HEIGHT: f32 = 22.0;
const LANTERN_RADIUS: f32 = 150.0;
const LANTERN_COLOR: Vec3 = Vec3::new(1.0, 0.72, 0.4);

/// Couche de rendu du décor: la caméra 3D des personnages (`live3d`, couche 0) ne doit pas le dessiner dans son atlas, ni la
/// caméra du décor les personnages.
const LAYER: usize = 1;

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
        app.add_plugins(MaterialPlugin::<DecorMaterial>::default())
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (keep_the_image, build_chunks.after(crate::art::rebuild_world), follow_camera, update_materials).chain(),
            );
    }
}

#[derive(Component)]
struct DecorCamera;

/// Un bloc de décor en 3D, et le sprite à plat dont il reprend l'image.
#[derive(Component)]
struct Decor3d {
    source: Entity,
    material: Handle<DecorMaterial>,
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
    /// (largeur et hauteur de la salle en tuiles, -, -).
    room: Vec4,
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

/// Position 3D (étirée en profondeur) d'un point du monde 2D, à la hauteur `height`.
fn to_3d(point: Vec2, height: f32) -> Vec3 {
    Vec3::new(point.x, height, -point.y / ELEVATION.sin())
}

/// Transforme chaque nouveau bloc de décor en maillage 3D, et cache le sprite à plat.
fn build_chunks(
    mut commands: Commands,
    room: Option<Res<Room>>,
    mut tiles: Query<(Entity, &mut Visibility, &Sprite, &Transform), (With<WorldTile>, Without<Built>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DecorMaterial>>,
    mut images: ResMut<Assets<Image>>,
    built: Query<(Entity, &Decor3d)>,
    alive: Query<(), With<WorldTile>>,
) {
    // Les blocs d'un ancien niveau disparaissent avec leurs sprites.
    for (entity, chunk) in &built {
        if !alive.contains(chunk.source) {
            commands.entity(entity).despawn();
        }
    }
    let Some(room) = room else {
        return;
    };
    if tiles.is_empty() {
        return;
    }
    let walls = images.add(wall_mask(&room));
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
        let mesh = meshes.add(chunk_mesh(&room, origin, cols, rows, lift));
        let material = materials.add(DecorMaterial {
            image: sprite.image.clone(),
            data: DecorData {
                tint: Vec4::ONE,
                params: Vec4::new(1.0 / ELEVATION.sin(), 0.0, wall_height(), TILE_SIZE),
                room: Vec4::new(room.size().x as f32, room.size().y as f32, 0.0, 0.0),
                light_pos: [Vec4::ZERO; MAX_LIGHTS],
                light_color: [Vec4::ZERO; MAX_LIGHTS],
            },
            walls: walls.clone(),
        });
        commands.spawn((Decor3d { source: entity, material: material.clone() }, Mesh3d(mesh), MeshMaterial3d(material), RenderLayers::layer(LAYER), Transform::default()));
        *visibility = Visibility::Hidden;
        commands.entity(entity).insert(Built);
    }
}

/// Les murs du niveau, une tuile par pixel (voir `DecorMaterial::walls`).
fn wall_mask(room: &Room) -> Image {
    let size = room.size();
    let data = (0..size.y as i32)
        .flat_map(|y| (0..size.x as i32).map(move |x| (x, y)))
        .map(|(x, y)| if room.tile(x, y) == Tile::Wall { 255 } else { 0 })
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

/// Le maillage d'un bloc: le sol en un seul rectangle (texturé par l'image du bloc, murs compris), et, pour chaque tuile de mur qui a du
/// sol devant elle, sa face dressée à la verticale avec la même image. L'art d'un mur est déjà sa face vue en 3/4: la dresser (sans
/// rien poser au-dessus) évite de le dessiner deux fois.
fn chunk_mesh(room: &Room, origin: IVec2, cols: u32, rows: u32, lift: f32) -> Mesh {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut uvs: Vec<Vec2> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut quad = |corners: [Vec3; 4], normal: Vec3, uv: [Vec2; 4]| {
        let base = positions.len() as u32;
        positions.extend(corners);
        normals.extend([normal; 4]);
        uvs.extend(uv);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    let world = |x: i32, y: i32| room.tile_center(x.max(0) as u32, y.max(0) as u32) - Vec2::splat(TILE_SIZE / 2.0);
    // L'image du bloc: le coin haut-gauche est (0, 0) et v croît vers le bas; le nord (y croissant) est en haut. `ty` compte depuis le haut.
    let uv_at = |tx: f32, ty: f32| Vec2::new(tx / cols as f32, ty / rows as f32);
    let (min, max) = (world(origin.x, origin.y), world(origin.x + cols as i32, origin.y + rows as i32));
    // Le sol: sud-ouest, sud-est, nord-est, nord-ouest, vu de dessus (normale vers le haut).
    quad(
        [to_3d(min, lift), to_3d(Vec2::new(max.x, min.y), lift), to_3d(max, lift), to_3d(Vec2::new(min.x, max.y), lift)],
        Vec3::Y,
        [uv_at(0.0, rows as f32), uv_at(cols as f32, rows as f32), uv_at(cols as f32, 0.0), uv_at(0.0, 0.0)],
    );
    for j in 0..rows as i32 {
        for i in 0..cols as i32 {
            let (x, y) = (origin.x + i, origin.y + j);
            if room.tile(x, y) != Tile::Wall {
                continue;
            }
            let sw = world(x, y);
            let se = sw + Vec2::new(TILE_SIZE, 0.0);
            let (u0, u1) = (i as f32, i as f32 + 1.0);
            let (v_top, v_bottom) = ((rows as i32 - 1 - j) as f32, (rows as i32 - j) as f32);
            let cell = [uv_at(u0, v_bottom), uv_at(u1, v_bottom), uv_at(u1, v_top), uv_at(u0, v_top)];
            // La face du sud, visible quand la tuile devant est du sol.
            if room.tile(x, y - 1) == Tile::Floor {
                quad([to_3d(sw, lift), to_3d(se, lift), to_3d(se, wall_height() + lift), to_3d(sw, wall_height() + lift)], Vec3::Z, cell);
            }
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
}

/// La caméra 3D suit la caméra 2D: même position, même zoom.
fn follow_camera(
    flat: Single<(&Transform, &Projection), (With<Camera2d>, Without<DecorCamera>)>,
    mut deep: Single<(&mut Transform, &mut Projection), With<DecorCamera>>,
) {
    let (flat_transform, flat_projection) = *flat;
    let target = to_3d(flat_transform.translation.truncate(), 0.0);
    let towards_camera = Vec3::new(0.0, ELEVATION.sin(), ELEVATION.cos());
    *deep.0 = Transform::from_translation(target + towards_camera * 1000.0).looking_at(target, Vec3::Y);
    if let (Projection::Orthographic(from), Projection::Orthographic(to)) = (flat_projection, &mut *deep.1) {
        to.scale = from.scale;
        to.scaling_mode = from.scaling_mode;
    }
}

/// Teinte du décor (celle des sprites à plat) et lampes (la lanterne du héros dirigé).
fn update_materials(
    mut materials: ResMut<Assets<DecorMaterial>>,
    chunks: Query<&Decor3d>,
    sprites: Query<&Sprite, With<WorldTile>>,
    hero: Option<Single<&Transform, With<LocalPlayer>>>,
) {
    let lantern = hero.map(|hero| (to_3d(hero.translation.truncate(), LANTERN_HEIGHT), hero.translation.truncate()));
    for chunk in &chunks {
        let Some(mut material) = materials.get_mut(&chunk.material) else {
            continue;
        };
        if let Ok(sprite) = sprites.get(chunk.source) {
            let tint = sprite.color.to_linear();
            material.data.tint = Vec4::new(tint.red, tint.green, tint.blue, 1.0);
        }
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
