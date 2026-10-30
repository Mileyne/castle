//! Onglet « Niveaux »: édition des niveaux de la campagne (`shared/levels/NN.ron`).
//!
//! On peint les murs et le sol, on pose les éléments (monstres, générateurs, objets, sortie...), on trace les zones
//! (départ, salles, sortie) et on règle l'objectif. « Essayer dans le jeu » enregistre puis lance le client sur ce niveau,
//! sans recompiler (variables `CASTLE_LEVELS_DIR` et `CASTLE_START_LEVEL`).

use std::path::PathBuf;

use bevy::math::{UVec2, Vec2 as V2};
use castle_shared::{
    DoorKind, ExitKind, LEVEL_COUNT, LevelDef, MonsterKind, PickupKind, QuestKind, Region, RoomInfo, RoomKind, Thing,
    level_file,
};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Wall,
    Floor,
    Place,
    Erase,
    Zone,
}

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
    const ALL: [Item; 17] = [
        Item::Monster(MonsterKind::Skeleton),
        Item::Monster(MonsterKind::Bat),
        Item::Monster(MonsterKind::Goblin),
        Item::Monster(MonsterKind::Orc),
        Item::Monster(MonsterKind::Ghoul),
        Item::Monster(MonsterKind::Wolf),
        Item::Generator,
        Item::Trap,
        Item::Pickup(PickupKind::Coin),
        Item::Pickup(PickupKind::Food),
        Item::Pickup(PickupKind::Key),
        Item::Pickup(PickupKind::Potion),
        Item::Pickup(PickupKind::Ingredient),
        Item::Pickup(PickupKind::GearPart),
        Item::Pickup(PickupKind::TargetDisc),
        Item::Relic,
        Item::Captive,
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
            zoom: 10.0,
            drag: None,
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
        egui::Panel::left("level_list").default_size(190.0).resizable(false).show(ui, |ui| {
            ui.heading("Niveaux");
            let mut chosen = None;
            for level in 1..=LEVEL_COUNT {
                let boss = if castle_shared::is_boss_level(level) { "  ☠ boss" } else { "" };
                if ui.selectable_label(self.selected == level, format!("Niveau {level}{boss}")).clicked() && level != self.selected {
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
        egui::Panel::right("level_props").default_size(330.0).show(ui, |ui| self.props_ui(ui, &mut def));
        egui::CentralPanel::default().show(ui, |ui| {
            self.toolbar_ui(ui);
            ui.separator();
            if let Some(def) = def.as_mut() {
                self.canvas_ui(ui, def);
            } else {
                ui.label(&self.message);
                if ui.button("Créer un niveau vide ici").clicked() {
                    let created = LevelDef::blank(&format!("Niveau {}", self.selected), Region::Gardens, UVec2::new(64, 40));
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
        ui.horizontal_wrapped(|ui| {
            for (tool, label) in [
                (Tool::Wall, "Mur"),
                (Tool::Floor, "Sol"),
                (Tool::Place, "Poser"),
                (Tool::Erase, "Effacer"),
                (Tool::Zone, "Zone"),
            ] {
                ui.selectable_value(&mut self.tool, tool, label);
            }
            ui.separator();
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.zoom, 4.0..=24.0));
        });
        match self.tool {
            Tool::Place => {
                ui.horizontal_wrapped(|ui| {
                    for item in Item::ALL.into_iter().chain(Item::EXITS) {
                        ui.selectable_value(&mut self.item, item, item.label());
                    }
                });
            }
            Tool::Zone => {
                ui.horizontal(|ui| {
                    ui.label("Genre de la zone à tracer (glisser sur la carte):");
                    for kind in [RoomKind::Combat, RoomKind::Start, RoomKind::Exit, RoomKind::Treasure] {
                        ui.selectable_value(&mut self.zone_kind, kind, room_label(kind));
                    }
                });
            }
            Tool::Wall | Tool::Floor => {
                ui.label("Clic ou glisser pour peindre. Clic droit: effacer un élément.");
            }
            Tool::Erase => {
                ui.label("Clic sur un élément pour le retirer (ou sur une zone, si rien d'autre n'est là).");
            }
        }
    }

    fn canvas_ui(&mut self, ui: &mut egui::Ui, def: &mut LevelDef) {
        let zoom = self.zoom;
        let (w, h) = (def.size.x as i32, def.size.y as i32);
        egui::ScrollArea::both().show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(w as f32, h as f32) * zoom, egui::Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            let screen = |x: f32, y_up: f32| Pos2::new(rect.min.x + x * zoom, rect.min.y + (h as f32 - y_up) * zoom);
            let tile_of = |pos: Pos2| -> (f32, f32) { ((pos.x - rect.min.x) / zoom, h as f32 - (pos.y - rect.min.y) / zoom) };

            painter.rect_filled(rect, 0.0, Color32::from_gray(24));
            let floor = Color32::from_rgb(86, 104, 84);
            let wall = Color32::from_rgb(52, 50, 62);
            for (row, line) in def.rows.iter().enumerate() {
                for (x, c) in line.chars().enumerate() {
                    let r = Rect::from_min_size(Pos2::new(rect.min.x + x as f32 * zoom, rect.min.y + row as f32 * zoom), Vec2::splat(zoom));
                    painter.rect_filled(r, 0.0, if c == '#' { wall } else { floor });
                }
            }
            if zoom >= 8.0 {
                let line = egui::Stroke::new(1.0, Color32::from_white_alpha(14));
                for x in 0..=w {
                    painter.line_segment([screen(x as f32, 0.0), screen(x as f32, h as f32)], line);
                }
                for y in 0..=h {
                    painter.line_segment([screen(0.0, y as f32), screen(w as f32, y as f32)], line);
                }
            }
            // Zones, couloirs (portes).
            for (i, info) in def.layout.rooms.iter().enumerate() {
                let r = Rect::from_min_max(screen(info.min.x as f32, info.max.y as f32 + 1.0), screen(info.max.x as f32 + 1.0, info.min.y as f32));
                let color = room_color(info.kind);
                painter.rect_stroke(r, 0.0, egui::Stroke::new(1.5, color), egui::StrokeKind::Inside);
                painter.text(r.min + Vec2::new(3.0, 1.0), egui::Align2::LEFT_TOP, format!("{i} {}", room_label(info.kind)), egui::FontId::proportional(10.0), color);
            }
            for conn in &def.layout.connections {
                if let Some(door) = conn.door {
                    let size = if conn.door_vertical { (1.0, 2.0) } else { (2.0, 1.0) };
                    let r = Rect::from_min_max(
                        screen(conn.door_min.x as f32, conn.door_min.y as f32 + size.1),
                        screen(conn.door_min.x as f32 + size.0, conn.door_min.y as f32),
                    );
                    painter.rect_filled(r, 0.0, if door == DoorKind::Royal { Color32::from_rgb(200, 60, 200) } else { Color32::from_rgb(210, 160, 60) });
                }
            }
            for thing in &def.things {
                let at = thing.at();
                let (letter, color) = marker(thing);
                let center = screen(at.x, at.y);
                painter.circle_filled(center, zoom * 0.42, color.gamma_multiply(0.85));
                painter.text(center, egui::Align2::CENTER_CENTER, letter, egui::FontId::proportional((zoom * 0.7).max(8.0)), Color32::BLACK);
            }

            let hovered = response.hover_pos().map(tile_of);
            if let Some((hx, hy)) = hovered {
                let (tx, ty) = (hx.floor(), hy.floor());
                if tx >= 0.0 && ty >= 0.0 && (tx as i32) < w && (ty as i32) < h {
                    let r = Rect::from_min_max(screen(tx, ty + 1.0), screen(tx + 1.0, ty));
                    painter.rect_stroke(r, 0.0, egui::Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Outside);
                }
            }
            if let (Tool::Zone, Some((sx, sy)), Some(pos)) = (self.tool, self.drag, response.interact_pointer_pos()) {
                let (cx, cy) = tile_of(pos);
                let (ax, bx) = (sx.min(cx.floor() as i32), sx.max(cx.floor() as i32));
                let (ay, by) = (sy.min(cy.floor() as i32), sy.max(cy.floor() as i32));
                let r = Rect::from_min_max(screen(ax as f32, by as f32 + 1.0), screen(bx as f32 + 1.0, ay as f32));
                painter.rect_stroke(r, 0.0, egui::Stroke::new(2.0, room_color(self.zone_kind)), egui::StrokeKind::Inside);
            }

            let mut changed = false;
            let in_map = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h;
            let tile_under = |pos: Pos2| {
                let (x, y) = tile_of(pos);
                (x.floor() as i32, y.floor() as i32)
            };
            // Peinture continue pendant le glisser.
            if let (Some(pos), true) = (response.interact_pointer_pos(), response.dragged() || response.clicked()) {
                let (x, y) = tile_under(pos);
                if in_map(x, y) && matches!(self.tool, Tool::Wall | Tool::Floor) && !ui.input(|i| i.pointer.secondary_down()) {
                    let row = (h - 1 - y) as usize;
                    let mut chars: Vec<char> = def.rows[row].chars().collect();
                    let value = if self.tool == Tool::Wall { '#' } else { '.' };
                    if chars[x as usize] != value {
                        chars[x as usize] = value;
                        def.rows[row] = chars.into_iter().collect();
                        changed = true;
                    }
                }
            }
            if response.drag_started() {
                if let Some(pos) = response.interact_pointer_pos() {
                    self.drag = Some(tile_under(pos));
                }
            }
            if response.drag_stopped() {
                if let (Tool::Zone, Some((sx, sy)), Some(pos)) = (self.tool, self.drag, response.interact_pointer_pos()) {
                    let (ex, ey) = tile_under(pos);
                    let (ax, bx) = (sx.min(ex).max(0), sx.max(ex).min(w - 1));
                    let (ay, by) = (sy.min(ey).max(0), sy.max(ey).min(h - 1));
                    if bx >= ax && by >= ay {
                        def.layout.rooms.push(RoomInfo {
                            min: UVec2::new(ax as u32, ay as u32),
                            max: UVec2::new(bx as u32, by as u32),
                            kind: self.zone_kind,
                        });
                        changed = true;
                    }
                }
                self.drag = None;
            }
            let secondary = response.secondary_clicked();
            if response.clicked() || secondary {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (fx, fy) = tile_of(pos);
                    let (x, y) = (fx.floor() as i32, fy.floor() as i32);
                    if secondary || self.tool == Tool::Erase {
                        // Retire l'élément le plus proche, à moins d'une tuile.
                        let nearest = def
                            .things
                            .iter()
                            .enumerate()
                            .map(|(i, t)| (i, (t.at() - V2::new(fx, fy)).length()))
                            .filter(|(_, d)| *d < 0.9)
                            .min_by(|a, b| a.1.total_cmp(&b.1));
                        if let Some((i, _)) = nearest {
                            def.things.remove(i);
                            changed = true;
                        }
                    } else if self.tool == Tool::Place && in_map(x, y) {
                        let at = V2::new(x as f32 + 0.5, y as f32 + 0.5);
                        if matches!(self.item, Item::Exit(_)) {
                            def.things.retain(|t| !matches!(t, Thing::Exit { .. }));
                        }
                        def.things.push(self.item.thing(at));
                        changed = true;
                    }
                }
            }
            if changed {
                self.dirty = true;
                self.problems = def.validate();
            }
        });
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
