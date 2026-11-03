//! Les gestes de l'éditeur sur la scène 3D: la caméra libre, la visée du sol, les outils, et le rechargement du décor.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::EguiWantsInput;
use castle_shared::{Decor, ExitKind, LevelDef, RoomInfo, TILE_SIZE, Thing};

use super::items::Item;
use super::state::{Editing, PanelRects, Paint, Shape, StyleTarget, Tool, Tools};
use crate::decor3d::DecorCamera;
use crate::editor_view::EditorCamera;

/// Tangente de la moitié du champ de vision (45 degrés).
const TAN_HALF_FOV: f32 = 0.4142;

/// Le point du sol que vise un pixel de la fenêtre, en tuiles (`y` vers le haut).
pub fn ground_point(camera: &Camera, transform: &GlobalTransform, cursor: Vec2, size: UVec2) -> Option<Vec2> {
    let ray = camera.viewport_to_world(transform, cursor).ok()?;
    let t = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
    let point = ray.get_point(t);
    Some(Vec2::new(point.x, -point.z) / TILE_SIZE + size.as_vec2() / 2.0)
}

/// Un point de la carte (en tuiles) dans le monde 3D, à la hauteur `height`.
pub fn to_world(size: UVec2, tile: Vec2, height: f32) -> Vec3 {
    let world = (tile - size.as_vec2() / 2.0) * TILE_SIZE;
    Vec3::new(world.x, height, -world.y)
}

/// Caméra libre: bouton du milieu (ou Alt + clic) pour tourner, Maj en plus pour se déplacer, molette pour le zoom; les flèches
/// (ou ZQSD / WASD) déplacent, A/E (Q/E) tournent.
pub fn camera_controls(
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    wants: Res<EguiWantsInput>,
    panels: Res<PanelRects>,
    mut camera: ResMut<EditorCamera>,
    mut editing: ResMut<Editing>,
) {
    if editing.recenter {
        if let Some(def) = &editing.def {
            *camera = EditorCamera { target: def.size.as_vec2() / 2.0, ..default() };
            editing.recenter = false;
        }
    }
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let (yaw, pitch) = (camera.yaw, camera.pitch);
    let forward = Vec2::new(-yaw.sin(), yaw.cos());
    let right = Vec2::new(yaw.cos(), yaw.sin());
    let over_panel = window.cursor_position().is_none_or(|cursor| panels.contain(cursor));
    if !wants.wants_any_pointer_input() && !over_panel {
        let pointer_orbit = mouse.pressed(MouseButton::Middle) || (keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]) && mouse.pressed(MouseButton::Left));
        if pointer_orbit {
            if shift {
                let scale = camera.dist * TAN_HALF_FOV * 2.0 / window.height().max(1.0);
                camera.target += (-right * motion.delta.x + forward * motion.delta.y) * scale;
            } else {
                camera.yaw -= motion.delta.x * 0.008;
                camera.pitch = (pitch + motion.delta.y * 0.008).clamp(0.2, 1.5);
            }
        }
        if scroll.delta.y != 0.0 {
            let lines = if scroll.unit == MouseScrollUnit::Line { scroll.delta.y } else { scroll.delta.y / 40.0 };
            camera.dist = (camera.dist * (-lines * 0.1).exp()).clamp(4.0, 260.0);
        }
    }
    if !wants.wants_any_keyboard_input() {
        let dt = time.delta_secs();
        let mut step = Vec2::ZERO;
        if keys.any_pressed([KeyCode::ArrowUp, KeyCode::KeyW, KeyCode::KeyZ]) {
            step += forward;
        }
        if keys.any_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) {
            step -= forward;
        }
        if keys.any_pressed([KeyCode::ArrowRight, KeyCode::KeyD]) {
            step += right;
        }
        if keys.any_pressed([KeyCode::ArrowLeft, KeyCode::KeyA, KeyCode::KeyQ]) {
            step -= right;
        }
        let speed = camera.dist * 0.6 * dt * if shift { 2.5 } else { 1.0 };
        camera.target += step * speed;
        if keys.pressed(KeyCode::KeyE) {
            camera.yaw += dt;
        }
    }
}

/// Le niveau en cours, tel que les outils le modifient.
fn level_mut(editing: &mut Editing) -> Option<&mut LevelDef> {
    editing.def.as_mut()
}

/// Les outils: peindre, poser, sélectionner, déplacer, effacer, tracer des zones.
pub fn pointer_tools(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<DecorCamera>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    panels: Res<PanelRects>,
    mut editing: ResMut<Editing>,
    mut tools: ResMut<Tools>,
) {
    // Raccourcis: Ctrl/Cmd + Z, Maj + Z (rétablir), S (enregistrer).
    if !wants.wants_any_keyboard_input() {
        let command = keys.any_pressed([KeyCode::SuperLeft, KeyCode::SuperRight, KeyCode::ControlLeft, KeyCode::ControlRight]);
        let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        if command && keys.just_pressed(KeyCode::KeyZ) {
            if shift {
                editing.redo();
            } else {
                editing.undo();
            }
        }
        if command && keys.just_pressed(KeyCode::KeyY) {
            editing.redo();
        }
        if command && keys.just_pressed(KeyCode::KeyS) {
            if let Err(err) = editing.save() {
                editing.message = format!("Échec de l'enregistrement: {err}");
            }
        }
        if keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace) {
            if let Some(picked) = editing.picked.filter(|p| editing.def.as_ref().is_some_and(|d| *p < d.things.len())) {
                editing.checkpoint();
                if let Some(def) = level_mut(&mut editing) {
                    def.things.remove(picked);
                }
                editing.picked = None;
                editing.edited();
            }
        }
    }
    let Some(size) = editing.def.as_ref().map(|d| d.size) else {
        return;
    };
    let over_ui = wants.wants_any_pointer_input() || window.cursor_position().is_none_or(|cursor| panels.contain(cursor));
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    let hover = window.cursor_position().filter(|_| !over_ui).and_then(|cursor| ground_point(camera.0, camera.1, cursor, size));
    tools.hover = hover;
    let mut changed = false;
    let precise = hover;
    let tile = precise.map(|p| (p.x.floor() as i32, p.y.floor() as i32));

    // Fin du geste (même si le pointeur est sur un panneau).
    if mouse.just_released(MouseButton::Left) {
        if let (Some((sx, sy)), Some((ex, ey))) = (tools.drag, tile.or(tools.drag)) {
            let (w, h) = (size.x as i32, size.y as i32);
            let (ax, bx) = (sx.min(ex).max(0), sx.max(ex).min(w - 1));
            let (ay, by) = (sy.min(ey).max(0), sy.max(ey).min(h - 1));
            if bx >= ax && by >= ay {
                match tools.tool {
                    Tool::Zone => {
                        editing.checkpoint();
                        let kind = tools.zone_kind;
                        if let Some(def) = level_mut(&mut editing) {
                            def.layout.rooms.push(RoomInfo { min: UVec2::new(ax as u32, ay as u32), max: UVec2::new(bx as u32, by as u32), kind });
                        }
                        changed = true;
                    }
                    tool if tool.paints() && tools.shape == Shape::Rect => {
                        editing.checkpoint();
                        let paint = paint_of(&tools);
                        changed |= editing.paint_rect(paint, (ax, ay), (bx, by));
                    }
                    _ => {}
                }
            }
        }
        tools.drag = None;
        tools.last_paint = None;
        tools.moving = None;
        tools.resizing = None;
        tools.last_decor = None;
    }
    if over_ui || editing.def.is_none() {
        if changed {
            editing.edited();
        }
        return;
    }
    let (Some(tile), Some(precise)) = (tile, precise) else {
        if changed {
            editing.edited();
        }
        return;
    };
    let pressed = mouse.just_pressed(MouseButton::Left) && !alt;
    let held = mouse.pressed(MouseButton::Left) && !alt;

    // Clic droit: retire l'élément ou le décor le plus proche.
    if mouse.just_pressed(MouseButton::Right) {
        changed |= erase_near(&mut editing, precise);
    }

    if pressed {
        match tools.tool {
            tool if tool.paints() => {
                editing.checkpoint();
                let paint = paint_of(&tools);
                match tools.shape {
                    Shape::Brush => {
                        changed |= editing.paint_brush(paint, tile.0, tile.1, tools.brush);
                        tools.last_paint = Some(tile);
                    }
                    Shape::Rect => tools.drag = Some(tile),
                    Shape::Fill => changed |= editing.paint_fill(paint, tile.0, tile.1),
                }
            }
            Tool::Zone => {
                // Saisir le bord ou le coin d'une zone existante la redimensionne, sinon on en trace une.
                let tol = (0.35_f32).max(camera_tolerance(camera.1));
                tools.resizing = editing.def.as_ref().and_then(|def| {
                    def.layout
                        .rooms
                        .iter()
                        .enumerate()
                        .filter_map(|(i, r)| {
                            let (x0, x1) = (r.min.x as f32, r.max.x as f32 + 1.0);
                            let (y0, y1) = (r.min.y as f32, r.max.y as f32 + 1.0);
                            let (px, py) = (precise.x, precise.y);
                            if px < x0 - tol || px > x1 + tol || py < y0 - tol || py > y1 + tol {
                                return None;
                            }
                            let edges = [
                                (px - x0).abs() < tol && py > y0 - tol && py < y1 + tol,
                                (px - x1).abs() < tol && py > y0 - tol && py < y1 + tol,
                                (py - y0).abs() < tol && px > x0 - tol && px < x1 + tol,
                                (py - y1).abs() < tol && px > x0 - tol && px < x1 + tol,
                            ];
                            edges.iter().any(|e| *e).then_some((i, edges))
                        })
                        .last()
                });
                if tools.resizing.is_some() {
                    editing.checkpoint();
                } else {
                    tools.drag = Some(tile);
                }
            }
            Tool::Move => {
                tools.moving = nearest(editing.def.as_ref(), precise);
                editing.picked = tools.moving;
                if tools.moving.is_some() {
                    editing.checkpoint();
                }
            }
            Tool::Place => {
                editing.checkpoint();
                let at = Vec2::new(tile.0 as f32 + 0.5, tile.1 as f32 + 0.5);
                let item = tools.item;
                if let Some(def) = level_mut(&mut editing) {
                    // Une seule sortie par niveau: la nouvelle remplace la précédente.
                    if matches!(item, Item::Exit(_)) {
                        def.things.retain(|t| !matches!(t, Thing::Exit { .. }));
                    }
                    def.things.push(item.thing(at));
                    let last = def.things.len() - 1;
                    snap_bridge(def, last, at);
                    editing.picked = Some(last);
                }
                changed = true;
            }
            Tool::Erase => changed |= erase_near(&mut editing, precise),
            Tool::Decor => {
                editing.checkpoint();
                tools.last_decor = None;
            }
            _ => {}
        }
    }

    if held {
        match tools.tool {
            tool if tool.paints() && tools.shape == Shape::Brush && !pressed => {
                // On relie le point précédent au point courant, pour ne pas laisser de trous.
                let paint = paint_of(&tools);
                let (from_x, from_y) = tools.last_paint.unwrap_or(tile);
                let steps = (tile.0 - from_x).abs().max((tile.1 - from_y).abs()).max(1);
                for step in 0..=steps {
                    let cx = from_x + (tile.0 - from_x) * step / steps;
                    let cy = from_y + (tile.1 - from_y) * step / steps;
                    changed |= editing.paint_brush(paint, cx, cy, tools.brush);
                }
                tools.last_paint = Some(tile);
            }
            Tool::Move => {
                if let Some(i) = tools.moving {
                    let to = Vec2::new(tile.0.clamp(0, size.x as i32 - 1) as f32 + 0.5, tile.1.clamp(0, size.y as i32 - 1) as f32 + 0.5);
                    if let Some(def) = level_mut(&mut editing) {
                        if i < def.things.len() && def.things[i].at() != to {
                            def.things[i].set_at(to);
                            snap_bridge(def, i, to);
                            changed = true;
                        }
                    }
                }
            }
            Tool::Zone => {
                if let Some((i, [left, right, bottom, top])) = tools.resizing {
                    if let Some(def) = level_mut(&mut editing) {
                        if let Some(room) = def.layout.rooms.get_mut(i) {
                            let before = (room.min, room.max);
                            let (x, y) = (tile.0.clamp(0, size.x as i32 - 1) as u32, tile.1.clamp(0, size.y as i32 - 1) as u32);
                            if left {
                                room.min.x = x.min(room.max.x);
                            }
                            if right {
                                room.max.x = x.max(room.min.x);
                            }
                            if bottom {
                                room.min.y = y.min(room.max.y);
                            }
                            if top {
                                room.max.y = y.max(room.min.y);
                            }
                            changed |= before != (room.min, room.max);
                        }
                    }
                }
            }
            Tool::Decor => {
                // Un clic en pose un; en glissant, on en sème un tous les trois quarts de tuile environ.
                if tools.last_decor.is_none_or(|last| last.distance(precise) > 0.75) {
                    let kind = tools.decor_kind;
                    let variant = ((precise.x * 31.0 + precise.y * 17.0).abs() as u32 % u32::from(castle_shared::DECOR_VARIANTS)) as u8;
                    if let Some(def) = level_mut(&mut editing) {
                        def.decor.push(Decor { kind, variant, at: precise });
                    }
                    tools.last_decor = Some(precise);
                    changed = true;
                }
            }
            _ => {}
        }
    }
    if changed {
        editing.edited();
    }
}

fn paint_of(tools: &Tools) -> Paint {
    let (terrain, target) = match tools.tool {
        Tool::Wall => ('#', StyleTarget::Walls),
        Tool::Water => ('~', StyleTarget::Both),
        Tool::Bridge => ('=', StyleTarget::Both),
        _ => ('.', StyleTarget::Floor),
    };
    // La matière ne s'applique qu'au sol et aux murs: l'eau et les ponts ont la leur.
    let material = tools.material.filter(|_| matches!(tools.tool, Tool::Wall | Tool::Floor));
    match material {
        Some(region) if tools.style_only => Paint::Style(region, target),
        material => Paint::Terrain(terrain, material),
    }
}

/// Tolérance de saisie d'un bord de zone: plus large quand la caméra est loin.
fn camera_tolerance(transform: &GlobalTransform) -> f32 {
    transform.translation().y / TILE_SIZE * 0.02
}

/// L'élément le plus proche, à moins d'une tuile.
fn nearest(def: Option<&LevelDef>, at: Vec2) -> Option<usize> {
    def?.things
        .iter()
        .enumerate()
        .map(|(i, t)| (i, (t.at() - at).length()))
        .filter(|(_, d)| *d < 0.9)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Retire l'élément le plus proche, sinon le décor le plus proche.
fn erase_near(editing: &mut Editing, at: Vec2) -> bool {
    if let Some(i) = nearest(editing.def.as_ref(), at) {
        editing.checkpoint();
        if let Some(def) = editing.def.as_mut() {
            def.things.remove(i);
        }
        editing.picked = None;
        return true;
    }
    let Some(i) = editing
        .def
        .as_ref()
        .and_then(|def| def.decor.iter().enumerate().map(|(i, d)| (i, (d.at - at).length())).filter(|(_, d)| *d < 0.8).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i))
    else {
        return false;
    };
    editing.checkpoint();
    if let Some(def) = editing.def.as_mut() {
        def.decor.remove(i);
    }
    true
}

/// Cale un pont cassé sur le bloc de deux tuiles sur deux qui contient le plus de pont (voir `Room::bridge_block_corner`), plutôt que
/// sur le coin le plus proche du curseur: sinon il peut ne couvrir qu'une tuile de la largeur du pont.
pub fn snap_bridge(def: &mut LevelDef, index: usize, target: Vec2) {
    let kind = match &def.things[index] {
        Thing::Exit { kind: ExitKind::Bridge, .. } => Some(false),
        Thing::Barrier { kind: ExitKind::Bridge, .. } => Some(true),
        _ => None,
    };
    let Some(barrier) = kind else {
        return;
    };
    if let Ok(dungeon) = def.to_dungeon() {
        let at = dungeon.room.bridge_block_corner(target);
        def.things[index] = if barrier { Thing::Barrier { kind: ExitKind::Bridge, at } } else { Thing::Exit { kind: ExitKind::Bridge, at } };
    }
}

/// Refait le décor 3D quand le niveau a changé: au plus quatre fois par seconde pendant un geste, tout de suite sinon.
pub fn sync_scene(
    mut commands: Commands,
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    room: Option<Res<castle_shared::Room>>,
    mut editing: ResMut<Editing>,
    mut last: Local<f32>,
) {
    if !editing.stale || room.is_none() {
        return;
    }
    let now = time.elapsed_secs();
    if mouse.pressed(MouseButton::Left) && now - *last < 0.25 {
        return;
    }
    let level = editing.level;
    let Some(def) = editing.def.clone() else {
        return;
    };
    castle_shared::override_level(level, def);
    commands.trigger(crate::sim::ReloadLevel(level));
    editing.stale = false;
    *last = now;
}
