//! Onglet « Niveaux »: édition des niveaux de la campagne (`shared/levels/NN.ron`).
//!
//! On peint les murs et le sol, on pose les éléments (monstres, générateurs, objets, sortie...), on trace les zones
//! (départ, salles, sortie) et on règle l'objectif. « Essayer dans le jeu » enregistre puis lance le client sur ce niveau,
//! sans recompiler (variables `CASTLE_LEVELS_DIR` et `CASTLE_START_LEVEL`).

use std::path::PathBuf;

use bevy::math::{UVec2, Vec2 as V2};
use castle_shared::{
    DoorKind, ExitKind, LEVEL_COUNT, LevelDef, MonsterKind, PickupKind, QuestKind, Region,
    RoomInfo, RoomKind, Thing, level_file,
};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};

use crate::level_art::LevelArt;

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Wall,
    Floor,
    /// Donne aux tuiles le décor (sol et murs) d'une autre région.
    Style,
    Place,
    Move,
    Erase,
    Zone,
}

/// Comment l'outil Mur / Sol peint.
/// Les outils qui peignent les tuiles (pinceau, rectangle, remplissage).
fn painting_tool(tool: Tool) -> bool {
    matches!(tool, Tool::Wall | Tool::Floor | Tool::Style)
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Brush,
    Rect,
    Fill,
}

/// Nombre de niveaux gardés pour « Annuler ».
const HISTORY: usize = 100;

/// Ce que l'outil « Poser » dépose.
#[derive(Clone, Copy, PartialEq)]
enum Item {
    Monster(MonsterKind),
    Generator,
    Trap,
    Pickup(PickupKind),
    Relic,
    Captive,
    Exit(ExitKind),
}

impl Item {
    const MONSTERS: [Item; 9] = [
        Item::Monster(MonsterKind::Skeleton),
        Item::Monster(MonsterKind::Bat),
        Item::Monster(MonsterKind::Goblin),
        Item::Monster(MonsterKind::Orc),
        Item::Monster(MonsterKind::Ghoul),
        Item::Monster(MonsterKind::Wolf),
        Item::Monster(MonsterKind::Warg),
        Item::Monster(MonsterKind::Troll),
        Item::Monster(MonsterKind::Lich),
    ];

    const ELEMENTS: [Item; 4] = [Item::Generator, Item::Trap, Item::Relic, Item::Captive];

    const PICKUPS: [Item; 7] = [
        Item::Pickup(PickupKind::Coin),
        Item::Pickup(PickupKind::Food),
        Item::Pickup(PickupKind::Key),
        Item::Pickup(PickupKind::Potion),
        Item::Pickup(PickupKind::Ingredient),
        Item::Pickup(PickupKind::GearPart),
        Item::Pickup(PickupKind::TargetDisc),
    ];

    const EXITS: [Item; 4] = [
        Item::Exit(ExitKind::Stairs),
        Item::Exit(ExitKind::Gate),
        Item::Exit(ExitKind::Bridge),
        Item::Exit(ExitKind::Drawbridge),
    ];

    fn label(self) -> &'static str {
        match self {
            Item::Monster(MonsterKind::Skeleton) => "Squelette",
            Item::Monster(MonsterKind::Bat) => "Chauve-souris",
            Item::Monster(MonsterKind::Goblin) => "Gobelin",
            Item::Monster(MonsterKind::Orc) => "Orque",
            Item::Monster(MonsterKind::Ghoul) => "Goule",
            Item::Monster(MonsterKind::Wolf) => "Loup",
            Item::Monster(MonsterKind::Warg) => "Boss: Croc-Noir (chef de meute)",
            Item::Monster(MonsterKind::Troll) => "Boss: Grogoth (roi-troll)",
            Item::Monster(MonsterKind::Lich) => "Boss: Mortemer (roi-liche)",
            Item::Generator => "Générateur",
            Item::Trap => "Piège",
            Item::Pickup(PickupKind::Coin) => "Pièce",
            Item::Pickup(PickupKind::Food) => "Nourriture",
            Item::Pickup(PickupKind::Key) => "Clé",
            Item::Pickup(PickupKind::Potion) => "Potion",
            Item::Pickup(PickupKind::Ingredient) => "Ingrédient (violet)",
            Item::Pickup(PickupKind::GearPart) => "Pièce de mécanisme (orange)",
            Item::Pickup(PickupKind::TargetDisc) => "Disque de cible (vert)",
            Item::Relic => "Relique",
            Item::Captive => "Prisonnier",
            Item::Exit(ExitKind::Stairs) => "Sortie: escalier",
            Item::Exit(ExitKind::Gate) => "Sortie: barrière",
            Item::Exit(ExitKind::Bridge) => "Sortie: pont",
            Item::Exit(ExitKind::Drawbridge) => "Sortie: pont-levis",
        }
    }

    fn thing(self, at: V2) -> Thing {
        match self {
            Item::Monster(kind) => Thing::Monster { kind, at },
            Item::Generator => Thing::Generator { at },
            Item::Trap => Thing::Trap { at },
            Item::Pickup(kind) => Thing::Pickup { kind, at },
            Item::Relic => Thing::Relic { at },
            Item::Captive => Thing::Captive { at },
            Item::Exit(kind) => Thing::Exit { kind, at },
        }
    }
}

/// Lettre et couleur d'un élément sur la carte.
fn marker(thing: &Thing) -> (&'static str, Color32) {
    match thing {
        Thing::Monster { kind, .. } => (
            match kind {
                MonsterKind::Skeleton => "S",
                MonsterKind::Bat => "B",
                MonsterKind::Goblin => "G",
                MonsterKind::Orc => "O",
                MonsterKind::Ghoul => "H",
                MonsterKind::Wolf => "L",
                MonsterKind::Warg | MonsterKind::Troll | MonsterKind::Lich => "☠",
            },
            Color32::from_rgb(220, 70, 70),
        ),
        Thing::Generator { .. } => ("⚙", Color32::from_rgb(230, 120, 40)),
        Thing::Pickup { kind, .. } => (
            match kind {
                PickupKind::Coin => "$",
                PickupKind::Food => "+",
                PickupKind::Key => "k",
                PickupKind::Potion => "p",
                PickupKind::Ingredient => "i",
                PickupKind::GearPart => "g",
                PickupKind::TargetDisc => "d",
            },
            Color32::from_rgb(230, 200, 60),
        ),
        Thing::Trap { .. } => ("^", Color32::from_rgb(190, 90, 190)),
        Thing::Relic { .. } => ("R", Color32::from_rgb(90, 200, 230)),
        Thing::Captive { .. } => ("P", Color32::from_rgb(120, 220, 140)),
        Thing::Ward { .. } => ("w", Color32::from_rgb(150, 110, 240)),
        Thing::Portcullis { .. } => ("h", Color32::from_rgb(170, 170, 180)),
        Thing::Panel { .. } => ("#", Color32::from_rgb(150, 110, 240)),
        Thing::Exit { .. } => ("E", Color32::from_rgb(255, 255, 255)),
        Thing::Trigger { .. } => ("t", Color32::from_rgb(120, 140, 255)),
    }
}

fn region_label(region: Region) -> &'static str {
    match region {
        Region::Crypts => "Cryptes",
        Region::Gardens => "Jardins",
        Region::Library => "Bibliothèque",
        Region::Forge => "Forges",
        Region::Tower => "Tour astrale",
        Region::Royal => "Salles royales",
        Region::Forest => "Forêt (extérieur)",
        Region::Marsh => "Marais (extérieur)",
        Region::Approach => "Chemin du château (extérieur)",
    }
}

fn room_label(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::Start => "Départ",
        RoomKind::Combat => "Combat",
        RoomKind::Treasure => "Trésor",
        RoomKind::Exit => "Sortie",
    }
}

fn room_color(kind: RoomKind) -> Color32 {
    match kind {
        RoomKind::Start => Color32::from_rgb(90, 220, 120),
        RoomKind::Combat => Color32::from_rgb(190, 190, 200),
        RoomKind::Treasure => Color32::from_rgb(240, 210, 70),
        RoomKind::Exit => Color32::from_rgb(120, 170, 255),
    }
}

fn quest_label(kind: Option<QuestKind>) -> &'static str {
    match kind {
        None => "Aucun",
        Some(QuestKind::Hunt) => "Chasse",
        Some(QuestKind::Retrieve) => "Récupération",
        Some(QuestKind::Rescue) => "Sauvetage",
        Some(QuestKind::Repair) => "Réparer le pont (Forgebraise)",
        Some(QuestKind::Release) => "Libérer le pont-levis (Rôdeur)",
        Some(QuestKind::Unseal) => "Briser le sceau (Ensorceleur)",
        Some(QuestKind::Breach) => "Forcer la porte (Chevalier)",
        // Posé automatiquement quand le niveau contient un boss.
        Some(QuestKind::Boss) => "Vaincre le boss",
    }
}

pub struct LevelsTab {
    repo: PathBuf,
    dir: PathBuf,
    selected: u32,
    def: Option<LevelDef>,
    dirty: bool,
    tool: Tool,
    item: Item,
    zone_kind: RoomKind,
    zoom: f32,
    drag: Option<(i32, i32)>,
    shape: Shape,
    brush: i32,
    /// Région dont l'outil Décor emprunte l'aspect.
    style_region: Region,
    show_grid: bool,
    show_zones: bool,
    /// Dernière tuile peinte pendant un trait (pour relier les points sans trou).
    last_paint: Option<(i32, i32)>,
    /// Élément en cours de déplacement.
    moving: Option<usize>,
    undo: Vec<LevelDef>,
    redo: Vec<LevelDef>,
    want_undo: bool,
    want_redo: bool,
    art: Option<LevelArt>,
    art_tried: bool,
    problems: Vec<String>,
    message: String,
    /// Le bouton « Essayer dans le jeu » a été cliqué (traité une fois le niveau remis en place).
    want_test: bool,
}

impl LevelsTab {
    pub fn new(repo: PathBuf) -> Self {
        let dir = repo.join("shared/levels");
        let mut tab = Self {
            repo,
            dir,
            selected: 1,
            def: None,
            dirty: false,
            tool: Tool::Wall,
            item: Item::Monster(MonsterKind::Skeleton),
            zone_kind: RoomKind::Combat,
            zoom: 12.0,
            drag: None,
            shape: Shape::Brush,
            brush: 1,
            style_region: Region::Crypts,
            show_grid: true,
            show_zones: true,
            last_paint: None,
            moving: None,
            undo: Vec::new(),
            redo: Vec::new(),
            want_undo: false,
            want_redo: false,
            art: None,
            art_tried: false,
            problems: Vec::new(),
            message: String::new(),
            want_test: false,
        };
        tab.load(1);
        tab
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    fn path(&self, level: u32) -> PathBuf {
        self.dir.join(level_file(level))
    }

    fn load(&mut self, level: u32) {
        self.selected = level;
        self.dirty = false;
        self.drag = None;
        self.undo.clear();
        self.redo.clear();
        let text = std::fs::read_to_string(self.path(level)).unwrap_or_default();
        match LevelDef::parse(&text) {
            Ok(def) => {
                self.problems = def.validate();
                self.def = Some(def);
                self.message.clear();
            }
            Err(err) => {
                self.def = None;
                self.problems.clear();
                self.message = format!("Niveau {level}: {err}");
            }
        }
    }

    pub fn save(&mut self) -> Result<(), String> {
        let Some(def) = &self.def else {
            return Ok(());
        };
        if !self.dirty {
            return Ok(());
        }
        std::fs::write(self.path(self.selected), def.to_ron()).map_err(|e| e.to_string())?;
        self.dirty = false;
        Ok(())
    }

    /// Enregistre, puis lance le jeu sur ce niveau (Jouer en solo, nouvelle partie).
    fn test_in_game(&mut self) {
        if let Err(err) = self.save() {
            self.message = format!("Échec de l'enregistrement: {err}");
            return;
        }
        let spawned = std::process::Command::new("cargo")
            .args(["run", "--release", "-p", "castle_client"])
            .current_dir(&self.repo)
            .env("CASTLE_LEVELS_DIR", &self.dir)
            .env("CASTLE_START_LEVEL", self.selected.to_string())
            .spawn();
        self.message = match spawned {
            Ok(_) => "Jeu lancé: choisis « Jouer en solo », puis une nouvelle partie (le niveau en cours démarre).".into(),
            Err(err) => format!("Impossible de lancer le jeu: {err}"),
        };
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if !self.art_tried {
            self.art_tried = true;
            self.art = LevelArt::load(ui.ctx(), &self.repo);
        }
        egui::Panel::left("level_list")
            .default_size(190.0)
            .resizable(false)
            .show(ui, |ui| {
                ui.heading("Niveaux");
                let mut chosen = None;
                for level in 1..=LEVEL_COUNT {
                    let boss = if castle_shared::is_boss_level(level) {
                        "  ☠ boss"
                    } else {
                        ""
                    };
                    if ui
                        .selectable_label(self.selected == level, format!("Niveau {level}{boss}"))
                        .clicked()
                        && level != self.selected
                    {
                        chosen = Some(level);
                    }
                }
                if let Some(level) = chosen {
                    if self.dirty {
                        self.message = "Enregistre d'abord le niveau en cours (Ctrl+S)".into();
                    } else {
                        self.load(level);
                    }
                }
            });
        let mut def = self.def.take();
        egui::Panel::right("level_props")
            .default_size(330.0)
            .show(ui, |ui| self.props_ui(ui, &mut def));
        egui::CentralPanel::default().show(ui, |ui| {
            self.toolbar_ui(ui);
            ui.separator();
            if let Some(def) = def.as_mut() {
                self.canvas_ui(ui, def);
            } else {
                ui.label(&self.message);
                if ui.button("Créer un niveau vide ici").clicked() {
                    let created = LevelDef::blank(
                        &format!("Niveau {}", self.selected),
                        Region::Gardens,
                        UVec2::new(64, 40),
                    );
                    self.problems = created.validate();
                    def = Some(created);
                    self.dirty = true;
                }
            }
        });
        self.def = def;
        if std::mem::take(&mut self.want_test) {
            self.test_in_game();
        }
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let art = self.art.take();
        ui.horizontal_wrapped(|ui| {
            for (tool, label) in [
                (Tool::Wall, "▮ Mur"),
                (Tool::Floor, "▯ Sol"),
                (Tool::Style, "Décor"),
                (Tool::Place, "Poser"),
                (Tool::Move, "Déplacer"),
                (Tool::Erase, "Effacer"),
                (Tool::Zone, "Zone"),
            ] {
                ui.selectable_value(&mut self.tool, tool, label);
            }
            ui.separator();
            if painting_tool(self.tool) {
                for (shape, label) in [
                    (Shape::Brush, "Pinceau"),
                    (Shape::Rect, "Rectangle"),
                    (Shape::Fill, "Remplir"),
                ] {
                    ui.selectable_value(&mut self.shape, shape, label);
                }
                if self.shape == Shape::Brush {
                    ui.label("Taille");
                    ui.add(egui::Slider::new(&mut self.brush, 1..=8));
                }
                ui.separator();
            }
            self.want_undo |= ui
                .add_enabled(!self.undo.is_empty(), egui::Button::new("↶ Annuler"))
                .clicked();
            self.want_redo |= ui
                .add_enabled(!self.redo.is_empty(), egui::Button::new("↷ Rétablir"))
                .clicked();
            ui.separator();
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.zoom, 4.0..=40.0));
            ui.checkbox(&mut self.show_grid, "Grille");
            ui.checkbox(&mut self.show_zones, "Zones");
        });
        match self.tool {
            Tool::Place => self.palette_ui(ui, art.as_ref()),
            Tool::Move => {
                ui.label("Glisser un élément pour le déplacer.");
            }
            Tool::Zone => {
                ui.horizontal(|ui| {
                    ui.label("Genre de la zone à tracer (glisser sur la carte):");
                    for kind in [
                        RoomKind::Combat,
                        RoomKind::Start,
                        RoomKind::Exit,
                        RoomKind::Treasure,
                    ] {
                        ui.selectable_value(&mut self.zone_kind, kind, room_label(kind));
                    }
                });
            }
            Tool::Style => {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Décor à peindre (sol et murs):");
                    for region in Region::ALL {
                        let selected = self.style_region == region;
                        let response = match art.as_ref() {
                            Some(art) => {
                                let (texture, uv) = art.tile(region, 5);
                                let image = egui::Image::new((texture.id(), Vec2::splat(36.0))).uv(uv);
                                ui.add(egui::Button::image(image).selected(selected))
                            }
                            None => ui.selectable_label(selected, region_label(region)),
                        };
                        if response.on_hover_text(region_label(region)).clicked() {
                            self.style_region = region;
                        }
                    }
                });
                ui.label("Un niveau peut changer d'aspect au fil de sa progression: peins les salles d'un autre décor. Rectangle et Remplir marchent aussi.");
            }
            Tool::Wall | Tool::Floor => {
                ui.label("Clic ou glisser pour peindre. Clic droit: effacer un élément. Ctrl + molette: zoom. Bouton du milieu: faire défiler.");
            }
            Tool::Erase => {
                ui.label("Clic sur un élément pour le retirer.");
            }
        }
        self.art = art;
    }

    /// Les éléments à poser, en images (le nom est dans l'info-bulle).
    fn palette_ui(&mut self, ui: &mut egui::Ui, art: Option<&LevelArt>) {
        let groups: [(&str, &[Item]); 4] = [
            ("Ennemis", &Item::MONSTERS),
            ("Éléments", &Item::ELEMENTS),
            ("Objets", &Item::PICKUPS),
            ("Sorties", &Item::EXITS),
        ];
        ui.horizontal_wrapped(|ui| {
            for (title, items) in groups {
                ui.vertical(|ui| {
                    ui.small(title);
                    ui.horizontal_wrapped(|ui| {
                        ui.set_max_width(items.len() as f32 * 62.0);
                        for &item in items {
                            let selected = self.item == item;
                            let sprite = art.and_then(|a| a.palette(&item.thing(V2::ZERO)));
                            let response = match sprite {
                                Some(s) => {
                                    let image =
                                        egui::Image::new((s.texture.id(), Vec2::splat(44.0)))
                                            .uv(s.uv)
                                            .tint(s.tint);
                                    ui.add(
                                        egui::Button::image(image)
                                            .selected(selected)
                                            .min_size(Vec2::splat(52.0)),
                                    )
                                }
                                None => ui.selectable_label(selected, item.label()),
                            };
                            if response.on_hover_text(item.label()).clicked() {
                                self.item = item;
                            }
                        }
                    });
                });
                ui.add_space(10.0);
            }
        });
        ui.label(format!("À poser: {}", self.item.label()));
    }

    /// Garde l'état avant une modification, pour « Annuler ».
    fn checkpoint(&mut self, def: &LevelDef) {
        self.undo.push(def.clone());
        if self.undo.len() > HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Ctrl+Z / Ctrl+Maj+Z (et les boutons): revient en arrière ou rétablit. Vrai si le niveau a changé.
    fn apply_history(&mut self, ui: &egui::Ui, def: &mut LevelDef) -> bool {
        let (undo_key, redo_key) = ui.input(|i| {
            let z = i.modifiers.command && i.key_pressed(egui::Key::Z);
            (
                z && !i.modifiers.shift,
                (z && i.modifiers.shift) || (i.modifiers.command && i.key_pressed(egui::Key::Y)),
            )
        });
        let (undo, redo) = (
            std::mem::take(&mut self.want_undo) || undo_key,
            std::mem::take(&mut self.want_redo) || redo_key,
        );
        if undo && let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(def, previous));
            return true;
        }
        if redo && let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(def, next));
            return true;
        }
        false
    }

    fn canvas_ui(&mut self, ui: &mut egui::Ui, def: &mut LevelDef) {
        let art = self.art.take();
        let mut changed = self.apply_history(ui, def);
        // Ctrl + molette (ou pincement): zoom.
        let pinch = ui.input(|i| i.zoom_delta());
        if pinch != 1.0 && ui.rect_contains_pointer(ui.max_rect()) {
            self.zoom = (self.zoom * pinch).clamp(4.0, 40.0);
        }
        let zoom = self.zoom;
        let (w, h) = (def.size.x as i32, def.size.y as i32);
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(w as f32, h as f32) * zoom,
                egui::Sense::click_and_drag(),
            );
            let painter = ui.painter_at(rect);
            let screen = |x: f32, y_up: f32| {
                Pos2::new(rect.min.x + x * zoom, rect.min.y + (h as f32 - y_up) * zoom)
            };
            let tile_of = |pos: Pos2| -> (f32, f32) {
                (
                    (pos.x - rect.min.x) / zoom,
                    h as f32 - (pos.y - rect.min.y) / zoom,
                )
            };
            let tile_under = |pos: Pos2| {
                let (x, y) = tile_of(pos);
                (x.floor() as i32, y.floor() as i32)
            };
            let in_map = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h;
            // Le caractère d'une tuile (`#` hors de la carte).
            let cell = |def: &LevelDef, x: i32, y: i32| -> char {
                if !in_map(x, y) {
                    return '#';
                }
                def.rows[(h - 1 - y) as usize].as_bytes()[x as usize] as char
            };

            painter.rect_filled(rect, 0.0, Color32::from_gray(24));
            // Tuiles visibles seulement: les niveaux font plus de 4000 tuiles.
            let visible = ui.clip_rect().intersect(rect);
            let col0 = (((visible.min.x - rect.min.x) / zoom).floor() as i32).clamp(0, w);
            let col1 = (((visible.max.x - rect.min.x) / zoom).ceil() as i32).clamp(0, w);
            let row0 = (((visible.min.y - rect.min.y) / zoom).floor() as i32).clamp(0, h);
            let row1 = (((visible.max.y - rect.min.y) / zoom).ceil() as i32).clamp(0, h);
            let snap = |v: i32| (v as f32 * zoom).round();
            for row in row0..row1 {
                for x in col0..col1 {
                    let y = h - 1 - row;
                    let tile_region = def.region_at(x, y);
                    let region_row = Region::ALL
                        .iter()
                        .position(|r| *r == tile_region)
                        .unwrap_or(0);
                    let wall = cell(def, x, y) == '#';
                    // Pas de coutures entre les tuiles, quel que soit le zoom.
                    let r = Rect::from_min_max(
                        Pos2::new(rect.min.x + snap(x), rect.min.y + snap(row)),
                        Pos2::new(rect.min.x + snap(x + 1), rect.min.y + snap(row + 1)),
                    );
                    match &art {
                        Some(art) => {
                            let column = castle_shared::tileart::tile_column(
                                x as u32,
                                y as u32,
                                region_row,
                                wall,
                                cell(def, x, y - 1) == '.',
                            );
                            let (texture, uv) = art.tile(tile_region, column);
                            painter.image(texture.id(), r, uv, Color32::WHITE);
                            // Ombre portée sous un mur, comme dans le jeu.
                            if !wall && cell(def, x, y + 1) == '#' {
                                painter.rect_filled(
                                    Rect::from_min_size(
                                        r.min,
                                        Vec2::new(r.width(), r.height() * 0.35),
                                    ),
                                    0.0,
                                    Color32::from_rgba_unmultiplied(5, 0, 15, 100),
                                );
                            }
                        }
                        None => {
                            painter.rect_filled(
                                r,
                                0.0,
                                if wall {
                                    Color32::from_rgb(52, 50, 62)
                                } else {
                                    Color32::from_rgb(86, 104, 84)
                                },
                            );
                        }
                    }
                }
            }
            if self.show_grid && zoom >= 8.0 {
                let line = egui::Stroke::new(1.0, Color32::from_white_alpha(22));
                for x in col0..=col1 {
                    painter.line_segment([screen(x as f32, 0.0), screen(x as f32, h as f32)], line);
                }
                for row in row0..=row1 {
                    painter.line_segment(
                        [
                            screen(0.0, (h - row) as f32),
                            screen(w as f32, (h - row) as f32),
                        ],
                        line,
                    );
                }
            }
            // Zones, couloirs (portes).
            if self.show_zones {
                for (i, info) in def.layout.rooms.iter().enumerate() {
                    let r = Rect::from_min_max(
                        screen(info.min.x as f32, info.max.y as f32 + 1.0),
                        screen(info.max.x as f32 + 1.0, info.min.y as f32),
                    );
                    let color = room_color(info.kind);
                    painter.rect_filled(r, 0.0, color.gamma_multiply(0.08));
                    painter.rect_stroke(
                        r,
                        0.0,
                        egui::Stroke::new(1.5, color),
                        egui::StrokeKind::Inside,
                    );
                    painter.text(
                        r.min + Vec2::new(3.0, 1.0),
                        egui::Align2::LEFT_TOP,
                        format!("{i} {}", room_label(info.kind)),
                        egui::FontId::proportional(11.0),
                        color,
                    );
                }
            }
            for conn in &def.layout.connections {
                if let Some(door) = conn.door {
                    let size = if conn.door_vertical {
                        (1.0, 2.0)
                    } else {
                        (2.0, 1.0)
                    };
                    let r = Rect::from_min_max(
                        screen(conn.door_min.x as f32, conn.door_min.y as f32 + size.1),
                        screen(conn.door_min.x as f32 + size.0, conn.door_min.y as f32),
                    );
                    painter.rect_filled(
                        r,
                        0.0,
                        if door == DoorKind::Royal {
                            Color32::from_rgb(200, 60, 200)
                        } else {
                            Color32::from_rgb(210, 160, 60)
                        },
                    );
                }
            }
            // Éléments, du fond vers l'avant: les sprites se recouvrent comme dans le jeu.
            let mut order: Vec<usize> = (0..def.things.len()).collect();
            order.sort_by(|a, b| def.things[*b].at().y.total_cmp(&def.things[*a].at().y));
            for i in order {
                let thing = &def.things[i];
                let at = thing.at();
                match art.as_ref().and_then(|art| art.thing(thing)) {
                    Some(sprite) => {
                        let center = screen(at.x, at.y + sprite.lift);
                        let r = Rect::from_center_size(center, Vec2::splat(sprite.size * zoom));
                        painter.image(sprite.texture.id(), r, sprite.uv, sprite.tint);
                    }
                    None => {
                        let (letter, color) = marker(thing);
                        let center = screen(at.x, at.y);
                        painter.circle_filled(center, zoom * 0.42, color.gamma_multiply(0.85));
                        painter.text(
                            center,
                            egui::Align2::CENTER_CENTER,
                            letter,
                            egui::FontId::proportional((zoom * 0.7).max(8.0)),
                            Color32::BLACK,
                        );
                    }
                }
                if self.moving == Some(i) {
                    painter.circle_stroke(
                        screen(at.x, at.y),
                        zoom * 0.7,
                        egui::Stroke::new(2.0, Color32::WHITE),
                    );
                }
            }

            // Aperçu sous le curseur: la tuile ou le pinceau, ou l'élément à poser en transparence.
            let pointer = response.hover_pos().or(response.interact_pointer_pos());
            if let Some(pos) = pointer {
                let (tx, ty) = tile_under(pos);
                if in_map(tx, ty) {
                    let outline = |min: (i32, i32), max: (i32, i32), color: Color32| {
                        let r = Rect::from_min_max(
                            screen(min.0 as f32, max.1 as f32 + 1.0),
                            screen(max.0 as f32 + 1.0, min.1 as f32),
                        );
                        painter.rect_stroke(
                            r,
                            0.0,
                            egui::Stroke::new(1.5, color),
                            egui::StrokeKind::Outside,
                        );
                    };
                    match (self.tool, self.shape) {
                        (Tool::Wall | Tool::Floor | Tool::Style, Shape::Brush) => {
                            let (lo, hi) = (self.brush / 2, self.brush - 1 - self.brush / 2);
                            outline((tx - lo, ty - lo), (tx + hi, ty + hi), Color32::WHITE);
                        }
                        (Tool::Place, _) => {
                            let at = V2::new(tx as f32 + 0.5, ty as f32 + 0.5);
                            let ghost = self.item.thing(at);
                            if let Some(sprite) = art.as_ref().and_then(|art| art.thing(&ghost)) {
                                let r = Rect::from_center_size(
                                    screen(at.x, at.y + sprite.lift),
                                    Vec2::splat(sprite.size * zoom),
                                );
                                painter.image(
                                    sprite.texture.id(),
                                    r,
                                    sprite.uv,
                                    sprite.tint.gamma_multiply(0.6),
                                );
                            }
                            outline((tx, ty), (tx, ty), Color32::WHITE);
                        }
                        _ => outline((tx, ty), (tx, ty), Color32::WHITE),
                    }
                }
            }
            // Zone ou rectangle en cours de tracé.
            if let (Some((sx, sy)), Some(pos)) = (self.drag, response.interact_pointer_pos()) {
                let rect_tool = self.tool == Tool::Zone
                    || (painting_tool(self.tool) && self.shape == Shape::Rect);
                if rect_tool {
                    let (cx, cy) = tile_under(pos);
                    let (ax, bx) = (sx.min(cx), sx.max(cx));
                    let (ay, by) = (sy.min(cy), sy.max(cy));
                    let r = Rect::from_min_max(
                        screen(ax as f32, by as f32 + 1.0),
                        screen(bx as f32 + 1.0, ay as f32),
                    );
                    let color = if self.tool == Tool::Zone {
                        room_color(self.zone_kind)
                    } else if self.tool == Tool::Style {
                        Color32::from_rgb(230, 170, 60)
                    } else if self.tool == Tool::Wall {
                        Color32::from_rgb(120, 120, 150)
                    } else {
                        Color32::from_rgb(130, 200, 130)
                    };
                    painter.rect_filled(r, 0.0, color.gamma_multiply(0.3));
                    painter.rect_stroke(
                        r,
                        0.0,
                        egui::Stroke::new(2.0, color),
                        egui::StrokeKind::Inside,
                    );
                }
            }

            // Défilement avec le bouton du milieu.
            if response.dragged_by(egui::PointerButton::Middle) {
                ui.scroll_with_delta(response.drag_delta());
            }
            let primary = ui.input(|i| i.pointer.primary_down());
            let pos = response.interact_pointer_pos();
            let cursor = pos.map(tile_under);
            let precise = pos.map(tile_of);

            // Retire l'élément le plus proche, à moins d'une tuile.
            let nearest = |def: &LevelDef, (fx, fy): (f32, f32)| {
                def.things
                    .iter()
                    .enumerate()
                    .map(|(i, t)| (i, (t.at() - V2::new(fx, fy)).length()))
                    .filter(|(_, d)| *d < 0.9)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(i, _)| i)
            };
            let paint_char = if self.tool == Tool::Wall { '#' } else { '.' };
            let style_tool = self.tool == Tool::Style;
            let style_region = self.style_region;
            let set = |def: &mut LevelDef, x: i32, y: i32| -> bool {
                if !in_map(x, y) {
                    return false;
                }
                if style_tool {
                    if def.region_at(x, y) == style_region {
                        return false;
                    }
                    def.set_region_at(x, y, style_region);
                    return true;
                }
                let row = (h - 1 - y) as usize;
                let mut chars: Vec<char> = def.rows[row].chars().collect();
                if chars[x as usize] == paint_char {
                    return false;
                }
                chars[x as usize] = paint_char;
                def.rows[row] = chars.into_iter().collect();
                true
            };

            if response.drag_started_by(egui::PointerButton::Primary) {
                self.drag = cursor;
                match self.tool {
                    Tool::Wall | Tool::Floor | Tool::Style if self.shape == Shape::Brush => self.checkpoint(def),
                    Tool::Move => {
                        self.moving = precise.and_then(|p| nearest(def, p));
                        if self.moving.is_some() {
                            self.checkpoint(def);
                        }
                    }
                    _ => {}
                }
            }
            // Pinceau: on relie le point précédent au point courant, pour ne pas laisser de trous.
            let brush_stroke =
                painting_tool(self.tool) && self.shape == Shape::Brush;
            if brush_stroke
                && primary
                && (response.dragged_by(egui::PointerButton::Primary)
                    || response.clicked_by(egui::PointerButton::Primary))
            {
                if let Some((x, y)) = cursor {
                    if response.clicked_by(egui::PointerButton::Primary) {
                        self.checkpoint(def);
                    }
                    let (from_x, from_y) = self.last_paint.unwrap_or((x, y));
                    let steps = (x - from_x).abs().max((y - from_y).abs()).max(1);
                    let (lo, hi) = (self.brush / 2, self.brush - 1 - self.brush / 2);
                    for step in 0..=steps {
                        let cx = from_x + (x - from_x) * step / steps;
                        let cy = from_y + (y - from_y) * step / steps;
                        for dy in -lo..=hi {
                            for dx in -lo..=hi {
                                changed |= set(def, cx + dx, cy + dy);
                            }
                        }
                    }
                    self.last_paint = Some((x, y));
                }
            }
            if self.tool == Tool::Move && response.dragged_by(egui::PointerButton::Primary) {
                if let (Some(i), Some((x, y))) = (self.moving, cursor) {
                    let to = V2::new(
                        x.clamp(0, w - 1) as f32 + 0.5,
                        y.clamp(0, h - 1) as f32 + 0.5,
                    );
                    if def.things[i].at() != to {
                        def.things[i].set_at(to);
                        changed = true;
                    }
                }
            }
            if response.drag_stopped() {
                self.last_paint = None;
                self.moving = None;
                if let (Some((sx, sy)), Some((ex, ey))) = (self.drag, cursor) {
                    let (ax, bx) = (sx.min(ex).max(0), sx.max(ex).min(w - 1));
                    let (ay, by) = (sy.min(ey).max(0), sy.max(ey).min(h - 1));
                    if bx >= ax && by >= ay {
                        match self.tool {
                            Tool::Zone => {
                                self.checkpoint(def);
                                def.layout.rooms.push(RoomInfo {
                                    min: UVec2::new(ax as u32, ay as u32),
                                    max: UVec2::new(bx as u32, by as u32),
                                    kind: self.zone_kind,
                                });
                                changed = true;
                            }
                            Tool::Wall | Tool::Floor | Tool::Style if self.shape == Shape::Rect => {
                                self.checkpoint(def);
                                for y in ay..=by {
                                    for x in ax..=bx {
                                        changed |= set(def, x, y);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                self.drag = None;
            }
            let secondary = response.secondary_clicked();
            if response.clicked() || secondary {
                if let (Some((x, y)), Some(precise)) = (cursor, precise) {
                    if secondary || self.tool == Tool::Erase {
                        if let Some(i) = nearest(def, precise) {
                            self.checkpoint(def);
                            def.things.remove(i);
                            changed = true;
                        }
                    } else if self.tool == Tool::Place && in_map(x, y) {
                        self.checkpoint(def);
                        let at = V2::new(x as f32 + 0.5, y as f32 + 0.5);
                        if matches!(self.item, Item::Exit(_)) {
                            def.things.retain(|t| !matches!(t, Thing::Exit { .. }));
                        }
                        def.things.push(self.item.thing(at));
                        changed = true;
                    } else if painting_tool(self.tool)
                        && self.shape == Shape::Rect
                        && in_map(x, y)
                    {
                        self.checkpoint(def);
                        changed |= set(def, x, y);
                    } else if painting_tool(self.tool)
                        && self.shape == Shape::Fill
                        && in_map(x, y)
                    {
                        // Remplit la zone d'une même nature (et, pour le décor, d'un même décor) autour du clic.
                        let key = |def: &LevelDef, x: i32, y: i32| {
                            (cell(def, x, y), style_tool.then(|| def.region_at(x, y)))
                        };
                        let from = key(def, x, y);
                        let already = if style_tool { from.1 == Some(style_region) } else { from.0 == paint_char };
                        if !already {
                            self.checkpoint(def);
                            let mut stack = vec![(x, y)];
                            while let Some((cx, cy)) = stack.pop() {
                                if !in_map(cx, cy) || key(def, cx, cy) != from {
                                    continue;
                                }
                                set(def, cx, cy);
                                stack.extend([
                                    (cx + 1, cy),
                                    (cx - 1, cy),
                                    (cx, cy + 1),
                                    (cx, cy - 1),
                                ]);
                            }
                            changed = true;
                        }
                    }
                }
            }
        });
        if changed {
            self.dirty = true;
            self.problems = def.validate();
        }
        self.art = art;
    }

    fn props_ui(&mut self, ui: &mut egui::Ui, def: &mut Option<LevelDef>) {
        let Some(def) = def.as_mut() else {
            ui.label(&self.message);
            return;
        };
        let mut changed = false;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(format!("Niveau {}", self.selected));
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
                    self.message = "Cette zone sert de départ, de sortie, de trésor ou de couloir: change d'abord cela.".into();
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
                        let label = match conn.door {
                            None => "Ouvert",
                            Some(DoorKind::Locked) => "Porte à clé",
                            Some(DoorKind::Royal) => "Porte royale",
                        };
                        egui::ComboBox::from_id_salt(("door", i)).selected_text(label).show_ui(ui, |ui| {
                            for (door, text) in [(None, "Ouvert"), (Some(DoorKind::Locked), "Porte à clé"), (Some(DoorKind::Royal), "Porte royale")] {
                                if ui.selectable_label(conn.door == door, text).clicked() {
                                    conn.door = door;
                                    changed = true;
                                }
                            }
                        });
                    });
                }
            }

            ui.separator();
            ui.strong("Vérification");
            if self.problems.is_empty() {
                ui.colored_label(Color32::from_rgb(120, 220, 130), "✓ Niveau valide");
            }
            for problem in &self.problems {
                ui.colored_label(Color32::from_rgb(255, 120, 100), problem);
            }
            ui.add_space(8.0);
            if ui.button("▶ Essayer dans le jeu").on_hover_text("Enregistre puis lance le jeu sur ce niveau").clicked() {
                self.want_test = true;
            }
            if !self.message.is_empty() {
                ui.add_space(6.0);
                ui.label(&self.message);
            }
        });
        if changed {
            self.dirty = true;
            self.problems = def.validate();
        }
    }
}
