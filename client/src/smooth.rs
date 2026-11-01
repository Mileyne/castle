//! Fusion douce des primitives d'un modèle: un seul corps continu au lieu de pièces qui se traversent.
//!
//! Seules des pièces de même nature se fondent: le corps (peau, visage, cheveux, barbe) d'un côté, la tenue (vêtements, armure,
//! couvre-chef) de l'autre, chacun avec sa grille et son congé. Un casque ne fait pas corps avec un visage, ni une épée avec une main:
//! l'équipement tenu reste en primitives (voir `sprite_forge::LiveLayer`).
//!
//! Chaque pièce est un champ de distance (négatif dedans, positif dehors). Leur union lissée (`smin`) arrondit les jonctions: l'épaule
//! se fond dans le torse, la botte dans la jambe. La surface est ensuite extraite d'une grille par « surface nets » (un sommet par case
//! traversée, des quadrilatères entre cases voisines), avec la normale prise dans le champ de distance (ombrage lisse) et la couleur
//! mélangée entre les pièces voisines.
//!
//! Une grille ne représente pas ce qui est plus fin que quelques cases (lame d'épée, corde d'arc, lanière): ces pièces fines restent des
//! primitives, ajoutées telles quelles au même maillage. La construction coûte quelques dizaines de millisecondes par pose: elle se fait à
//! l'avance (voir `merged::Baking`), jamais en jeu.

use bevy::prelude::*;
use sprite_forge::{LiveLayer, LivePart, LiveShape};

use crate::merged::{Buffers, part_transform, vertex_color};

/// Réglages de fusion d'une couche.
#[derive(Clone, Copy, Debug)]
pub struct Blend {
    /// Rayon du congé entre deux pièces, en mètres: plus il est grand, plus les jonctions fondent.
    pub radius: f32,
    /// Côté d'une case de la grille, en mètres: plus il est petit, plus le détail est fin (et le maillage lourd).
    pub cell: f32,
}

/// Les réglages de chaque couche. Le corps tient dans une petite boîte (la tête, le cou): sa grille peut être très fine, ce qui garde
/// les yeux, le nez et les sourcils; la tenue couvre tout le personnage, avec une grille plus large.
#[derive(Clone, Copy, Debug)]
pub struct Blends {
    pub body: Blend,
    pub outfit: Blend,
}

impl Default for Blends {
    fn default() -> Self {
        Self { body: Blend { radius: 0.012, cell: 0.012 }, outfit: Blend { radius: 0.02, cell: 0.03 } }
    }
}

/// Une pièce prête à être évaluée: sa place, et son champ de distance dans l'espace de la forme de base.
struct Solid {
    shape: LiveShape,
    center: Vec3,
    inverse_rotation: Quat,
    scale: Vec3,
    color: [f32; 3],
    glow: bool,
    /// Rayon d'une sphère qui contient la pièce.
    bound: f32,
}

impl Solid {
    fn new(part: &LivePart) -> Self {
        let place = part_transform(part);
        // La moitié de l'étendue de la forme de base le long de ses axes.
        let half = match part.shape {
            LiveShape::Sphere | LiveShape::Cube => Vec3::ONE,
            LiveShape::Capsule(_) => Vec3::new(1.0, 2.0, 1.0),
            LiveShape::Frustum(a, b) => {
                let radius = a.max(b);
                Vec3::new(radius, 1.0, radius)
            }
        };
        Self {
            shape: part.shape,
            center: place.translation,
            inverse_rotation: place.rotation.inverse(),
            scale: place.scale,
            color: part.color,
            glow: part.glow,
            bound: (place.scale * half).length(),
        }
    }

    /// La pièce est-elle plus fine que `thickness` mètres? Elle reste alors une primitive.
    fn is_thin(part: &LivePart, thickness: f32) -> bool {
        let place = part_transform(part);
        let width = match part.shape {
            LiveShape::Sphere | LiveShape::Cube => 2.0 * place.scale.min_element(),
            LiveShape::Capsule(_) => 2.0 * place.scale.x.min(place.scale.z),
            LiveShape::Frustum(a, b) => 2.0 * place.scale.x.min(place.scale.z) * a.max(b),
        };
        width < thickness
    }

    /// Distance signée du point `p` (monde) à la pièce, à peu près exacte près de la surface.
    ///
    /// La pièce est la forme de base étirée par `scale`: sa distance se ramène du monde à la forme de base en divisant par la norme du
    /// gradient mis à l'échelle (pour une sphère aplatie, la distance dans le sens aplati est plus courte que dans l'autre: la
    /// multiplier par la plus petite échelle la sous-estimerait partout et étalerait les congés sur des centimètres de trop).
    fn distance(&self, p: Vec3) -> f32 {
        let q = (self.inverse_rotation * (p - self.center)) / self.scale;
        let (distance, gradient) = base_distance(self.shape, q);
        distance / (gradient / self.scale).length().max(1e-3)
    }
}

/// Distance signée à la forme de base (rayon 1; capsule et cône le long de y), et son gradient (la direction qui s'éloigne de la
/// surface, de norme 1).
fn base_distance(shape: LiveShape, p: Vec3) -> (f32, Vec3) {
    match shape {
        LiveShape::Sphere => {
            let length = p.length();
            (length - 1.0, if length > 1e-6 { p / length } else { Vec3::Y })
        }
        LiveShape::Cube => {
            let q = p.abs() - Vec3::ONE;
            let outside = q.max(Vec3::ZERO);
            let length = outside.length();
            let signs = Vec3::new(p.x.signum(), p.y.signum(), p.z.signum());
            if length > 1e-6 {
                (length + q.max_element().min(0.0), signs * outside / length)
            } else {
                // Dedans: la direction de la face la plus proche.
                let axis = if q.x >= q.y && q.x >= q.z { Vec3::X } else if q.y >= q.z { Vec3::Y } else { Vec3::Z };
                (q.max_element(), signs * axis)
            }
        }
        LiveShape::Capsule(_) => {
            // Un segment de demi-longueur 1, de rayon 1.
            let along = Vec3::new(p.x, p.y - p.y.clamp(-1.0, 1.0), p.z);
            let length = along.length();
            (length - 1.0, if length > 1e-6 { along / length } else { Vec3::X })
        }
        LiveShape::Frustum(bottom, top) => {
            let distance = capped_cone(p, 1.0, bottom, top);
            let step = 0.01;
            let gradient = Vec3::new(
                capped_cone(p + Vec3::X * step, 1.0, bottom, top) - distance,
                capped_cone(p + Vec3::Y * step, 1.0, bottom, top) - distance,
                capped_cone(p + Vec3::Z * step, 1.0, bottom, top) - distance,
            );
            (distance, if gradient.length() > 1e-6 { gradient.normalize() } else { Vec3::Y })
        }
    }
}

/// Cône tronqué de demi-hauteur `h`, de rayon `r1` en bas (y = -h) et `r2` en haut.
fn capped_cone(p: Vec3, h: f32, r1: f32, r2: f32) -> f32 {
    let q = Vec2::new(Vec2::new(p.x, p.z).length(), p.y);
    let k1 = Vec2::new(r2, h);
    let k2 = Vec2::new(r2 - r1, 2.0 * h);
    let ca = Vec2::new(q.x - q.x.min(if q.y < 0.0 { r1 } else { r2 }), q.y.abs() - h);
    let cb = q - k1 + k2 * (((k1 - q).dot(k2)) / k2.dot(k2)).clamp(0.0, 1.0);
    let sign = if cb.x < 0.0 && ca.y < 0.0 { -1.0 } else { 1.0 };
    sign * ca.dot(ca).min(cb.dot(cb)).sqrt()
}

/// Union lissée polynomiale de deux distances: comme `min`, avec un congé de rayon `k` là où elles se rejoignent.
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// Les pièces rangées par morceaux d'espace, pour n'évaluer en un point que celles qui sont assez près pour compter.
struct Field {
    solids: Vec<Solid>,
    blend: f32,
    origin: Vec3,
    bucket: f32,
    dims: [usize; 3],
    lists: Vec<Vec<u16>>,
}

impl Field {
    fn new(solids: Vec<Solid>, blend: f32) -> Self {
        let reach = |solid: &Solid| solid.bound + blend * 2.0;
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for solid in &solids {
            min = min.min(solid.center - Vec3::splat(reach(solid)));
            max = max.max(solid.center + Vec3::splat(reach(solid)));
        }
        let bucket = 0.25;
        let size = (max - min).max(Vec3::splat(bucket));
        let dims = [(size.x / bucket).ceil() as usize + 1, (size.y / bucket).ceil() as usize + 1, (size.z / bucket).ceil() as usize + 1];
        let mut lists = vec![Vec::new(); dims[0] * dims[1] * dims[2]];
        for (index, solid) in solids.iter().enumerate() {
            let low = ((solid.center - Vec3::splat(reach(solid)) - min) / bucket).floor().max(Vec3::ZERO);
            let high = ((solid.center + Vec3::splat(reach(solid)) - min) / bucket).floor();
            for z in low.z as usize..=(high.z as usize).min(dims[2] - 1) {
                for y in low.y as usize..=(high.y as usize).min(dims[1] - 1) {
                    for x in low.x as usize..=(high.x as usize).min(dims[0] - 1) {
                        lists[(z * dims[1] + y) * dims[0] + x].push(index as u16);
                    }
                }
            }
        }
        Self { solids, blend, origin: min, bucket, dims, lists }
    }

    /// Les pièces qui comptent au point `p`.
    fn near(&self, p: Vec3) -> &[u16] {
        let cell = ((p - self.origin) / self.bucket).floor();
        if cell.min_element() < 0.0 {
            return &[];
        }
        let (x, y, z) = (cell.x as usize, cell.y as usize, cell.z as usize);
        if x >= self.dims[0] || y >= self.dims[1] || z >= self.dims[2] {
            return &[];
        }
        &self.lists[(z * self.dims[1] + y) * self.dims[0] + x]
    }

    /// La distance de l'union lissée au point `p` (positive et grande hors de portée de toute pièce).
    fn distance(&self, p: Vec3) -> f32 {
        let mut distance = f32::MAX;
        for index in self.near(p) {
            let d = self.solids[*index as usize].distance(p);
            distance = if distance == f32::MAX { d } else { smooth_min(distance, d, self.blend) };
        }
        if distance == f32::MAX { 1.0 } else { distance }
    }

    /// Couleur, éclat et reflet au point `p`: le mélange des pièces voisines, pondérées par leur proximité.
    fn color(&self, p: Vec3) -> [f32; 4] {
        let mut candidates: Vec<(f32, &Solid)> = self.near(p).iter().map(|i| (self.solids[*i as usize].distance(p), &self.solids[*i as usize])).collect();
        let nearest = candidates.iter().map(|(d, _)| *d).fold(f32::MAX, f32::min);
        candidates.retain(|(d, _)| *d - nearest < self.blend * 3.0);
        let mut total = 0.0;
        let mut rgb = Vec3::ZERO;
        let mut best = (f32::MAX, false);
        for (d, solid) in &candidates {
            let weight = (-(d - nearest) / (self.blend * 0.6)).exp();
            total += weight;
            rgb += Vec3::from(solid.color) * weight;
            if *d < best.0 {
                best = (*d, solid.glow);
            }
        }
        if total <= 0.0 {
            return vertex_color([0.5, 0.5, 0.5], false);
        }
        let rgb = rgb / total;
        vertex_color([rgb.x, rgb.y, rgb.z], best.1)
    }

    /// La normale: le gradient du champ de distance.
    fn normal(&self, p: Vec3, step: f32) -> Vec3 {
        let dx = self.distance(p + Vec3::X * step) - self.distance(p - Vec3::X * step);
        let dy = self.distance(p + Vec3::Y * step) - self.distance(p - Vec3::Y * step);
        let dz = self.distance(p + Vec3::Z * step) - self.distance(p - Vec3::Z * step);
        Vec3::new(dx, dy, dz).normalize_or_zero()
    }
}

/// Les pièces de chaque couche fondues en un corps continu, plus les pièces fines et l'équipement tenus tels quels.
pub fn smooth_merge(parts: &[LivePart], blends: Blends) -> Mesh {
    let mut buffers = Buffers::default();
    for (layer, blend) in [(LiveLayer::Body, blends.body), (LiveLayer::Outfit, blends.outfit)] {
        let thin_limit = blend.cell * 2.5;
        let (thin, body): (Vec<&LivePart>, Vec<&LivePart>) = parts.iter().filter(|part| part.layer == layer).partition(|part| Solid::is_thin(part, thin_limit));
        if !body.is_empty() {
            let field = Field::new(body.iter().map(|part| Solid::new(part)).collect(), blend.radius);
            extract_surface(&field, blend.cell, &mut buffers);
        }
        for part in thin {
            buffers.push_part(part);
        }
    }
    // L'équipement tenu (arme, bouclier, bâton, arc) reste en primitives: il a ses arêtes.
    for part in parts.iter().filter(|part| part.layer == LiveLayer::Gear) {
        buffers.push_part(part);
    }
    buffers.into_mesh()
}

/// Surface nets: un sommet par case que la surface traverse, relié aux sommets des cases voisines.
fn extract_surface(field: &Field, cell: f32, out: &mut Buffers) {
    // La grille couvre ce que les pièces peuvent toucher, avec une marge.
    let margin = Vec3::splat(cell * 2.0);
    let (mut cell_cells, origin) = {
        let min = field.origin + Vec3::splat(field.blend) - margin;
        let max = field.origin + Vec3::new(field.dims[0] as f32, field.dims[1] as f32, field.dims[2] as f32) * field.bucket - Vec3::splat(field.blend) + margin;
        (((max - min) / cell).ceil().as_uvec3().max(UVec3::splat(2)), min)
    };
    // Une grille trop fine pour la mémoire: on grossit les cases.
    let mut cell = cell;
    while (cell_cells.x as u64 + 1) * (cell_cells.y as u64 + 1) * (cell_cells.z as u64 + 1) > 4_000_000 {
        cell *= 1.25;
        let min = origin;
        let max = field.origin + Vec3::new(field.dims[0] as f32, field.dims[1] as f32, field.dims[2] as f32) * field.bucket;
        cell_cells = ((max - min) / cell).ceil().as_uvec3().max(UVec3::splat(2));
    }
    let (nx, ny, nz) = (cell_cells.x as usize + 1, cell_cells.y as usize + 1, cell_cells.z as usize + 1);
    let node = |x: usize, y: usize, z: usize| (z * ny + y) * nx + x;
    let position = |x: usize, y: usize, z: usize| origin + Vec3::new(x as f32, y as f32, z as f32) * cell;
    let mut values = vec![1.0f32; nx * ny * nz];
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                values[node(x, y, z)] = field.distance(position(x, y, z));
            }
        }
    }
    // Un sommet par case traversée: la moyenne des points où la surface coupe les arêtes de la case.
    const NONE: u32 = u32::MAX;
    let mut vertex_of = vec![NONE; (nx - 1) * (ny - 1) * (nz - 1)];
    let cell_index = |x: usize, y: usize, z: usize| (z * (ny - 1) + y) * (nx - 1) + x;
    const CORNERS: [(usize, usize, usize); 8] = [(0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 0), (0, 0, 1), (1, 0, 1), (0, 1, 1), (1, 1, 1)];
    const EDGES: [(usize, usize); 12] = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];
    for z in 0..nz - 1 {
        for y in 0..ny - 1 {
            for x in 0..nx - 1 {
                let corner: [f32; 8] = std::array::from_fn(|i| values[node(x + CORNERS[i].0, y + CORNERS[i].1, z + CORNERS[i].2)]);
                let inside = corner.iter().filter(|v| **v < 0.0).count();
                if inside == 0 || inside == 8 {
                    continue;
                }
                let (mut sum, mut count) = (Vec3::ZERO, 0.0);
                for (a, b) in EDGES {
                    if (corner[a] < 0.0) != (corner[b] < 0.0) {
                        let t = corner[a] / (corner[a] - corner[b]);
                        let pa = position(x + CORNERS[a].0, y + CORNERS[a].1, z + CORNERS[a].2);
                        let pb = position(x + CORNERS[b].0, y + CORNERS[b].1, z + CORNERS[b].2);
                        sum += pa + (pb - pa) * t;
                        count += 1.0;
                    }
                }
                let at = sum / count;
                let normal = field.normal(at, cell * 0.5);
                vertex_of[cell_index(x, y, z)] = out.positions.len() as u32;
                out.positions.push(at.to_array());
                out.normals.push(normal.to_array());
                out.colors.push(field.color(at));
            }
        }
    }
    // Un quadrilatère pour chaque arête de la grille que la surface traverse, entre les quatre cases qui la partagent.
    let mut quad = |cells: [(usize, usize, usize); 4], outward: Vec3| {
        let ids = cells.map(|(x, y, z)| vertex_of[cell_index(x, y, z)]);
        if ids.contains(&NONE) {
            return;
        }
        let at = |i: usize| Vec3::from(out.positions[ids[i] as usize]);
        // Le sens des triangles: la normale du quadrilatère (le produit de ses diagonales) doit regarder vers l'extérieur.
        let facing = (at(2) - at(0)).cross(at(3) - at(1)).dot(outward);
        let order: [usize; 6] = if facing >= 0.0 { [0, 1, 2, 0, 2, 3] } else { [0, 2, 1, 0, 3, 2] };
        out.indices.extend(order.map(|i| ids[i]));
    };
    for z in 1..nz - 1 {
        for y in 1..ny - 1 {
            for x in 1..nx - 1 {
                let here = values[node(x, y, z)] < 0.0;
                if here != (values[node(x + 1, y, z)] < 0.0) {
                    quad([(x, y - 1, z - 1), (x, y, z - 1), (x, y, z), (x, y - 1, z)], if here { Vec3::X } else { Vec3::NEG_X });
                }
                if here != (values[node(x, y + 1, z)] < 0.0) {
                    quad([(x - 1, y, z - 1), (x - 1, y, z), (x, y, z), (x, y, z - 1)], if here { Vec3::Y } else { Vec3::NEG_Y });
                }
                if here != (values[node(x, y, z + 1)] < 0.0) {
                    quad([(x - 1, y - 1, z), (x, y - 1, z), (x, y, z), (x - 1, y, z)], if here { Vec3::Z } else { Vec3::NEG_Z });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn part(shape: LiveShape, center: [f32; 3], scale: [f32; 3]) -> LivePart {
        LivePart { shape, center, rot: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], scale, color: [0.8, 0.2, 0.2], glow: false, shine: 0.0, layer: LiveLayer::Outfit }
    }

    fn vertices(mesh: &Mesh) -> Vec<Vec3> {
        match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(values)) => values.iter().map(|v| Vec3::from(*v)).collect(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn a_sphere_becomes_a_sphere_with_outward_normals() {
        let mesh = smooth_merge(&[part(LiveShape::Sphere, [0.0, 1.0, 0.0], [0.5; 3])], Blends::default());
        let points = vertices(&mesh);
        assert!(points.len() > 200, "{} sommets", points.len());
        for p in &points {
            let radius = (*p - Vec3::new(0.0, 1.0, 0.0)).length();
            assert!((radius - 0.5).abs() < 0.03, "rayon {radius}");
        }
        let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL) else {
            panic!("normales manquantes");
        };
        assert!(points.iter().zip(normals).all(|(p, n)| (*p - Vec3::new(0.0, 1.0, 0.0)).normalize().dot(Vec3::from(*n)) > 0.95));
    }

    #[test]
    fn triangles_face_outward() {
        let mesh = smooth_merge(&[part(LiveShape::Sphere, [0.0, 1.0, 0.0], [0.5; 3])], Blends::default());
        let points = vertices(&mesh);
        let Some(bevy::mesh::Indices::U32(indices)) = mesh.indices() else {
            panic!("indices manquants");
        };
        for triangle in indices.chunks(3) {
            let (a, b, c) = (points[triangle[0] as usize], points[triangle[1] as usize], points[triangle[2] as usize]);
            let outward = (a + b + c) / 3.0 - Vec3::new(0.0, 1.0, 0.0);
            assert!((b - a).cross(c - a).dot(outward) > 0.0, "triangle retourné");
        }
    }

    #[test]
    fn two_touching_spheres_are_joined_by_a_neck() {
        // Deux sphères séparées de 10 cm: sans congé, deux corps; avec, un pont de matière au milieu.
        let parts = [part(LiveShape::Sphere, [-0.55, 1.0, 0.0], [0.5; 3]), part(LiveShape::Sphere, [0.55, 1.0, 0.0], [0.5; 3])];
        let smooth = smooth_merge(&parts, Blends { outfit: Blend { radius: 0.2, cell: 0.03 }, ..Blends::default() });
        let sharp = smooth_merge(&parts, Blends { outfit: Blend { radius: 0.001, cell: 0.03 }, ..Blends::default() });
        let bridge = |mesh: &Mesh| vertices(mesh).iter().filter(|p| p.x.abs() < 0.02 && (p.y - 1.0).abs() < 0.2).count();
        assert!(bridge(&smooth) > bridge(&sharp), "le pont existe seulement avec le congé");
    }

    #[test]
    fn thin_parts_stay_primitives() {
        // Une lame de 1 cm n'entre pas dans la grille: elle est ajoutée comme primitive, donc avec ses 24 sommets de cube.
        let blade = part(LiveShape::Cube, [0.0, 1.0, 0.0], [0.005, 0.4, 0.005]);
        let mesh = smooth_merge(&[blade], Blends::default());
        assert_eq!(mesh.count_vertices(), 24);
    }

    #[test]
    fn layers_are_never_fused_with_each_other() {
        // Un « casque » (tenue) posé contre une « tête » (corps), à 1 cm l'un de l'autre: avec un gros congé, ils se joindraient s'ils
        // étaient dans la même couche; dans deux couches, la tête reste un corps à part et le casque aussi.
        let mut head = part(LiveShape::Sphere, [0.0, 1.0, 0.0], [0.4; 3]);
        head.layer = LiveLayer::Body;
        let mut helmet = part(LiveShape::Sphere, [0.0, 1.0, 0.85], [0.4; 3]);
        helmet.layer = LiveLayer::Outfit;
        let big = Blends { body: Blend { radius: 0.3, cell: 0.03 }, outfit: Blend { radius: 0.3, cell: 0.03 } };
        let separate = smooth_merge(&[head.clone(), helmet.clone()], big);
        helmet.layer = LiveLayer::Body;
        let fused = smooth_merge(&[head, helmet], big);
        // Entre eux (z = 0,425, au milieu des deux sphères, dont les bords sont à z = 0,4 et 0,45): un pont n'existe que fondu.
        let neck = |mesh: &Mesh| vertices(mesh).iter().filter(|p| (p.z - 0.425).abs() < 0.02 && (p.y - 1.0).abs() < 0.2).count();
        assert!(neck(&fused) > neck(&separate), "fondus: {}, séparés: {}", neck(&fused), neck(&separate));
    }

    #[test]
    fn held_gear_stays_made_of_primitives() {
        let mut blade = part(LiveShape::Cube, [0.0, 1.0, 0.0], [0.3, 0.3, 0.3]);
        blade.layer = LiveLayer::Gear;
        assert_eq!(smooth_merge(&[blade], Blends::default()).count_vertices(), 24, "un cube intact, pas une surface fondue");
    }
}
