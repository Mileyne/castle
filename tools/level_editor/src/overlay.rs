//! Ce que l'éditeur dessine par-dessus la scène: zones, portes, liens, sortie, sélection, pinceau.

use std::f32::consts::FRAC_PI_2;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_egui::egui;
use castle_shared::{LinkEnd, RoomKind, TILE_SIZE, Thing};

use super::items::{room_label, thing_label};
use super::state::{Editing, Shape, Tool, Tools};
use super::tools::to_world;

/// Les traits de l'éditeur se dessinent avec la scène (même couche que la caméra du décor), par-dessus tout le reste.
pub fn setup_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.render_layers = RenderLayers::layer(castle_client::DECOR_LAYER);
    config.depth_bias = -1.0;
    config.line.width = 3.0;
}

fn room_color(kind: RoomKind) -> Color {
    match kind {
        RoomKind::Start => Color::srgb(0.35, 0.86, 0.47),
        RoomKind::Combat => Color::srgb(0.75, 0.75, 0.78),
        RoomKind::Treasure => Color::srgb(0.94, 0.82, 0.27),
        RoomKind::Exit => Color::srgb(0.47, 0.67, 1.0),
    }
}

/// Le contour d'un rectangle de tuiles, posé sur le sol.
fn outline(gizmos: &mut Gizmos, size: UVec2, min: Vec2, max: Vec2, height: f32, color: Color) {
    let p = |x: f32, y: f32| to_world(size, Vec2::new(x, y), height);
    gizmos.linestrip([p(min.x, min.y), p(max.x, min.y), p(max.x, max.y), p(min.x, max.y), p(min.x, min.y)], color);
}

fn ring(gizmos: &mut Gizmos, center: Vec3, radius: f32, color: Color) {
    gizmos.circle(Isometry3d::new(center, Quat::from_rotation_x(FRAC_PI_2)), radius, color);
}

/// La position de la sortie: l'élément posé, sinon le centre de la zone de sortie.
fn exit_at(editing: &Editing) -> Vec2 {
    let Some(def) = &editing.def else {
        return Vec2::ZERO;
    };
    def.things
        .iter()
        .find(|t| matches!(t, Thing::Exit { .. }))
        .map(Thing::at)
        .unwrap_or_else(|| def.layout.rooms.get(def.layout.exit).map_or(Vec2::ZERO, |r| r.center().as_vec2() + Vec2::splat(0.5)))
}

pub fn draw_overlay(mut gizmos: Gizmos, editing: Res<Editing>, tools: Res<Tools>) {
    let Some(def) = &editing.def else {
        return;
    };
    let size = def.size;
    let lift = 1.5;
    for info in &def.layout.rooms {
        outline(&mut gizmos, size, info.min.as_vec2(), info.max.as_vec2() + Vec2::ONE, lift, room_color(info.kind));
    }
    for conn in &def.layout.connections {
        if let Some(door) = conn.door {
            let (w, h) = if conn.door_vertical { (1.0, 2.0) } else { (2.0, 1.0) };
            let color = if door == castle_shared::DoorKind::Royal { Color::srgb(0.78, 0.24, 0.78) } else { Color::srgb(0.82, 0.63, 0.24) };
            outline(&mut gizmos, size, conn.door_min.as_vec2(), conn.door_min.as_vec2() + Vec2::new(w, h), lift + 1.0, color);
        }
    }
    // La sortie.
    let flag = Color::srgb(0.43, 0.86, 1.0);
    let exit = exit_at(&editing);
    ring(&mut gizmos, to_world(size, exit, lift), TILE_SIZE * 1.1, flag);

    // Les liens entre éléments: en jaune ce qui débloque un accès, en bleu l'objectif qui ouvre la sortie.
    if tools.show_links {
        let end = |end: LinkEnd| match end {
            LinkEnd::Thing(i) => def.things.get(i).map_or(Vec2::ZERO, Thing::at),
            LinkEnd::Exit => exit,
            LinkEnd::Door(c) => def.layout.connections.get(c).map_or(Vec2::ZERO, |c| c.door_min.as_vec2() + Vec2::splat(0.5)),
        };
        for link in def.links() {
            let involved = editing.picked.is_some_and(|p| link.from == LinkEnd::Thing(p) || link.to == LinkEnd::Thing(p));
            let objective = link.to == LinkEnd::Exit;
            let color = match (involved, objective) {
                (true, false) => Color::srgb(1.0, 0.84, 0.31),
                (true, true) => Color::srgb(0.43, 0.86, 1.0),
                (false, true) => Color::srgba(0.43, 0.86, 1.0, 0.3),
                (false, false) => Color::srgba(1.0, 1.0, 1.0, 0.3),
            };
            let (a, b) = (end(link.from), end(link.to));
            if a != b {
                gizmos.arrow(to_world(size, a, 8.0), to_world(size, b, 8.0), color);
            }
        }
    }
    // L'élément sélectionné.
    if let Some(thing) = editing.picked.and_then(|p| def.things.get(p)) {
        let at = thing.at();
        ring(&mut gizmos, to_world(size, at, lift), TILE_SIZE * 0.9, Color::srgb(1.0, 0.84, 0.31));
        if let Thing::Trigger { radius, .. } = thing {
            ring(&mut gizmos, to_world(size, at, lift), *radius, Color::srgb(0.47, 0.55, 1.0));
        }
    }
    // Le pinceau, ou le rectangle en cours de tracé.
    if let Some(hover) = tools.hover {
        let tile = IVec2::new(hover.x.floor() as i32, hover.y.floor() as i32);
        let white = Color::WHITE;
        match (tools.tool, tools.drag) {
            (tool, Some(start)) if tool.paints() || tool == Tool::Zone => {
                let (a, b) = (IVec2::new(start.0, start.1).min(tile), IVec2::new(start.0, start.1).max(tile));
                outline(&mut gizmos, size, a.as_vec2(), b.as_vec2() + Vec2::ONE, lift + 0.5, if tool == Tool::Zone { room_color(tools.zone_kind) } else { Color::srgb(0.9, 0.67, 0.24) });
            }
            (tool, None) if tool.paints() && tools.shape == Shape::Brush => {
                let (lo, hi) = (tools.brush / 2, tools.brush - 1 - tools.brush / 2);
                outline(&mut gizmos, size, (tile - IVec2::splat(lo)).as_vec2(), (tile + IVec2::splat(hi + 1)).as_vec2(), lift + 0.5, white);
            }
            _ => outline(&mut gizmos, size, tile.as_vec2(), tile.as_vec2() + Vec2::ONE, lift + 0.5, white),
        }
    }
}

/// Les noms des zones, de la sortie et de l'élément sélectionné, écrits à l'écran à leur place dans la scène.
pub fn draw_labels(ctx: &egui::Context, scene: egui::Rect, camera: &Camera, transform: &GlobalTransform, editing: &Editing) {
    let Some(def) = &editing.def else {
        return;
    };
    let painter = ctx.layer_painter(egui::LayerId::background()).with_clip_rect(scene);
    let text = |at: Vec2, height: f32, label: String, color: egui::Color32| {
        if let Ok(screen) = camera.world_to_viewport(transform, to_world(def.size, at, height)) {
            let position = egui::pos2(screen.x, screen.y);
            if scene.contains(position) {
                painter.text(position, egui::Align2::CENTER_BOTTOM, label, egui::FontId::proportional(13.0), color);
            }
        }
    };
    for (i, info) in def.layout.rooms.iter().enumerate() {
        let corner = Vec2::new(info.min.x as f32 + 0.1, info.max.y as f32 + 0.9);
        text(corner + Vec2::new(1.5, 0.0), 2.0, format!("{i} {}", room_label(info.kind)), egui::Color32::from_gray(220));
    }
    text(exit_at(editing), 14.0, "⚑ Sortie".into(), egui::Color32::from_rgb(110, 220, 255));
    if let Some(thing) = editing.picked.and_then(|p| def.things.get(p)) {
        text(thing.at(), 40.0, thing_label(thing), egui::Color32::from_rgb(255, 214, 80));
    }
}
