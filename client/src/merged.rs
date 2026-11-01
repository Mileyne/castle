//! Fusion des primitives d'un modèle de `sprite_forge` en un seul maillage.
//!
//! Un héros se compose de 90 à 155 primitives (sphères, cubes, capsules, cônes). Les dessiner une à une coûte une entité, un
//! `Transform` et un appel de dessin chacune, à chaque image. Ici toutes les pièces d'une pose sont posées dans un même maillage,
//! avec leur couleur, leur éclat et leur reflet dans les attributs des sommets: un personnage devient une entité et un appel de dessin.
//! Le maillage d'une pose se construit une fois, puis se garde (voir `live3d`).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use sprite_forge::{LivePart, LiveShape};

/// Un maillage de base (une forme de rayon 1), déjà réduit à des tableaux.
struct Base {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

/// Un cône de rayons `bottom` et `top` se distingue de l'autre au centième près.
fn base_key(shape: LiveShape) -> (u8, i32, i32) {
    match shape {
        LiveShape::Sphere => (0, 0, 0),
        LiveShape::Cube => (1, 0, 0),
        LiveShape::Capsule(_) => (2, 0, 0),
        LiveShape::Frustum(a, b) => (3, (a * 50.0).round() as i32, (b * 50.0).round() as i32),
    }
}

/// Les formes de base, avec moins de facettes que pour un affichage isolé: une pièce de personnage est petite à l'écran.
fn build_base(shape: LiveShape) -> Base {
    let mesh: Mesh = match shape {
        LiveShape::Sphere => Sphere::new(1.0).mesh().uv(12, 8),
        LiveShape::Cube => Cuboid::new(2.0, 2.0, 2.0).into(),
        LiveShape::Capsule(_) => Capsule3d::new(1.0, 2.0).mesh().rings(2).latitudes(4).longitudes(10).build(),
        LiveShape::Frustum(a, b) => ConicalFrustum { radius_top: b, radius_bottom: a, height: 2.0 }.mesh().resolution(12).build(),
    };
    let float3 = |attribute| match mesh.attribute(attribute) {
        Some(bevy::mesh::VertexAttributeValues::Float32x3(values)) => values.clone(),
        _ => Vec::new(),
    };
    let indices = match mesh.indices() {
        Some(Indices::U16(indices)) => indices.iter().map(|i| u32::from(*i)).collect(),
        Some(Indices::U32(indices)) => indices.clone(),
        None => Vec::new(),
    };
    Base { positions: float3(Mesh::ATTRIBUTE_POSITION), normals: float3(Mesh::ATTRIBUTE_NORMAL), indices }
}

fn base(shape: LiveShape) -> Arc<Base> {
    static BASES: OnceLock<Mutex<HashMap<(u8, i32, i32), Arc<Base>>>> = OnceLock::new();
    let mut bases = BASES.get_or_init(Default::default).lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    bases.entry(base_key(shape)).or_insert_with(|| Arc::new(build_base(shape))).clone()
}

/// La place d'une pièce dans le modèle.
pub fn part_transform(part: &LivePart) -> Transform {
    let mut scale = Vec3::from(part.scale);
    if let LiveShape::Capsule(h) = part.shape {
        // La capsule de référence a une demi-longueur de 2 pour un rayon de 1: on l'étire pour que le total fasse `rayon * (h + 1)`.
        scale.y = part.scale[0] * (h + 1.0) / 2.0;
    }
    Transform { translation: Vec3::from(part.center), rotation: Quat::from_mat3(&Mat3::from_cols_array_2d(&part.rot)), scale }
}

/// Toutes les pièces dans un seul maillage. Chaque sommet porte la couleur de sa pièce (sRGB, comme dans `sprite_forge`) et, en
/// coordonnées de texture, son éclat (1: la pièce brille, sans ombre) et son reflet.
pub fn merge(parts: &[LivePart]) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for part in parts {
        let shape = base(part.shape);
        let place = part_transform(part);
        let first = positions.len() as u32;
        // Une normale suit l'inverse de l'échelle (puis la rotation): une sphère aplatie garde des normales justes.
        let inverse_scale = Vec3::new(1.0 / place.scale.x.max(1e-4), 1.0 / place.scale.y.max(1e-4), 1.0 / place.scale.z.max(1e-4));
        for (position, normal) in shape.positions.iter().zip(&shape.normals) {
            positions.push((place.rotation * (Vec3::from(*position) * place.scale) + place.translation).to_array());
            normals.push((place.rotation * (Vec3::from(*normal) * inverse_scale)).normalize_or_zero().to_array());
            colors.push([part.color[0], part.color[1], part.color[2], 1.0]);
            uvs.push([f32::from(u8::from(part.glow)), part.shine]);
        }
        indices.extend(shape.indices.iter().map(|index| first + index));
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(shape: LiveShape, center: [f32; 3], glow: bool) -> LivePart {
        LivePart { shape, center, rot: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], scale: [1.0; 3], color: [1.0, 0.5, 0.25], glow, shine: 0.3 }
    }

    #[test]
    fn merging_keeps_every_vertex_and_every_triangle() {
        let parts = [part(LiveShape::Sphere, [0.0; 3], false), part(LiveShape::Cube, [2.0, 0.0, 0.0], true)];
        let (sphere, cube) = (base(LiveShape::Sphere), base(LiveShape::Cube));
        let mesh = merge(&parts);
        assert_eq!(mesh.count_vertices(), sphere.positions.len() + cube.positions.len());
        assert_eq!(mesh.indices().map(|i| i.len()), Some(sphere.indices.len() + cube.indices.len()));
    }

    #[test]
    fn parts_are_placed_and_carry_their_own_colour_and_glow() {
        let mesh = merge(&[part(LiveShape::Cube, [10.0, 0.0, 0.0], true)]);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!("positions manquantes");
        };
        assert!(positions.iter().all(|p| p[0] >= 9.0 && p[0] <= 11.0), "le cube est centré en x = 10");
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else {
            panic!("uv manquants");
        };
        assert!(uvs.iter().all(|uv| uv[0] == 1.0 && (uv[1] - 0.3).abs() < 1e-6));
    }

    #[test]
    fn indices_never_point_outside_their_part() {
        let parts = [part(LiveShape::Capsule(1.5), [0.0; 3], false), part(LiveShape::Frustum(1.0, 0.5), [0.0, 3.0, 0.0], false)];
        let mesh = merge(&parts);
        let count = mesh.count_vertices() as u32;
        assert!(mesh.indices().is_some_and(|indices| indices.iter().all(|i| (i as u32) < count)));
    }
}
