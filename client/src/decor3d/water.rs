//! L'eau et les ponts des niveaux. L'eau est une surface plate posée sur chaque tuile d'eau, dessinée par un shader
//! (`assets/shaders/water.wesl`) qui l'anime (vagues, reflets, éclats, écume sur les rives). Le sol cuit en dessous garde sa propre eau peinte
//! (utile sous les ponts), mais on ne la voit plus. Les ponts sont des ponts à corde en 3D: des planches, des poteaux aux bords qui longent
//! l'eau et deux cordes de garde-corps, dans les couleurs de l'objet « pont cassé » (`sprite_forge`).

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::RenderLayers,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};
use castle_shared::{ExitKind, LevelExit, Position, Room, TILE_SIZE, Tile};

use super::look::Mood;
use super::{Decor3dWalls, LAYER, MAX_LIGHTS, to_3d};

/// Hauteur de la surface au-dessus du sol, en unités du monde: juste assez pour recouvrir l'eau peinte du sol.
const SURFACE_HEIGHT: f32 = 0.3;

/// Ce que le shader de l'eau reçoit: la lumière du niveau, la même que celle du sol (voir `DecorData`).
#[derive(ShaderType, Clone, Debug, PartialEq)]
pub struct WaterData {
    /// La teinte du niveau (celle des sprites à plat).
    pub tint: Vec4,
    /// (nombre de lampes, -, ambiance du niveau, soleil).
    pub params: Vec4,
    pub light_pos: [Vec4; MAX_LIGHTS],
    pub light_color: [Vec4; MAX_LIGHTS],
}

impl WaterData {
    pub fn new(mood: Mood) -> Self {
        Self { tint: Vec4::ONE, params: Vec4::new(0.0, 0.0, mood.ambient, mood.sun), light_pos: [Vec4::ZERO; MAX_LIGHTS], light_color: [Vec4::ZERO; MAX_LIGHTS] }
    }
}

/// La matière de l'eau: éclairée comme le sol, elle ne brille pas d'elle-même.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub data: WaterData,
}

impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wesl".into()
    }
}

/// La surface d'eau d'un niveau.
#[derive(Component)]
pub struct WaterSurface;

/// Les ponts à corde d'un niveau (tous en un seul maillage).
#[derive(Component)]
pub struct BridgeDeck;

/// Les tuiles percées par un pont cassé (le trou béant): le tablier y manque et l'eau se voit en dessous.
type Holes = Vec<(i32, i32)>;

/// Cette tuile est-elle de l'eau (ou un trou dans un pont, au-dessus de l'eau) ?
fn is_water_like(room: &Room, holes: &Holes, x: i32, y: i32) -> bool {
    room.tile(x, y) == Tile::Water || holes.contains(&(x, y))
}

/// Les rives d'une tuile d'eau (ou de trou): ses voisins qui ne sont pas de l'eau. L'eau passe sous un pont: un pont n'est pas une rive pour
/// l'eau, mais il en est une pour un trou, dont il fait le bord.
fn is_shore(room: &Room, holes: &Holes, x: i32, y: i32, from_hole: bool) -> bool {
    if is_water_like(room, holes, x, y) {
        return false;
    }
    from_hole || room.tile(x, y) != Tile::Bridge
}

/// Le maillage de toute l'eau d'un niveau, une tuile par rectangle. Les rives d'une tuile sont codées dans ses coordonnées de texture (voir
/// `water.wesl`).
fn water_mesh(room: &Room, holes: &Holes) -> Option<Mesh> {
    let size = room.size();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for y in 0..size.y as i32 {
        for x in 0..size.x as i32 {
            if !is_water_like(room, holes, x, y) {
                continue;
            }
            let hole = holes.contains(&(x, y));
            let corner = room.tile_center(x as u32, y as u32) - Vec2::splat(TILE_SIZE / 2.0);
            // Le sud (y du monde plus petit) est vers +z: v = 0 au nord, 1 au sud.
            let (x0, x1, y0, y1) = (corner.x, corner.x + TILE_SIZE, corner.y, corner.y + TILE_SIZE);
            let shore = |dx: i32, dy: i32| f32::from(is_shore(room, holes, x + dx, y + dy, hole));
            let west_east = shore(-1, 0) + 2.0 * shore(1, 0);
            let north_south = shore(0, 1) + 2.0 * shore(0, -1);
            let (ou, ov) = (2.0 * west_east, 2.0 * north_south);
            let base = positions.len() as u32;
            for (px, py, u, v) in [(x0, y1, 0.0, 0.0), (x1, y1, 1.0, 0.0), (x1, y0, 1.0, 1.0), (x0, y0, 0.0, 1.0)] {
                positions.push(to_3d(Vec2::new(px, py), SURFACE_HEIGHT).to_array());
                uvs.push([u + ou, v + ov]);
            }
            indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
    if positions.is_empty() {
        return None;
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Some(
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
            .with_inserted_indices(Indices::U32(indices)),
    )
}

/// Des boîtes (planches, poteaux, cordes) réunies en un seul maillage, avec leur couleur dans les sommets.
#[derive(Default)]
struct Boxes {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Boxes {
    fn add(&mut self, center: Vec3, half: Vec3, rgb: u32) {
        let color = Color::srgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8).to_linear().to_f32_array();
        // Six faces: sa normale, puis ses deux axes de plan.
        for (normal, u, v) in [
            (Vec3::X, Vec3::Y, Vec3::Z),
            (Vec3::NEG_X, Vec3::Z, Vec3::Y),
            (Vec3::Y, Vec3::Z, Vec3::X),
            (Vec3::NEG_Y, Vec3::X, Vec3::Z),
            (Vec3::Z, Vec3::X, Vec3::Y),
            (Vec3::NEG_Z, Vec3::Y, Vec3::X),
        ] {
            let base = self.positions.len() as u32;
            for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                self.positions.push((center + (normal + u * a + v * b) * half).to_array());
                self.normals.push(normal.to_array());
                self.colors.push(color);
            }
            self.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    fn into_mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// Les couleurs de l'objet « pont cassé » et de son bois (voir `sprite_forge::props`).
const PLANKS: [u32; 3] = [0x7a5232, 0x4a3220, 0x9a6c40];
const POST: u32 = 0x4a3220;
const ROPE: u32 = 0xb89860;
/// Dimensions d'un pont à corde, en unités du monde: dessus des planches, hauteur des poteaux, hauteur des deux cordes.
const DECK_TOP: f32 = 0.6;
const POST_HEIGHT: f32 = 11.0;
const ROPE_HEIGHTS: [f32; 2] = [9.5, 6.0];

/// Le maillage de tous les ponts d'un niveau: par tuile de pont, des planches en travers du sens de la marche (le long de l'eau, si elle le
/// borde), et du côté de chaque bord qui longe l'eau, deux cordes tendues entre des poteaux.
fn bridge_mesh(room: &Room, holes: &Holes) -> Option<Mesh> {
    let size = room.size();
    let mut boxes = Boxes::default();
    let mut posts = std::collections::BTreeSet::new();
    let water = |x: i32, y: i32| room.tile(x, y) == Tile::Water;
    for y in 0..size.y as i32 {
        for x in 0..size.x as i32 {
            if room.tile(x, y) != Tile::Bridge {
                continue;
            }
            let corner = room.tile_center(x as u32, y as u32) - Vec2::splat(TILE_SIZE / 2.0);
            let center = to_3d(corner + Vec2::splat(TILE_SIZE / 2.0), 0.0);
            let half = TILE_SIZE / 2.0;
            // De l'eau à l'ouest ou à l'est: le pont va du nord au sud, ses planches sont en travers (longues selon x).
            let north_south = water(x - 1, y) || water(x + 1, y);
            let plank = TILE_SIZE / 4.0;
            // Sous un pont cassé, le tablier manque: l'eau se voit (ses cordes et ses poteaux restent).
            for k in (0..4).filter(|_| !holes.contains(&(x, y))) {
                let offset = -half + plank * (k as f32 + 0.5);
                let tone = PLANKS[((x * 7 + y * 13 + k * 3).unsigned_abs() % 3) as usize];
                let (at, extent) = if north_south {
                    (center + Vec3::new(0.0, DECK_TOP - 0.25, offset), Vec3::new(half, 0.25, plank / 2.0 - 0.15))
                } else {
                    (center + Vec3::new(offset, DECK_TOP - 0.25, 0.0), Vec3::new(plank / 2.0 - 0.15, 0.25, half))
                };
                boxes.add(at, extent, tone);
            }
            // Un garde-corps du côté de chaque bord qui longe l'eau: (voisin, bord vu de dessus: sens du bord et décalage du centre).
            for (nx, ny, along, shift) in [(x, y + 1, Vec3::X, Vec3::new(0.0, 0.0, -half)), (x, y - 1, Vec3::X, Vec3::new(0.0, 0.0, half)), (x - 1, y, Vec3::Z, Vec3::new(-half, 0.0, 0.0)), (x + 1, y, Vec3::Z, Vec3::new(half, 0.0, 0.0))] {
                if !water(nx, ny) {
                    continue;
                }
                for height in ROPE_HEIGHTS {
                    boxes.add(center + shift + Vec3::new(0.0, height, 0.0), Vec3::splat(0.3) + along * (half - 0.3), ROPE);
                }
                // Un poteau à chaque bout du bord (un seul par angle, même si deux tuiles le partagent).
                for end in [-1.0f32, 1.0] {
                    let at = center + shift + along * (end * half);
                    let key = ((at.x * 2.0).round() as i32, (at.z * 2.0).round() as i32);
                    if posts.insert(key) {
                        boxes.add(at + Vec3::new(0.0, POST_HEIGHT / 2.0, 0.0), Vec3::new(0.7, POST_HEIGHT / 2.0, 0.7), POST);
                    }
                }
            }
        }
    }
    if boxes.positions.is_empty() {
        return None;
    }
    Some(boxes.into_mesh())
}

/// Les tuiles percées par les ponts cassés qui ne sont pas réparés (leur sortie est encore fermée).
fn hole_tiles(room: &Room, exits: &Query<(&LevelExit, &Position)>) -> Holes {
    let mut holes: Holes = exits
        .iter()
        .filter(|(exit, _)| exit.kind == ExitKind::Bridge && !exit.open)
        .flat_map(|(_, position)| {
            // La position d'un pont cassé est un coin de tuiles (en coordonnées du monde): le bloc de deux sur deux autour de lui.
            let corner = (position.0 / TILE_SIZE + room.size().as_vec2() / 2.0).round();
            room.bridge_block(corner)
        })
        .collect();
    holes.sort_unstable();
    holes.dedup();
    holes
}

/// À chaque nouveau niveau (ses murs viennent d'être construits), et quand un pont cassé est réparé (son trou se comble), l'eau et les
/// ponts sont refaits.
pub fn build_water(
    mut commands: Commands,
    room: Option<Res<Room>>,
    fresh: Query<(), Added<Decor3dWalls>>,
    exits: Query<(&LevelExit, &Position)>,
    mut last_holes: Local<Holes>,
    old: Query<Entity, Or<(With<WaterSurface>, With<BridgeDeck>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mood: Res<Mood>,
) {
    let Some(room) = room.as_deref() else {
        return;
    };
    let holes = hole_tiles(room, &exits);
    if fresh.is_empty() && holes == *last_holes {
        return;
    }
    *last_holes = holes.clone();
    for entity in &old {
        commands.entity(entity).despawn();
    }
    if let Some(mesh) = water_mesh(room, &holes) {
        commands.spawn((
            WaterSurface,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(WaterMaterial { data: WaterData::new(*mood) })),
            RenderLayers::layer(LAYER),
            Transform::default(),
        ));
    }
    if let Some(mesh) = bridge_mesh(room, &holes) {
        commands.spawn((
            BridgeDeck,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(standard.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.85, ..default() })),
            RenderLayers::layer(LAYER),
            Transform::default(),
        ));
    }
}

/// Au retour au titre, l'eau du niveau quitté disparaît.
pub fn forget_water(_: On<crate::title::ReturnToTitle>, mut commands: Commands, water: Query<Entity, Or<(With<WaterSurface>, With<BridgeDeck>)>>) {
    for entity in &water {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le nombre de sommets d'eau d'une salle sans trou.
    fn bridge_free_water(room: &Room) -> usize {
        water_mesh(room, &Vec::new()).map_or(0, |mesh| mesh.count_vertices())
    }

    /// Une tuile d'eau a quatre sommets et deux triangles; ses rives sont celles de ses voisins qui ne sont ni de l'eau ni un pont.
    #[test]
    fn water_tiles_become_quads_with_their_shores_encoded() {
        let room = Room::from_rows(&["....", ".~~.", ".~=.", "...."]);
        let mesh = water_mesh(&room, &Vec::new()).expect("de l'eau");
        assert_eq!(mesh.count_vertices(), 3 * 4, "trois tuiles d'eau");
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else {
            panic!("des coordonnées de texture");
        };
        // Toute tuile garde ses coordonnées locales dans [0, 1] une fois le code des rives retiré.
        for uv in uvs {
            let (kx, ky) = ((uv[0] / 2.0).floor(), (uv[1] / 2.0).floor());
            assert!((0.0..=1.0).contains(&(uv[0] - 2.0 * kx)) && (0.0..=1.0).contains(&(uv[1] - 2.0 * ky)));
        }
        // Un pont à corde: des planches et, du côté de l'eau, des poteaux et des cordes; pas de pont, pas de maillage.
        let river = Room::from_rows(&["..~~..", "..==..", "..==..", "..~~.."]);
        let bridge = bridge_mesh(&river, &Vec::new()).expect("un pont");
        assert!(bridge.count_vertices() > 8 * 24, "des planches, des poteaux et des cordes: {}", bridge.count_vertices());
        assert!(bridge_mesh(&Room::from_rows(&["..~~..", "..~~.."]), &Vec::new()).is_none());
        // Sous un pont cassé, le tablier est percé et l'eau remplit le trou.
        let holes: Holes = vec![(2, 1), (3, 1), (2, 2), (3, 2)];
        let water_with_hole = water_mesh(&river, &holes).unwrap().count_vertices();
        assert_eq!(water_with_hole, bridge_free_water(&river) + 4 * 4, "quatre tuiles de trou de plus");
        assert!(bridge_mesh(&river, &holes).unwrap().count_vertices() < bridge.count_vertices(), "moins de planches");
        // Pas d'eau, pas de maillage.
        assert!(water_mesh(&Room::from_rows(&["...", "..."]), &Vec::new()).is_none());
    }
}
