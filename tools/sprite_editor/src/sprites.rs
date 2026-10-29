//! Onglet Sprites: choix d'un modèle, aperçu animé sous les huit directions, réglages des proportions et des matières.

use std::collections::HashMap;

use eframe::egui::{self, Color32, TextureHandle, TextureOptions};
use sprite_forge::tuning::{self, Material, ModelTuning, Tuning};
use sprite_forge::{Anim, DIRECTION_COUNT, ModelInfo, SPRITE_PX, Theme};

/// Zoom d'affichage maximal (pixels d'écran par pixel du sprite).
const MAX_ZOOM: f32 = 32.0;

const DIRECTION_NAMES: [&str; DIRECTION_COUNT] = ["Est", "Nord-est", "Nord (dos)", "Nord-ouest", "Ouest", "Sud-ouest", "Sud (face)", "Sud-est"];

#[derive(Clone, Copy, PartialEq)]
enum AnimKind {
    Idle,
    Walk,
    Attack,
}

impl AnimKind {
    fn at(self, t: f32) -> Anim {
        match self {
            AnimKind::Idle => Anim::Idle(t),
            AnimKind::Walk => Anim::Walk(t),
            AnimKind::Attack => Anim::Attack(t),
        }
    }
}

/// Durée d'un tour d'animation, en secondes.
const CYCLE: f32 = 1.2;

pub struct SpritesTab {
    models: Vec<ModelInfo>,
    selected: usize,
    /// Réglages en cours d'édition, et ceux du fichier.
    tuning: Tuning,
    saved: Tuning,
    /// Compteur incrémenté à chaque modification, pour savoir quand refaire l'aperçu.
    revision: u64,
    direction: usize,
    anim: AnimKind,
    playing: bool,
    phase: f32,
    zoom: f32,
    /// Résolution du rendu de l'aperçu: 1 = pixels du jeu, 2 ou 4 = plus de détail pour travailler de près.
    detail: usize,
    /// Position de défilement à imposer à l'image au prochain affichage (zoom sur un point, aller à la tête).
    pending_offset: Option<egui::Vec2>,
    /// Où l'image et son défilement se trouvaient à la dernière image, pour zoomer autour du curseur.
    last_view: Option<(egui::Rect, egui::Vec2)>,
    /// Matière sélectionnée (couleur d'origine): ses détails s'affichent et elle est surlignée en magenta si demandé.
    material: Option<u32>,
    highlight: bool,
    main_texture: Option<TextureHandle>,
    strip: Vec<TextureHandle>,
    /// Ce que les images affichées représentent.
    main_key: Option<(usize, u64, usize, u8, u32, Option<u32>, usize)>,
    strip_key: Option<(usize, u64, Option<u32>)>,
    /// Valeurs d'origine des réglages et matières, par modèle.
    params: HashMap<usize, Vec<(String, f32)>>,
    materials: HashMap<usize, Vec<(u32, usize)>>,
}

impl SpritesTab {
    /// `--modele <clé>` ouvre directement ce modèle (par exemple `hero:Sorcerer`), `--tete` cadre sa tête.
    pub fn new(theme: Theme) -> Self {
        let tuning = tuning::load(theme);
        let args: Vec<String> = std::env::args().collect();
        let models = sprite_forge::models();
        let selected = args
            .iter()
            .position(|a| a == "--modele")
            .and_then(|i| args.get(i + 1))
            .and_then(|key| models.iter().position(|m| &m.key == key))
            .unwrap_or(0);
        let on_head = args.iter().any(|a| a == "--tete");
        Self {
            models,
            selected,
            saved: tuning.clone(),
            tuning,
            revision: 0,
            direction: 6,
            anim: AnimKind::Idle,
            playing: false,
            phase: 0.0,
            zoom: if on_head { 16.0 } else { 5.0 },
            detail: if on_head { 4 } else { 1 },
            pending_offset: on_head.then(|| egui::vec2(48.0 * 16.0 - 400.0, 24.0 * 16.0 - 250.0)),
            last_view: None,
            material: None,
            highlight: true,
            main_texture: None,
            strip: Vec::new(),
            main_key: None,
            strip_key: None,
            params: HashMap::new(),
            materials: HashMap::new(),
        }
    }

    pub fn dirty(&self) -> bool {
        pruned(&self.tuning, &self.params) != pruned(&self.saved, &self.params)
    }

    pub fn save(&mut self, theme: Theme) -> Result<(), String> {
        // Les modèles jamais ouverts n'ont pas de valeurs d'origine en cache: elles servent à retirer les réglages
        // inutiles, on les calcule donc pour tous avant d'écrire.
        for index in 0..self.models.len() {
            self.base_params(index, theme);
        }
        let clean = pruned(&self.tuning, &self.params);
        tuning::save(theme, &clean).map_err(|e| e.to_string())?;
        self.saved = self.tuning.clone();
        Ok(())
    }

    fn base_params(&mut self, index: usize, theme: Theme) -> &Vec<(String, f32)> {
        let key = self.models[index].key.clone();
        self.params.entry(index).or_insert_with(|| sprite_forge::model_params(&key, theme))
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, theme: Theme, busy: bool) {
        egui::Panel::left("models").default_size(190.0).resizable(false).show(ui, |ui| {
            ui.heading("Modèles");
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut group = "";
                for index in 0..self.models.len() {
                    let info = &self.models[index];
                    if info.group != group {
                        group = info.group;
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(group).strong());
                    }
                    let changed = self.tuning.models.get(&info.key).is_some_and(|m| *m != ModelTuning::default());
                    let text = if changed { format!("{} •", info.label) } else { info.label.to_string() };
                    if ui.selectable_label(self.selected == index, text).clicked() {
                        self.selected = index;
                        self.material = None;
                    }
                }
            });
        });

        egui::Panel::right("settings").default_size(420.0).show(ui, |ui| {
            self.settings_ui(ui, theme);
        });

        egui::CentralPanel::default().show(ui, |ui| {
            self.preview_ui(ui, theme, busy);
        });
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui, theme: Theme, busy: bool) {
        let info = self.models[self.selected].clone();
        ui.horizontal(|ui| {
            ui.heading(info.label);
            ui.separator();
            ui.selectable_value(&mut self.anim, AnimKind::Idle, "Repos");
            ui.selectable_value(&mut self.anim, AnimKind::Walk, "Marche");
            ui.selectable_value(&mut self.anim, AnimKind::Attack, "Attaque");
            ui.checkbox(&mut self.playing, "Animer");
            ui.separator();
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.zoom, 1.0..=MAX_ZOOM).logarithmic(true));
        });
        ui.horizontal(|ui| {
            if ui.button("Tout voir").on_hover_text("Zoom 5 sur le sprite entier").clicked() {
                self.zoom = 5.0;
                self.detail = 1;
                self.pending_offset = Some(egui::Vec2::ZERO);
            }
            if info.oriented && ui.button("Tête").on_hover_text("Zoom sur la tête, en haute définition").clicked() {
                self.zoom = 16.0;
                self.detail = 4;
                // La tête d'un personnage est vers le haut et le centre de l'image (en pixels du sprite).
                let view = ui.available_size();
                self.pending_offset = Some(egui::vec2(
                    (48.0 * self.zoom - view.x / 2.0).max(0.0),
                    (24.0 * self.zoom - view.y / 2.0).max(0.0),
                ));
            }
            ui.label("Rendu");
            for (detail, label) in [(1, "pixels du jeu"), (2, "détaillé ×2"), (4, "détaillé ×4")] {
                ui.selectable_value(&mut self.detail, detail, label);
            }
            ui.label(egui::RichText::new("Ctrl+molette ou pincement: zoom, glisser: déplacer").weak().small());
        });
        if self.playing {
            self.phase = (self.phase + ui.input(|i| i.stable_dt) / CYCLE).fract();
            ui.request_repaint();
        } else {
            ui.horizontal(|ui| {
                ui.label("Image");
                ui.add(egui::Slider::new(&mut self.phase, 0.0..=1.0));
            });
        }
        if info.oriented {
            ui.horizontal_wrapped(|ui| {
                for (i, name) in DIRECTION_NAMES.iter().enumerate() {
                    ui.selectable_value(&mut self.direction, i, *name);
                }
            });
        }

        // Pendant la génération des planches, les réglages actifs sont ceux du fichier: pas d'aperçu.
        if busy {
            ui.add_space(20.0);
            ui.label("Génération des planches en cours...");
            return;
        }
        let highlight = if self.highlight { self.material } else { None };
        let direction = if info.oriented { self.direction } else { 0 };
        let quantized = (self.phase * 100.0) as u32;
        let key = (self.selected, self.revision, direction, self.anim as u8, quantized, highlight, self.detail);
        if self.main_key != Some(key) {
            let rgba = sprite_forge::render_model_detail(&info.key, theme, self.anim.at(self.phase), direction, &self.tuning, highlight, self.detail);
            self.main_texture = Some(load(ui.ctx(), "main", &rgba, SPRITE_PX * self.detail));
            self.main_key = Some(key);
        }

        // Zoom à la molette (Ctrl) ou au pincement, centré sur le curseur.
        if let (Some((rect, offset)), Some(pointer)) = (self.last_view, ui.input(|i| i.pointer.hover_pos())) {
            let factor = ui.input(|i| i.zoom_delta());
            if factor != 1.0 && rect.contains(pointer) {
                let new_zoom = (self.zoom * factor).clamp(1.0, MAX_ZOOM);
                let ratio = new_zoom / self.zoom;
                // Le point de l'image sous le curseur reste sous le curseur.
                let under = pointer - rect.min;
                self.pending_offset = Some((offset + under * (ratio - 1.0)).max(egui::Vec2::ZERO));
                self.zoom = new_zoom;
            }
        }

        // Le sprite occupe la place qui reste, au-dessus de la rangée des directions; il défile quand il déborde.
        let strip_height = if info.oriented { SPRITE_PX as f32 + 24.0 } else { 0.0 };
        let view_height = (ui.available_height() - strip_height).max(200.0);
        let size = SPRITE_PX as f32 * self.zoom;
        let mut area = egui::ScrollArea::both().max_height(view_height).scroll_source(egui::scroll_area::ScrollSource::ALL);
        if let Some(offset) = self.pending_offset.take() {
            area = area.scroll_offset(offset);
        }
        let output = area.show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
            if let Some(texture) = &self.main_texture {
                ui.painter().rect_filled(rect, 4.0, Color32::from_gray(52));
                // Sans lissage tant qu'on est aux pixels du jeu; les rendus détaillés sont réduits proprement.
                ui.painter().image(texture.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
            }
            rect
        });
        self.last_view = Some((output.inner, output.state.clamped_offset()));

        // Les huit directions côte à côte, rendues quand on ne tire pas un curseur (sinon l'édition devient lente).
        if info.oriented {
            let dragging = ui.input(|i| i.pointer.any_down());
            let strip_key = (self.selected, self.revision, highlight);
            if self.strip_key != Some(strip_key) && !dragging && !self.playing {
                let anim = self.anim.at(self.phase);
                self.strip = (0..DIRECTION_COUNT)
                    .map(|d| load(ui.ctx(), &format!("dir{d}"), &sprite_forge::render_model(&info.key, theme, anim, d, &self.tuning, highlight), SPRITE_PX))
                    .collect();
                self.strip_key = Some(strip_key);
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (i, texture) in self.strip.iter().enumerate() {
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(SPRITE_PX as f32, SPRITE_PX as f32), egui::Sense::click());
                    let selected = i == self.direction;
                    ui.painter().rect_filled(rect, 4.0, if selected { Color32::from_gray(70) } else { Color32::from_gray(45) });
                    ui.painter().image(texture.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                    if response.clicked() {
                        self.direction = i;
                    }
                }
            });
        }
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui, theme: Theme) {
        let index = self.selected;
        let info = self.models[index].clone();
        let base = self.base_params(index, theme).clone();
        let materials = self.materials.entry(index).or_insert_with(|| sprite_forge::model_materials(&info.key, theme)).clone();
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.heading("Réglages");
            if ui.button("Tout réinitialiser").clicked() {
                self.tuning.models.remove(&info.key);
                self.material = None;
                changed = true;
            }
        });
        egui::ScrollArea::vertical().show(ui, |ui| {
            let selected_material = self.material;
            let mut next_material = selected_material;
            let tuned = self.tuning.models.entry(info.key.clone()).or_default();

            ui.horizontal(|ui| {
                ui.label("Taille du modèle");
                changed |= ui.add(egui::Slider::new(&mut tuned.size, 0.5..=1.6)).changed();
                if ui.small_button("↺").clicked() {
                    tuned.size = 1.0;
                    changed = true;
                }
            });

            if !base.is_empty() {
                ui.add_space(6.0);
                egui::CollapsingHeader::new("Proportions du corps").default_open(true).show(ui, |ui| {
                    for (name, default) in base.iter().filter(|(n, _)| n.starts_with("dims.")) {
                        changed |= param_row(ui, tuned, name, *default);
                    }
                });
                if base.iter().any(|(n, _)| n.starts_with("gear.")) {
                    egui::CollapsingHeader::new("Équipement").default_open(true).show(ui, |ui| {
                        for (name, default) in base.iter().filter(|(n, _)| n.starts_with("gear.")) {
                            changed |= param_row(ui, tuned, name, *default);
                        }
                    });
                }
                if base.iter().any(|(n, _)| n.starts_with("face.")) {
                    egui::CollapsingHeader::new("Visage").default_open(false).show(ui, |ui| {
                        for (name, default) in base.iter().filter(|(n, _)| n.starts_with("face.")) {
                            changed |= param_row(ui, tuned, name, *default);
                        }
                    });
                }
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Matières").strong());
                ui.checkbox(&mut self.highlight, "surligner en magenta");
            });
            ui.label(egui::RichText::new("Une matière regroupe toutes les parties de la même couleur d'origine.").weak().small());
            for (original, count) in &materials {
                let entry = tuned.materials.get(original).cloned().unwrap_or_default();
                let shown = entry.color.unwrap_or(*original);
                let is_selected = selected_material == Some(*original);
                ui.horizontal(|ui| {
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(380.0), 22.0), egui::Sense::click());
                    let fill = if is_selected { Color32::from_gray(75) } else { Color32::from_gray(40) };
                    ui.painter().rect_filled(rect, 3.0, fill);
                    let swatch = |x: f32, hex: u32| {
                        let r = egui::Rect::from_min_size(rect.min + egui::vec2(x, 3.0), egui::vec2(28.0, 16.0));
                        ui.painter().rect_filled(r, 2.0, color_of(hex));
                    };
                    swatch(4.0, *original);
                    if entry.color.is_some() {
                        ui.painter().text(rect.min + egui::vec2(36.0, 11.0), egui::Align2::LEFT_CENTER, "→", egui::FontId::proportional(13.0), Color32::GRAY);
                        swatch(50.0, shown);
                    }
                    let mut label = format!("{count} partie{}", if *count > 1 { "s" } else { "" });
                    if !entry.is_default() {
                        label.push_str("  •");
                    }
                    ui.painter().text(rect.min + egui::vec2(90.0, 11.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(13.0), Color32::LIGHT_GRAY);
                    if response.clicked() {
                        next_material = if is_selected { None } else { Some(*original) };
                    }
                });
                if is_selected {
                    let material = tuned.materials.entry(*original).or_default();
                    changed |= material_ui(ui, material, *original);
                }
            }
            tuned.materials.retain(|_, m| !m.is_default());
            self.material = next_material;
        });
        if changed {
            self.revision += 1;
        }
    }
}

/// Réglages détaillés de la matière sélectionnée.
fn material_ui(ui: &mut egui::Ui, material: &mut Material, original: u32) -> bool {
    let mut changed = false;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Couleur");
            let mut color = color_of(material.color.unwrap_or(original));
            if ui.color_edit_button_srgba(&mut color).changed() {
                material.color = Some(hex_of(color));
                changed = true;
            }
            if ui.small_button("↺").clicked() {
                material.color = None;
                changed = true;
            }
            ui.label(egui::RichText::new(format!("d'origine #{original:06x}")).weak().small());
        });
        ui.horizontal(|ui| {
            ui.label("Taille");
            changed |= ui.add(egui::Slider::new(&mut material.size, 0.4..=2.0)).changed();
            if ui.small_button("↺").clicked() {
                material.size = 1.0;
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            let mut custom = material.shine.is_some();
            if ui.checkbox(&mut custom, "Brillance").changed() {
                material.shine = custom.then_some(0.5);
                changed = true;
            }
            if let Some(shine) = &mut material.shine {
                changed |= ui.add(egui::Slider::new(shine, 0.0..=1.0)).changed();
            } else {
                ui.label(egui::RichText::new("d'origine").weak());
            }
        });
        ui.horizontal(|ui| {
            ui.label("Lumineux");
            let label = |g: Option<bool>| match g {
                None => "d'origine",
                Some(true) => "oui",
                Some(false) => "non",
            };
            egui::ComboBox::from_id_salt(("glow", original)).selected_text(label(material.glow)).show_ui(ui, |ui| {
                for option in [None, Some(true), Some(false)] {
                    changed |= ui.selectable_value(&mut material.glow, option, label(option)).changed();
                }
            });
        });
    });
    changed
}

/// Une ligne de réglage de proportion: curseur (ou case à cocher), valeur d'origine, remise à zéro.
fn param_row(ui: &mut egui::Ui, tuned: &mut ModelTuning, name: &str, default: f32) -> bool {
    let mut changed = false;
    let mut value = tuned.params.get(name).copied().unwrap_or(default);
    let label = param_label(name);
    let modified = (value - default).abs() > 1e-5;
    ui.horizontal(|ui| {
        let text = if modified { egui::RichText::new(label).color(Color32::from_rgb(255, 200, 90)) } else { egui::RichText::new(label) };
        if matches!(name, "face.lines" | "face.glasses" | "gear.hat") {
            let mut on = value > 0.5;
            if ui.checkbox(&mut on, text).changed() {
                value = f32::from(on);
                changed = true;
            }
        } else {
            ui.label(text);
            let (lo, hi) = if name.starts_with("dims.") {
                (default * 0.5, default * 1.7)
            } else if name == "face.brow_tilt" || name == "face.brow_arch" {
                (-1.5, 1.5)
            } else {
                (0.0, (default * 2.0).max(2.0))
            };
            changed |= ui.add(egui::Slider::new(&mut value, lo..=hi)).changed();
        }
        if ui.add_enabled(modified, egui::Button::new("↺").small()).clicked() {
            value = default;
            changed = true;
        }
    });
    if changed {
        if (value - default).abs() > 1e-5 {
            tuned.params.insert(name.to_string(), value);
        } else {
            tuned.params.remove(name);
        }
    }
    changed
}

fn param_label(name: &str) -> &'static str {
    match name {
        "dims.thigh" => "Cuisses (longueur)",
        "dims.shin" => "Tibias (longueur)",
        "dims.torso" => "Torse (hauteur)",
        "dims.hip_x" => "Largeur des hanches",
        "dims.shoulder_x" => "Largeur des épaules",
        "dims.upper" => "Bras (longueur)",
        "dims.fore" => "Avant-bras (longueur)",
        "dims.head_r" => "Taille de la tête",
        "dims.head_rise" => "Hauteur de la tête",
        "dims.leg_r" => "Épaisseur des jambes",
        "dims.arm_r" => "Épaisseur des bras",
        "dims.torso_rx" => "Largeur du torse",
        "dims.torso_rz" => "Profondeur du torse",
        "face.eye_size" => "Taille des yeux",
        "face.eye_open" => "Ouverture des yeux",
        "face.eye_spacing" => "Écart des yeux",
        "face.brow_thick" => "Épaisseur des sourcils",
        "face.brow_tilt" => "Inclinaison des sourcils",
        "face.brow_arch" => "Arc des sourcils",
        "face.nose_len" => "Longueur du nez",
        "face.nose_width" => "Largeur du nez",
        "face.smile" => "Sourire",
        "face.mouth_width" => "Largeur de la bouche",
        "face.width" => "Largeur du visage",
        "face.jaw" => "Mâchoire",
        "face.ears" => "Oreilles",
        "face.beard_len" => "Longueur de la barbe",
        "face.lines" => "Rides d'expression",
        "face.glasses" => "Lunettes",
        "gear.hat" => "Couvre-chef (chapeau, casque)",
        _ => "?",
    }
}

/// Réglages sans ce qui ne change rien: valeurs de proportions égales à l'origine, matières neutres, modèles vides.
fn pruned(tuning: &Tuning, base: &HashMap<usize, Vec<(String, f32)>>) -> Tuning {
    let models = sprite_forge::models();
    let mut clean = Tuning::default();
    for (key, tuned) in &tuning.models {
        let mut tuned = tuned.clone();
        if let Some(index) = models.iter().position(|m| &m.key == key) {
            if let Some(defaults) = base.get(&index) {
                tuned.params.retain(|name, value| defaults.iter().find(|(n, _)| n == name).is_none_or(|(_, d)| (*value - d).abs() > 1e-5));
            }
        }
        tuned.materials.retain(|_, m| !m.is_default());
        if tuned != ModelTuning::default() {
            clean.models.insert(key.clone(), tuned);
        }
    }
    clean
}

fn color_of(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn hex_of(color: Color32) -> u32 {
    (u32::from(color.r()) << 16) | (u32::from(color.g()) << 8) | u32::from(color.b())
}

fn load(ctx: &egui::Context, name: &str, rgba: &[u8], side: usize) -> TextureHandle {
    let image = egui::ColorImage::from_rgba_unmultiplied([side, side], rgba);
    // Pixels nets pour le rendu du jeu; les rendus détaillés sont lissés quand on les agrandit.
    let options = if side > SPRITE_PX { TextureOptions::LINEAR } else { TextureOptions::NEAREST };
    ctx.load_texture(name, image, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pruning_drops_settings_that_change_nothing() {
        let mut tuning = Tuning::default();
        let tuned = tuning.models.entry("hero:Knight".into()).or_default();
        tuned.params.insert("dims.thigh".into(), 0.46);
        tuned.params.insert("dims.shin".into(), 0.9);
        tuned.materials.insert(0x112233, Material::default());
        tuned.materials.insert(0x445566, Material { size: 1.5, ..Default::default() });
        tuning.models.entry("hero:Ranger".into()).or_default();
        let base = HashMap::from([(0, vec![("dims.thigh".to_string(), 0.46), ("dims.shin".to_string(), 0.46)])]);
        let clean = pruned(&tuning, &base);
        assert_eq!(clean.models.len(), 1, "le modèle vide disparaît");
        let knight = &clean.models["hero:Knight"];
        assert_eq!(knight.params.keys().collect::<Vec<_>>(), ["dims.shin"]);
        assert_eq!(knight.materials.keys().collect::<Vec<_>>(), [&0x445566]);
    }
}
