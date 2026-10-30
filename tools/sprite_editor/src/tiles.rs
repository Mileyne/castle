//! Onglet Tuiles: peinture pixel d'un groupe de tuiles (un grand motif de sol, une bande de mur...) avec crayon, gomme
//! (qui rend le pixel d'origine), pipette, remplissage, symétrie, annulation, et un aperçu en répétition pour juger les
//! raccords. Seules les tuiles modifiées sont enregistrées.

use std::collections::HashMap;

use eframe::egui::{self, Color32, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use sprite_forge::overrides::{self, TILE_PX};
use sprite_forge::{REGIONS, Theme, TileGroup};

type Pixel = [u8; 4];

const REGION_NAMES: [&str; REGIONS] = [
    "Cryptes oubliées",
    "Jardins maudits",
    "Bibliothèques vivantes",
    "Forges infernales",
    "Tours astrales",
    "Appartements royaux",
    "Forêt (extérieur)",
    "Marais (extérieur)",
    "Chemin du château (extérieur)",
];

/// Nombre d'étapes d'annulation gardées.
const UNDO_LIMIT: usize = 60;

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Pencil,
    /// Rend le pixel d'origine (celui du générateur).
    Restore,
    Picker,
    Fill,
}

/// Un groupe de tuiles en cours d'édition.
struct Doc {
    width: usize,
    height: usize,
    base: Vec<Pixel>,
    canvas: Vec<Pixel>,
    undo: Vec<Vec<Pixel>>,
    redo: Vec<Vec<Pixel>>,
    /// Ce qui est enregistré sur le disque, pour savoir si le groupe a changé.
    saved: Vec<Pixel>,
    texture: Option<TextureHandle>,
    base_texture: Option<TextureHandle>,
    texture_stale: bool,
}

fn to_pixels(rgba: &[u8]) -> Vec<Pixel> {
    rgba.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect()
}

impl Doc {
    fn load(theme: Theme, region: usize, group: TileGroup) -> Self {
        let art = sprite_forge::tile_base(theme, region, group);
        let base = to_pixels(&art.rgba);
        let mut canvas = base.clone();
        let (tiles_x, tiles_y) = group.size_in_tiles();
        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let Some(saved) = overrides::load_tile(theme, region, group.column(tx, ty)) else {
                    continue;
                };
                let saved = to_pixels(&saved);
                for y in 0..TILE_PX {
                    for x in 0..TILE_PX {
                        canvas[(ty * TILE_PX + y) * art.width + tx * TILE_PX + x] = saved[y * TILE_PX + x];
                    }
                }
            }
        }
        Self {
            width: art.width,
            height: art.height,
            base,
            saved: canvas.clone(),
            canvas,
            undo: Vec::new(),
            redo: Vec::new(),
            texture: None,
            base_texture: None,
            texture_stale: true,
        }
    }

    fn dirty(&self) -> bool {
        self.canvas != self.saved
    }

    fn push_undo(&mut self) {
        self.undo.push(self.canvas.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.canvas, previous));
            self.texture_stale = true;
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.canvas, next));
            self.texture_stale = true;
        }
    }

    /// Écrit chaque tuile modifiée; une tuile redevenue identique à l'original perd sa retouche.
    fn save(&mut self, theme: Theme, region: usize, group: TileGroup) -> std::io::Result<()> {
        let (tiles_x, tiles_y) = group.size_in_tiles();
        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let column = group.column(tx, ty);
                let mut tile = Vec::with_capacity(TILE_PX * TILE_PX * 4);
                let mut original = Vec::with_capacity(TILE_PX * TILE_PX);
                for y in 0..TILE_PX {
                    for x in 0..TILE_PX {
                        let i = (ty * TILE_PX + y) * self.width + tx * TILE_PX + x;
                        tile.extend_from_slice(&self.canvas[i]);
                        original.push(self.base[i]);
                    }
                }
                if to_pixels(&tile) == original {
                    overrides::delete_tile(theme, region, column);
                } else {
                    overrides::save_tile(theme, region, column, &tile)?;
                }
            }
        }
        self.saved = self.canvas.clone();
        Ok(())
    }

    fn image(&self, pixels: &[Pixel]) -> egui::ColorImage {
        let flat: Vec<u8> = pixels.iter().flatten().copied().collect();
        egui::ColorImage::from_rgba_unmultiplied([self.width, self.height], &flat)
    }

    fn refresh_textures(&mut self, ctx: &egui::Context) {
        if self.texture_stale || self.texture.is_none() {
            let image = self.image(&self.canvas);
            match &mut self.texture {
                Some(texture) => texture.set(image, TextureOptions::NEAREST),
                None => self.texture = Some(ctx.load_texture("tile_canvas", image, TextureOptions::NEAREST)),
            }
            self.texture_stale = false;
        }
        if self.base_texture.is_none() {
            let image = self.image(&self.base);
            self.base_texture = Some(ctx.load_texture("tile_base", image, TextureOptions::NEAREST));
        }
    }

    fn put(&mut self, x: i32, y: i32, pixel: Pixel) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let i = y as usize * self.width + x as usize;
            if self.canvas[i] != pixel {
                self.canvas[i] = pixel;
                self.texture_stale = true;
            }
        }
    }

    /// Remplit la zone de même couleur autour de (`x`, `y`).
    fn flood(&mut self, x: i32, y: i32, pixel: Pixel) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let target = self.canvas[y as usize * self.width + x as usize];
        if target == pixel {
            return;
        }
        let mut stack = vec![(x as usize, y as usize)];
        while let Some((cx, cy)) = stack.pop() {
            let i = cy * self.width + cx;
            if self.canvas[i] != target {
                continue;
            }
            self.canvas[i] = pixel;
            if cx > 0 {
                stack.push((cx - 1, cy));
            }
            if cx + 1 < self.width {
                stack.push((cx + 1, cy));
            }
            if cy > 0 {
                stack.push((cx, cy - 1));
            }
            if cy + 1 < self.height {
                stack.push((cx, cy + 1));
            }
        }
        self.texture_stale = true;
    }
}

pub struct TilesTab {
    region: usize,
    group: TileGroup,
    docs: HashMap<(usize, TileGroup), Doc>,
    tool: Tool,
    color: Color32,
    recent: Vec<Color32>,
    brush: i32,
    mirror_x: bool,
    mirror_y: bool,
    zoom: f32,
    grid: bool,
    show_base: bool,
    /// Dernier pixel peint pendant le trait en cours.
    last: Option<(i32, i32)>,
}

impl TilesTab {
    pub fn new() -> Self {
        Self {
            region: 0,
            group: TileGroup::Floor(0),
            docs: HashMap::new(),
            tool: Tool::Pencil,
            color: Color32::from_rgb(180, 170, 150),
            recent: Vec::new(),
            brush: 1,
            mirror_x: false,
            mirror_y: false,
            zoom: 4.0,
            grid: true,
            show_base: false,
            last: None,
        }
    }

    pub fn dirty(&self) -> bool {
        self.docs.values().any(Doc::dirty)
    }

    pub fn save(&mut self, theme: Theme) -> Result<(), String> {
        for ((region, group), doc) in &mut self.docs {
            if doc.dirty() {
                doc.save(theme, *region, *group).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, theme: Theme) {
        let key = (self.region, self.group);
        self.docs.entry(key).or_insert_with(|| Doc::load(theme, key.0, key.1));

        egui::Panel::left("tile_list").default_size(200.0).resizable(false).show(ui, |ui| {
            ui.heading("Région");
            for (i, name) in REGION_NAMES.iter().enumerate() {
                let dirty = self.docs.iter().any(|((r, _), d)| *r == i && d.dirty());
                let text = if dirty { format!("{name} •") } else { name.to_string() };
                if ui.selectable_label(self.region == i, text).clicked() {
                    self.region = i;
                }
            }
            ui.add_space(10.0);
            ui.heading("Tuiles");
            for group in TileGroup::all() {
                let dirty = self.docs.get(&(self.region, group)).is_some_and(Doc::dirty);
                let text = if dirty { format!("{} •", group.label()) } else { group.label() };
                if ui.selectable_label(self.group == group, text).clicked() {
                    self.group = group;
                }
            }
        });

        egui::Panel::right("tile_tools").default_size(300.0).show(ui, |ui| self.tools_ui(ui, theme));
        egui::CentralPanel::default().show(ui, |ui| self.canvas_ui(ui, theme));
    }

    fn tools_ui(&mut self, ui: &mut egui::Ui, theme: Theme) {
        let key = (self.region, self.group);
        ui.heading("Outils");
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.tool, Tool::Pencil, "Crayon");
            ui.selectable_value(&mut self.tool, Tool::Restore, "Gomme (original)");
            ui.selectable_value(&mut self.tool, Tool::Picker, "Pipette");
            ui.selectable_value(&mut self.tool, Tool::Fill, "Remplir");
        });
        ui.horizontal(|ui| {
            ui.label("Couleur");
            if ui.color_edit_button_srgba(&mut self.color).changed() {
                self.remember(self.color);
            }
        });
        if !self.recent.is_empty() {
            ui.horizontal_wrapped(|ui| {
                for color in self.recent.clone() {
                    let (rect, response) = ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::click());
                    ui.painter().rect_filled(rect, 2.0, color);
                    if response.clicked() {
                        self.color = color;
                    }
                }
            });
        }
        ui.horizontal(|ui| {
            ui.label("Pinceau");
            ui.add(egui::Slider::new(&mut self.brush, 1..=6).suffix(" px"));
        });
        ui.horizontal(|ui| {
            ui.label("Symétrie");
            ui.checkbox(&mut self.mirror_x, "gauche/droite");
            ui.checkbox(&mut self.mirror_y, "haut/bas");
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.zoom, 1.0..=10.0).step_by(1.0));
        });
        ui.checkbox(&mut self.grid, "Grille des pixels et des tuiles");
        ui.checkbox(&mut self.show_base, "Montrer l'original (sans mes retouches)");
        ui.separator();
        let Some(doc) = self.docs.get_mut(&key) else {
            return;
        };
        ui.horizontal(|ui| {
            if ui.add_enabled(!doc.undo.is_empty(), egui::Button::new("Annuler")).clicked() {
                doc.undo();
            }
            if ui.add_enabled(!doc.redo.is_empty(), egui::Button::new("Rétablir")).clicked() {
                doc.redo();
            }
        });
        if ui.button("Tout remettre à l'original").clicked() {
            doc.push_undo();
            doc.canvas = doc.base.clone();
            doc.texture_stale = true;
        }
        ui.label(egui::RichText::new("Les retouches s'enregistrent par tuile et recouvrent la version générée.").weak().small());
        ui.separator();
        ui.heading("Aperçu en répétition");
        let ctx = ui.ctx().clone();
        doc.refresh_textures(&ctx);
        let _ = theme;
        if let Some(texture) = &doc.texture {
            let (reps_x, reps_y) = match self.group {
                TileGroup::Face(_) => (3, 1),
                _ => (2, 2),
            };
            let scale = (280.0 / (doc.width as f32 * reps_x as f32)).min(1.0);
            let cell = Vec2::new(doc.width as f32, doc.height as f32) * scale;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(cell.x * reps_x as f32, cell.y * reps_y as f32), egui::Sense::hover());
            for ry in 0..reps_y {
                for rx in 0..reps_x {
                    let r = Rect::from_min_size(rect.min + Vec2::new(cell.x * rx as f32, cell.y * ry as f32), cell);
                    ui.painter().image(texture.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
            }
        }
    }

    fn remember(&mut self, color: Color32) {
        self.recent.retain(|c| *c != color);
        self.recent.insert(0, color);
        self.recent.truncate(16);
    }

    fn canvas_ui(&mut self, ui: &mut egui::Ui, theme: Theme) {
        let key = (self.region, self.group);
        let ctx = ui.ctx().clone();
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
            if let Some(doc) = self.docs.get_mut(&key) {
                doc.undo();
            }
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)) {
            if let Some(doc) = self.docs.get_mut(&key) {
                doc.redo();
            }
        }
        let _ = theme;
        let (tool, brush, mirror_x, mirror_y, zoom) = (self.tool, self.brush, self.mirror_x, self.mirror_y, self.zoom);
        let color = self.color;
        let (show_base, grid) = (self.show_base, self.grid);
        let mut picked = None;
        let mut painted_color = None;
        let mut last = self.last;

        let Some(doc) = self.docs.get_mut(&key) else {
            return;
        };
        doc.refresh_textures(&ctx);
        let size = Vec2::new(doc.width as f32, doc.height as f32) * zoom;
        egui::ScrollArea::both().show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, Color32::from_gray(30));
            let texture = if show_base { doc.base_texture.as_ref() } else { doc.texture.as_ref() };
            if let Some(texture) = texture {
                painter.image(texture.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            if grid {
                let line = |a: Pos2, b: Pos2, strong: bool| {
                    let alpha = if strong { 150 } else { 40 };
                    painter.line_segment([a, b], egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, alpha)));
                };
                for x in 0..=doc.width {
                    let strong = x % TILE_PX == 0;
                    if strong || zoom >= 6.0 {
                        let px = rect.min.x + x as f32 * zoom;
                        line(Pos2::new(px, rect.min.y), Pos2::new(px, rect.max.y), strong);
                    }
                }
                for y in 0..=doc.height {
                    let strong = y % TILE_PX == 0;
                    if strong || zoom >= 6.0 {
                        let py = rect.min.y + y as f32 * zoom;
                        line(Pos2::new(rect.min.x, py), Pos2::new(rect.max.x, py), strong);
                    }
                }
            }
            let pixel_at = |pos: Pos2| -> (i32, i32) {
                (((pos.x - rect.min.x) / zoom).floor() as i32, ((pos.y - rect.min.y) / zoom).floor() as i32)
            };
            // La retouche n'a lieu que sur l'image retouchable, pas sur l'original affiché en comparaison.
            if show_base {
                return;
            }
            let hover = response.hover_pos().map(pixel_at);
            if let (Some((hx, hy)), Tool::Pencil | Tool::Restore) = (hover, tool) {
                let half = brush / 2;
                let r = Rect::from_min_size(rect.min + Vec2::new((hx - half) as f32, (hy - half) as f32) * zoom, Vec2::splat(brush as f32 * zoom));
                painter.rect_stroke(r, 0.0, egui::Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Outside);
            }
            let pixel: Pixel = color.to_srgba_unmultiplied();
            let stamp = |doc: &mut Doc, x: i32, y: i32| {
                let half = brush / 2;
                let mut targets = vec![(x, y)];
                if mirror_x {
                    targets.push((doc.width as i32 - 1 - x, y));
                }
                if mirror_y {
                    targets.push((x, doc.height as i32 - 1 - y));
                }
                if mirror_x && mirror_y {
                    targets.push((doc.width as i32 - 1 - x, doc.height as i32 - 1 - y));
                }
                for (tx, ty) in targets {
                    for dy in 0..brush {
                        for dx in 0..brush {
                            let (px, py) = (tx - half + dx, ty - half + dy);
                            let value = match tool {
                                Tool::Restore if px >= 0 && py >= 0 && (px as usize) < doc.width && (py as usize) < doc.height => {
                                    doc.base[py as usize * doc.width + px as usize]
                                }
                                _ => pixel,
                            };
                            doc.put(px, py, value);
                        }
                    }
                }
            };
            if response.drag_started() || response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (x, y) = pixel_at(pos);
                    match tool {
                        Tool::Picker => {
                            if x >= 0 && y >= 0 && (x as usize) < doc.width && (y as usize) < doc.height {
                                let p = doc.canvas[y as usize * doc.width + x as usize];
                                picked = Some(Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]));
                            }
                        }
                        Tool::Fill => {
                            doc.push_undo();
                            doc.flood(x, y, pixel);
                            painted_color = Some(color);
                        }
                        Tool::Pencil | Tool::Restore => {
                            doc.push_undo();
                            stamp(doc, x, y);
                            last = Some((x, y));
                            if tool == Tool::Pencil {
                                painted_color = Some(color);
                            }
                        }
                    }
                }
            } else if response.dragged() && matches!(tool, Tool::Pencil | Tool::Restore) {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (x, y) = pixel_at(pos);
                    // Relie le point précédent à celui-ci pour ne pas laisser de trous quand la souris va vite.
                    let (mut cx, mut cy) = last.unwrap_or((x, y));
                    let (dx, dy) = ((x - cx).abs(), -(y - cy).abs());
                    let (sx, sy) = (if cx < x { 1 } else { -1 }, if cy < y { 1 } else { -1 });
                    let mut error = dx + dy;
                    loop {
                        stamp(doc, cx, cy);
                        if (cx, cy) == (x, y) {
                            break;
                        }
                        let e2 = 2 * error;
                        if e2 >= dy {
                            error += dy;
                            cx += sx;
                        }
                        if e2 <= dx {
                            error += dx;
                            cy += sy;
                        }
                    }
                    last = Some((x, y));
                }
            }
            if response.drag_stopped() {
                last = None;
            }
        });
        self.last = last;
        if let Some(color) = picked {
            self.color = color;
            self.tool = Tool::Pencil;
        }
        if let Some(color) = painted_color {
            self.remember(color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn painting_saving_and_restoring_a_tile() {
        let dir = std::env::temp_dir().join(format!("castle_editor_tiles_{}", std::process::id()));
        // Un seul test touche à l'environnement.
        unsafe { std::env::set_var("CASTLE_ART_OVERRIDES", &dir) };
        let (theme, region, group) = (Theme::Medieval, 3, TileGroup::Face(1));
        let mut doc = Doc::load(theme, region, group);
        assert_eq!((doc.width, doc.height), (4 * TILE_PX, TILE_PX));
        assert!(!doc.dirty());

        // Un pixel peint dans la 3e tuile de la bande ne retouche que cette tuile.
        let original = doc.canvas[10 * doc.width + 2 * TILE_PX + 5];
        doc.push_undo();
        doc.put(2 * TILE_PX as i32 + 5, 10, [1, 2, 3, 255]);
        assert!(doc.dirty());
        doc.save(theme, region, group).unwrap();
        let column = |x| group.column(x, 0);
        assert!(overrides::load_tile(theme, region, column(2)).is_some());
        for x in [0, 1, 3] {
            assert!(overrides::load_tile(theme, region, column(x)).is_none(), "tuile {x} intacte");
        }

        // Rechargé, le groupe contient la retouche.
        let reloaded = Doc::load(theme, region, group);
        assert_eq!(reloaded.canvas[10 * reloaded.width + 2 * TILE_PX + 5], [1, 2, 3, 255]);

        // Rendre le pixel d'origine supprime la retouche: la tuile redevient celle du générateur.
        doc.put(2 * TILE_PX as i32 + 5, 10, original);
        doc.save(theme, region, group).unwrap();
        assert!(overrides::load_tile(theme, region, column(2)).is_none());

        // Annuler remet l'état d'avant le trait.
        doc.push_undo();
        doc.put(0, 0, [9, 9, 9, 255]);
        doc.undo();
        assert_eq!(doc.canvas[0], doc.base[0]);
        doc.redo();
        assert_eq!(doc.canvas[0], [9, 9, 9, 255]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn flood_fill_stays_inside_its_region() {
        let mut doc = Doc {
            width: 4,
            height: 2,
            base: vec![[0; 4]; 8],
            canvas: [[0, 0, 0, 255], [0, 0, 0, 255], [9, 9, 9, 255], [0, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255], [9, 9, 9, 255], [0, 0, 0, 255]].to_vec(),
            undo: Vec::new(),
            redo: Vec::new(),
            saved: Vec::new(),
            texture: None,
            base_texture: None,
            texture_stale: false,
        };
        doc.flood(0, 0, [5, 5, 5, 255]);
        assert_eq!(doc.canvas[0], [5, 5, 5, 255]);
        assert_eq!(doc.canvas[5], [5, 5, 5, 255]);
        assert_eq!(doc.canvas[2], [9, 9, 9, 255], "le mur reste");
        assert_eq!(doc.canvas[3], [0, 0, 0, 255], "l'autre côté du mur reste");
    }
}
