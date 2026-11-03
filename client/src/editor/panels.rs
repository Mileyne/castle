//! Les panneaux egui de l'éditeur: les niveaux, les outils, les réglages de l'élément sélectionné, les zones, l'objectif et la vérification.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, LayerId, Ui, UiBuilder};
use bevy_egui::EguiContexts;
use castle_shared::{
    DoorKind, ExitKind, InteractKind, LEVEL_COUNT, LandmarkKind, LevelDef, LinkEnd, MonsterKind, PickupKind, QuestKind, Region, RoomKind,
    Thing, is_boss_level,
};

use super::items::{Item, choose, default_node, panel_label, quest_label, region_label, room_label, scene_picker, thing_label};
use super::overlay::draw_labels;
use super::state::{Editing, PanelRects, Shape, Tool, Tools};
use super::tools::snap_bridge;
use crate::decor3d::DecorCamera;

/// Tous les panneaux, dessinés autour de la scène.
pub fn panels(
    mut contexts: EguiContexts,
    camera: Single<(&Camera, &GlobalTransform), With<DecorCamera>>,
    mut editing: ResMut<Editing>,
    mut tools: ResMut<Tools>,
    mut rects: ResMut<PanelRects>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let mut root = Ui::new(ctx.clone(), "editor_root".into(), UiBuilder::new().layer_id(LayerId::background()).max_rect(ctx.viewport_rect()));
    let top = egui::Panel::top("toolbar").show(&mut root, |ui| toolbar(ui, &mut editing, &mut tools)).response.rect;
    let left = egui::Panel::left("palette").default_size(150.0).show(&mut root, |ui| palette(ui, &mut editing, &mut tools)).response.rect;
    let right = egui::Panel::right("properties").default_size(340.0).show(&mut root, |ui| properties(ui, &mut editing, &mut tools)).response.rect;
    rects.0 = vec![top, left, right];
    // Les noms ne s'écrivent que dans ce que les panneaux laissent de la scène.
    let screen = ctx.viewport_rect();
    let scene = egui::Rect::from_min_max(egui::pos2(left.max.x, top.max.y), egui::pos2(right.min.x, screen.max.y));
    draw_labels(&ctx, scene, camera.0, camera.1, &editing);
    Ok(())
}

/// Un bouton par valeur, l'un sous l'autre sur toute la largeur du panneau.
fn choices<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, options: impl IntoIterator<Item = (T, String)>) {
    for (value, label) in options {
        if ui.add_sized([ui.available_width(), 22.0], egui::Button::selectable(*current == value, label)).clicked() {
            *current = value;
        }
    }
}

/// Les outils de la barre, deux par ligne: l'outil, et son nom (l'info-bulle).
const TOOLS: [(Tool, &str); 9] = [
    (Tool::Move, "Sélectionner / Déplacer"),
    (Tool::Erase, "Effacer"),
    (Tool::Wall, "Mur"),
    (Tool::Floor, "Sol"),
    (Tool::Water, "Eau"),
    (Tool::Bridge, "Pont"),
    (Tool::Decor, "Décor au sol"),
    (Tool::Place, "Éléments (monstres, objets, sortie, énigme...)"),
    (Tool::Zone, "Zone"),
];

/// Le dessin d'un outil, dans `r`.
fn draw_icon(painter: &egui::Painter, r: egui::Rect, tool: Tool) {
    let at = |fx: f32, fy: f32| egui::pos2(r.min.x + r.width() * fx, r.min.y + r.height() * fy);
    let line = |points: &[(f32, f32)], stroke: egui::Stroke| {
        painter.add(egui::Shape::line(points.iter().map(|(x, y)| at(*x, *y)).collect(), stroke));
    };
    let white = Color32::from_gray(225);
    match tool {
        Tool::Move => {
            // Un curseur en flèche.
            let points = [(0.15, 0.05), (0.15, 0.85), (0.38, 0.65), (0.55, 0.98), (0.7, 0.9), (0.53, 0.58), (0.82, 0.55)];
            painter.add(egui::Shape::convex_polygon(points.iter().map(|(x, y)| at(*x, *y)).collect(), white, egui::Stroke::new(1.5, Color32::BLACK)));
        }
        Tool::Erase => {
            let red = egui::Stroke::new(4.0, Color32::from_rgb(230, 80, 70));
            line(&[(0.15, 0.15), (0.85, 0.85)], red);
            line(&[(0.85, 0.15), (0.15, 0.85)], red);
        }
        Tool::Wall => {
            painter.rect_filled(r, 2.0, Color32::from_rgb(110, 108, 120));
            let mortar = egui::Stroke::new(1.5, Color32::from_rgb(52, 50, 62));
            for y in [0.33, 0.66] {
                line(&[(0.0, y), (1.0, y)], mortar);
            }
            line(&[(0.5, 0.0), (0.5, 0.33)], mortar);
            line(&[(0.25, 0.33), (0.25, 0.66)], mortar);
            line(&[(0.75, 0.33), (0.75, 0.66)], mortar);
            line(&[(0.5, 0.66), (0.5, 1.0)], mortar);
        }
        Tool::Floor => {
            painter.rect_filled(r, 2.0, Color32::from_rgb(92, 120, 80));
            for (x, y) in [(0.25, 0.25), (0.7, 0.3), (0.45, 0.55), (0.2, 0.78), (0.75, 0.75)] {
                painter.circle_filled(at(x, y), r.width() * 0.07, Color32::from_rgb(140, 170, 110));
            }
        }
        Tool::Water => {
            painter.rect_filled(r, 2.0, Color32::from_rgb(40, 96, 160));
            let wave = egui::Stroke::new(2.0, Color32::from_rgb(190, 225, 250));
            for y in [0.3, 0.62] {
                line(&[(0.05, y), (0.25, y - 0.1), (0.5, y + 0.1), (0.75, y - 0.1), (0.95, y)], wave);
            }
        }
        Tool::Bridge => {
            painter.rect_filled(r, 2.0, Color32::from_rgb(130, 90, 52));
            let plank = egui::Stroke::new(1.5, Color32::from_rgb(52, 34, 18));
            for x in [0.25, 0.5, 0.75] {
                line(&[(x, 0.0), (x, 1.0)], plank);
            }
            let rope = egui::Stroke::new(2.5, Color32::from_rgb(200, 170, 110));
            line(&[(0.0, 0.08), (1.0, 0.08)], rope);
            line(&[(0.0, 0.92), (1.0, 0.92)], rope);
        }
        Tool::Decor => {
            // Des brins d'herbe.
            let grass = egui::Stroke::new(2.5, Color32::from_rgb(110, 190, 90));
            for (x, lean) in [(0.25, -0.1), (0.45, 0.0), (0.62, 0.1), (0.8, 0.18)] {
                line(&[(x, 0.95), (x + lean, 0.5), (x + lean * 1.6, 0.12)], grass);
            }
        }
        Tool::Place => {
            // Un personnage: tête, corps.
            painter.circle_filled(at(0.5, 0.25), r.width() * 0.2, Color32::from_rgb(235, 190, 140));
            painter.add(egui::Shape::convex_polygon(vec![at(0.3, 0.95), at(0.7, 0.95), at(0.62, 0.5), at(0.38, 0.5)], Color32::from_rgb(190, 70, 70), egui::Stroke::NONE));
        }
        Tool::Zone => {
            // Un rectangle en pointillés.
            let stroke = egui::Stroke::new(2.0, Color32::from_rgb(120, 170, 255));
            let corners = [at(0.05, 0.1), at(0.95, 0.1), at(0.95, 0.9), at(0.05, 0.9), at(0.05, 0.1)];
            for pair in corners.windows(2) {
                painter.add(egui::Shape::dashed_line(&[pair[0], pair[1]], stroke, 5.0, 3.0));
            }
        }
    }
}

/// Un bouton d'outil: son dessin, surligné quand l'outil est choisi.
fn tool_button(ui: &mut Ui, selected: bool, tool: Tool, name: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(54.0, 54.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, selected);
        let painter = ui.painter();
        painter.rect(rect, 6.0, visuals.bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
        draw_icon(painter, rect.shrink(11.0), tool);
        if selected {
            painter.rect_stroke(rect, 6.0, egui::Stroke::new(2.0, Color32::from_rgb(255, 214, 80)), egui::StrokeKind::Inside);
        }
    }
    response.on_hover_text(name).clicked()
}

/// Le panneau de gauche: la barre d'outils (deux colonnes d'icônes), puis la sous-palette de l'outil choisi.
fn palette(ui: &mut Ui, editing: &mut Editing, tools: &mut Tools) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("tool_icons").num_columns(2).spacing([6.0, 6.0]).show(ui, |ui| {
            for (i, (tool, name)) in TOOLS.iter().enumerate() {
                if tool_button(ui, tools.tool == *tool, *tool, name) {
                    tools.tool = *tool;
                }
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        ui.separator();
        ui.strong(TOOLS.iter().find(|(t, _)| *t == tools.tool).map_or("", |(_, name)| name));
        match tools.tool {
            Tool::Move => {
                ui.small("Clic sur un élément: ses réglages et ses liens s'affichent à droite. Glisser: le déplacer. Suppr: le retirer.");
            }
            Tool::Erase => {
                ui.small("Clic sur un élément ou un décor pour le retirer (le clic droit le fait avec n'importe quel outil).");
            }
            Tool::Wall | Tool::Floor | Tool::Water | Tool::Bridge => {
                ui.label("Forme");
                choices(ui, &mut tools.shape, [(Shape::Brush, "Pinceau"), (Shape::Rect, "Rectangle"), (Shape::Fill, "Remplir")].map(|(v, l)| (v, l.to_string())));
                if tools.shape == Shape::Brush {
                    ui.add(egui::Slider::new(&mut tools.brush, 1..=8).text("taille"));
                }
                if matches!(tools.tool, Tool::Wall | Tool::Floor) {
                    ui.separator();
                    ui.label("Matière");
                    let options = std::iter::once((None, "Celle du niveau (inchangée)".to_string())).chain(Region::ALL.map(|r| (Some(r), region_label(r).to_string())));
                    choices(ui, &mut tools.material, options);
                    ui.add_enabled(tools.material.is_some(), egui::Checkbox::new(&mut tools.style_only, "Changer la matière seulement"))
                        .on_hover_text("Repeint les tuiles existantes sans changer ce qui est mur ou sol");
                } else {
                    ui.separator();
                    ui.small("L'eau ne se franchit que sur un pont. Pose les ponts après l'eau.");
                }
            }
            Tool::Decor => {
                choices(ui, &mut tools.decor_kind, castle_shared::DecorKind::ALL.map(|k| (k, k.label().to_string())));
                ui.small("Clic ou glisser pour poser; clic droit pour retirer.");
                if ui.button("Semer selon la région").on_hover_text("Remplace tout le décor par un semis adapté au décor de chaque tuile").clicked() {
                    editing.checkpoint();
                    let seed = u64::from(editing.level) * 7919 + 13;
                    if let Some(def) = editing.def.as_mut() {
                        def.decor = def.scatter_decor(seed);
                    }
                    editing.edited();
                }
                if ui.button("Tout retirer").clicked() {
                    editing.checkpoint();
                    if let Some(def) = editing.def.as_mut() {
                        def.decor.clear();
                    }
                    editing.edited();
                }
            }
            Tool::Place => {
                let groups: [(&str, &[Item]); 8] = [
                    ("Sortie du niveau", &Item::EXITS),
                    ("Ennemis", &Item::MONSTERS),
                    ("Éléments", &Item::ELEMENTS),
                    ("Objets", &Item::PICKUPS),
                    ("Grands motifs", &Item::LANDMARKS),
                    ("Obstacle", &Item::BARRIERS),
                    ("Énigme", &Item::PANELS),
                    ("Scènes", &Item::SCENES),
                ];
                for (title, items) in groups {
                    ui.collapsing(title, |ui| {
                        choices(ui, &mut tools.item, items.iter().map(|item| (*item, item.label().to_string())));
                    });
                }
                ui.small("Clic sur la carte pour poser. La sortie est unique: la nouvelle remplace la précédente.");
            }
            Tool::Zone => {
                choices(ui, &mut tools.zone_kind, [RoomKind::Combat, RoomKind::Start, RoomKind::Exit, RoomKind::Treasure].map(|k| (k, room_label(k).to_string())));
                ui.small("Glisser pour tracer. Saisir un bord ou un coin d'une zone pour la redimensionner.");
            }
        }
        ui.separator();
        ui.small("Caméra: molette pour le zoom; bouton du milieu (ou Alt + clic) pour tourner, avec Maj pour se déplacer; flèches ou ZQSD pour se déplacer, E pour tourner.");
    });
}

/// La barre du haut: le niveau édité, l'historique, l'enregistrement et l'essai.
fn toolbar(ui: &mut Ui, editing: &mut Editing, tools: &mut Tools) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Niveau");
        let name = editing.def.as_ref().map_or(String::new(), |d| format!(" — {}", d.name));
        let mut chosen = None;
        egui::ComboBox::from_id_salt("level").width(300.0).selected_text(format!("{}{name}", editing.level)).show_ui(ui, |ui| {
            for level in 1..=LEVEL_COUNT {
                let boss = if is_boss_level(level) { "  ☠ boss" } else { "" };
                if ui.selectable_label(editing.level == level, format!("Niveau {level}{boss}")).clicked() && level != editing.level {
                    chosen = Some(level);
                }
            }
        });
        if let Some(level) = chosen {
            if editing.dirty {
                editing.message = "Enregistre d'abord le niveau en cours (Ctrl+S)".into();
            } else {
                editing.load(level);
                tools.drag = None;
            }
        }
        ui.separator();
        if ui.add_enabled(!editing.undo.is_empty(), egui::Button::new("↶ Annuler")).clicked() {
            editing.undo();
        }
        if ui.add_enabled(!editing.redo.is_empty(), egui::Button::new("↷ Rétablir")).clicked() {
            editing.redo();
        }
        if ui.add_enabled(editing.dirty, egui::Button::new("💾 Enregistrer")).clicked() {
            match editing.save() {
                Ok(()) => editing.message = "Enregistré".into(),
                Err(err) => editing.message = format!("Échec de l'enregistrement: {err}"),
            }
        }
        if ui.button("▶ Essayer dans le jeu").on_hover_text("Enregistre puis lance le jeu sur ce niveau").clicked() {
            test_in_game(editing);
        }
        ui.separator();
        if ui.button("Recentrer").clicked() {
            editing.recenter = true;
        }
        ui.checkbox(&mut tools.show_links, "Liens");
        if !editing.message.is_empty() {
            ui.separator();
            ui.label(editing.message.clone());
        }
    });
}

/// Enregistre, puis lance le jeu sur ce niveau (Jouer en solo, nouvelle partie).
fn test_in_game(editing: &mut Editing) {
    if let Err(err) = editing.save() {
        editing.message = format!("Échec de l'enregistrement: {err}");
        return;
    }
    let spawned = std::process::Command::new("cargo")
        .args(["run", "--release", "-p", "castle_client"])
        .env("CASTLE_LEVELS_DIR", &editing.dir)
        .env("CASTLE_START_LEVEL", editing.level.to_string())
        .spawn();
    editing.message = match spawned {
        Ok(_) => "Jeu lancé: choisis « Jouer en solo », puis une nouvelle partie (le niveau en cours démarre).".into(),
        Err(err) => format!("Impossible de lancer le jeu: {err}"),
    };
}

fn properties(ui: &mut Ui, editing: &mut Editing, tools: &mut Tools) {
    let Some(mut def) = editing.def.take() else {
        ui.label(editing.message.clone());
        return;
    };
    let mut changed = false;
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading(format!("Niveau {}{}", editing.level, if editing.dirty { " •" } else { "" }));
        ui.horizontal(|ui| {
            ui.label("Nom");
            changed |= ui.text_edit_singleline(&mut def.name).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Région (décor)");
            egui::ComboBox::from_id_salt("region").selected_text(region_label(def.region)).show_ui(ui, |ui| {
                for region in Region::ALL {
                    if ui.selectable_label(def.region == region, region_label(region)).clicked() {
                        def.region = region;
                        changed = true;
                    }
                }
            });
        });
        ui.label(format!("Taille: {} x {} tuiles", def.size.x, def.size.y));
        ui.separator();
        changed |= inspector(ui, editing, tools, &mut def);

        ui.separator();
        ui.strong("Sortie");
        match def.things.iter().position(|t| matches!(t, Thing::Exit { .. })) {
            Some(i) => {
                let (kind, at) = match &def.things[i] {
                    Thing::Exit { kind, at } => (*kind, *at),
                    _ => unreachable!(),
                };
                ui.label(format!("{} en ({:.1}, {:.1})", Item::Exit(kind).label(), at.x, at.y));
                ui.horizontal(|ui| {
                    if ui.small_button("Sélectionner").clicked() {
                        editing.picked = Some(i);
                    }
                    if ui.small_button("Retirer").on_hover_text("La sortie revient au centre de la zone de sortie").clicked() {
                        def.things.remove(i);
                        editing.picked = None;
                        changed = true;
                    }
                });
            }
            None => {
                ui.label(format!("Au centre de la zone de sortie (zone {}). Outil ⚑ Sortie: pour la poser ailleurs.", def.layout.exit));
            }
        }
        changed |= ui
            .checkbox(&mut def.exit_area, "La zone entière est la sortie (invisible)")
            .on_hover_text("Sans sortie posée: tous les héros doivent se trouver dans la zone, au lieu d'un escalier en son centre")
            .changed();

        ui.collapsing(format!("Éléments ({})", def.things.len()), |ui| {
            egui::ScrollArea::vertical().id_salt("things_list").max_height(220.0).show(ui, |ui| {
                for (i, thing) in def.things.iter().enumerate() {
                    let at = thing.at();
                    if ui.selectable_label(editing.picked == Some(i), format!("{i}  {}  ({:.0}, {:.0})", thing_label(thing), at.x, at.y)).clicked() {
                        editing.picked = Some(i);
                    }
                }
            });
        });

        ui.separator();
        ui.strong("Objectif");
        let current = def.quest.map(|(k, _)| k);
        egui::ComboBox::from_id_salt("quest").selected_text(quest_label(current)).show_ui(ui, |ui| {
            for kind in [
                None,
                Some(QuestKind::Hunt),
                Some(QuestKind::Retrieve),
                Some(QuestKind::Rescue),
                Some(QuestKind::Repair),
                Some(QuestKind::Release),
                Some(QuestKind::Unseal),
                Some(QuestKind::Breach),
            ] {
                if ui.selectable_label(current == kind, quest_label(kind)).clicked() && current != kind {
                    def.quest = kind.map(|k| (k, def.quest.map_or(3, |(_, g)| g)));
                    changed = true;
                }
            }
        });
        if let Some((_, goal)) = def.quest.as_mut() {
            ui.horizontal(|ui| {
                ui.label("À atteindre");
                changed |= ui.add(egui::DragValue::new(goal).range(1..=40)).changed();
            });
            ui.label("Reliques ou prisonniers: à poser sur la carte (autant que l'objectif).");
        }

        ui.separator();
        ui.strong("Zones");
        let mut delete = None;
        for i in 0..def.layout.rooms.len() {
            ui.horizontal(|ui| {
                ui.label(format!("{i}"));
                let kind = def.layout.rooms[i].kind;
                egui::ComboBox::from_id_salt(("zone", i)).selected_text(room_label(kind)).width(90.0).show_ui(ui, |ui| {
                    for k in [RoomKind::Start, RoomKind::Combat, RoomKind::Treasure, RoomKind::Exit] {
                        if ui.selectable_label(kind == k, room_label(k)).clicked() {
                            def.layout.rooms[i].kind = k;
                            changed = true;
                        }
                    }
                });
                if ui.small_button("Départ").clicked() {
                    def.layout.start = i;
                    def.layout.rooms[i].kind = RoomKind::Start;
                    changed = true;
                }
                if ui.small_button("Sortie").clicked() {
                    def.layout.exit = i;
                    def.layout.rooms[i].kind = RoomKind::Exit;
                    changed = true;
                }
                if def.layout.start == i {
                    ui.label("◀ départ");
                }
                if def.layout.exit == i {
                    ui.label("◀ sortie");
                }
                if ui.small_button("🗑").clicked() {
                    delete = Some(i);
                }
            });
        }
        if let Some(i) = delete {
            let used = def.layout.start == i
                || def.layout.exit == i
                || def.layout.treasure == Some(i)
                || def.layout.connections.iter().any(|c| c.a == i || c.b == i);
            if used {
                editing.message = "Cette zone sert de départ, de sortie, de trésor ou de couloir: change d'abord cela.".into();
            } else {
                def.layout.rooms.remove(i);
                // Les zones suivantes changent de numéro.
                let shift = |n: &mut usize| {
                    if *n > i {
                        *n -= 1;
                    }
                };
                shift(&mut def.layout.start);
                shift(&mut def.layout.exit);
                if let Some(t) = def.layout.treasure.as_mut() {
                    shift(t);
                }
                for c in &mut def.layout.connections {
                    shift(&mut c.a);
                    shift(&mut c.b);
                }
                changed = true;
            }
        }
        if !def.layout.connections.is_empty() {
            ui.separator();
            ui.strong("Portes");
            for (i, conn) in def.layout.connections.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{} → {}", conn.a, conn.b));
                    let options = [(None, "Ouvert"), (Some(DoorKind::Locked), "Porte à clé"), (Some(DoorKind::Royal), "Porte royale")];
                    changed |= choose(ui, &format!("door{i}"), &mut conn.door, &options);
                });
            }
        }

        ui.separator();
        ui.strong("Vérification");
        if editing.problems.is_empty() {
            ui.colored_label(Color32::from_rgb(120, 220, 130), "✓ Niveau valide");
        }
        for problem in &editing.problems {
            ui.colored_label(Color32::from_rgb(255, 120, 100), problem);
        }
    });
    editing.def = Some(def);
    if changed {
        editing.edited();
    }
}

/// Les réglages de l'élément sélectionné, et ses liens. Vrai si le niveau a changé.
fn inspector(ui: &mut Ui, editing: &mut Editing, _tools: &mut Tools, def: &mut LevelDef) -> bool {
    let Some(i) = editing.picked.filter(|i| *i < def.things.len()) else {
        editing.picked = None;
        ui.weak("Aucun élément sélectionné: outil « Sélection / Déplacer », clic sur un élément (ou dans la liste plus bas).");
        return false;
    };
    let mut changed = false;
    ui.strong(format!("Élément {i}: {}", thing_label(&def.things[i])));
    let mut at = def.things[i].at();
    ui.horizontal(|ui| {
        ui.label("Position (tuiles)");
        let moved = ui.add(egui::DragValue::new(&mut at.x).speed(0.1).fixed_decimals(1)).changed()
            | ui.add(egui::DragValue::new(&mut at.y).speed(0.1).fixed_decimals(1)).changed();
        if moved {
            at = at.clamp(Vec2::ZERO, def.size.as_vec2() - Vec2::splat(0.01));
            def.things[i].set_at(at);
            snap_bridge(def, i, at);
            changed = true;
        }
    });
    match &mut def.things[i] {
        Thing::Monster { kind, .. } => {
            let options: Vec<(MonsterKind, &str)> = Item::MONSTERS.iter().filter_map(|it| if let Item::Monster(k) = it { Some((*k, it.label())) } else { None }).collect();
            changed |= choose(ui, "inspect_monster", kind, &options);
        }
        Thing::Pickup { kind, .. } => {
            let options: Vec<(PickupKind, &str)> = Item::PICKUPS.iter().filter_map(|it| if let Item::Pickup(k) = it { Some((*k, it.label())) } else { None }).collect();
            changed |= choose(ui, "inspect_pickup", kind, &options);
        }
        Thing::Exit { kind, .. } => {
            let options: Vec<(ExitKind, &str)> =
                [ExitKind::Stairs, ExitKind::Gate, ExitKind::Bridge, ExitKind::Drawbridge, ExitKind::Hidden].map(|k| (k, Item::Exit(k).label())).to_vec();
            changed |= choose(ui, "inspect_exit", kind, &options);
        }
        Thing::Landmark { kind, .. } => {
            let options: Vec<(LandmarkKind, &str)> = LandmarkKind::ALL.map(|k| (k, k.label())).to_vec();
            changed |= choose(ui, "inspect_landmark", kind, &options);
        }
        Thing::Panel { kind, node, .. } => {
            let options = [InteractKind::Target, InteractKind::Rune, InteractKind::Mechanism].map(|k| (k, panel_label(k)));
            if choose(ui, "inspect_panel", kind, &options) {
                *node = default_node(*kind).to_string();
                changed = true;
            }
            ui.label("Scène jouée à l'approche");
            changed |= scene_picker(ui, "inspect_panel_scene", node);
        }
        Thing::Portcullis { vertical, .. } => changed |= ui.checkbox(vertical, "Verticale (s'étend en hauteur)").changed(),
        Thing::Guide { caged, .. } => changed |= ui.checkbox(caged, "Prisonnière (à délivrer)").changed(),
        Thing::Trigger { node, radius, delay, .. } => {
            ui.label("Scène jouée");
            changed |= scene_picker(ui, "inspect_trigger_scene", node);
            ui.horizontal(|ui| {
                ui.label("Rayon (unités du monde)");
                changed |= ui.add(egui::DragValue::new(radius).range(8.0..=800.0).speed(2.0)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Délai (s)");
                changed |= ui.add(egui::DragValue::new(delay).range(0.0..=30.0).speed(0.1)).changed();
            });
        }
        Thing::Generator { .. } | Thing::Trap { .. } | Thing::Relic { .. } | Thing::Captive { .. } | Thing::Ward { .. } | Thing::Barrier { .. } => {}
    }
    ui.horizontal(|ui| {
        if ui.button("Dupliquer").clicked() {
            let mut copy = def.things[i].clone();
            copy.set_at(at + Vec2::new(1.0, 0.0));
            def.things.push(copy);
            editing.picked = Some(def.things.len() - 1);
            changed = true;
        }
        if ui.button("🗑 Supprimer").clicked() {
            def.things.remove(i);
            editing.picked = None;
            changed = true;
        }
    });
    // Les liens de l'élément: ce qui lui sert, et ce qu'il défait.
    let describe = |end: LinkEnd| match end {
        LinkEnd::Thing(j) => def.things.get(j).map_or("?".to_string(), |t| format!("{j} {}", thing_label(t))),
        LinkEnd::Exit => "la sortie (objectif rempli)".to_string(),
        LinkEnd::Door(_) => "la porte royale".to_string(),
    };
    let links: Vec<_> = def.links().into_iter().filter(|l| l.from == LinkEnd::Thing(i) || l.to == LinkEnd::Thing(i)).collect();
    if links.is_empty() {
        ui.weak("Aucun lien avec d'autres éléments.");
    } else {
        ui.label("Liens");
    }
    let mut goto = None;
    for link in &links {
        let (arrow, other) = if link.from == LinkEnd::Thing(i) { ("→", link.to) } else { ("←", link.from) };
        ui.horizontal(|ui| {
            ui.label(arrow);
            let text = format!("{} ({})", describe(other), link.why);
            if let LinkEnd::Thing(j) = other {
                if ui.link(text).clicked() {
                    goto = Some(j);
                }
            } else {
                ui.label(text);
            }
        });
    }
    if goto.is_some() {
        editing.picked = goto;
    }
    changed
}
