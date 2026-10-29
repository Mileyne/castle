//! Éditeur d'art de Castle.
//!
//! - **Tuiles**: peinture pixel sur les sols, dessus et faces de murs de chaque région. Les retouches sont gardées dans
//!   `tools/sprite_forge/art_overrides/` et recouvrent les tuiles générées: régénérer ne les efface jamais.
//! - **Sprites**: proportions du corps et du visage, et, pour chaque matière d'un modèle, couleur, taille, brillance et
//!   lueur, avec un aperçu en direct sous les huit directions et dans les animations.
//!
//! « Appliquer au jeu » enregistre puis régénère les planches du thème dans `client/assets/art/`.

mod sprites;
mod tiles;

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui;
use sprite_forge::Theme;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Tiles,
    Sprites,
}

struct App {
    theme: Theme,
    tab: Tab,
    status: String,
    tiles: tiles::TilesTab,
    sprites: sprites::SpritesTab,
    /// Génération des planches en cours: le message final arrive ici.
    job: Option<Receiver<String>>,
    repo: PathBuf,
}

fn theme_label(theme: Theme) -> &'static str {
    match theme {
        Theme::Medieval => "Médiéval (le style du jeu)",
        Theme::Manga => "Manga",
        Theme::Gore => "Sanglant",
    }
}

impl App {
    fn new(repo: PathBuf) -> Self {
        let theme = Theme::Medieval;
        Self {
            theme,
            tab: if std::env::args().any(|a| a == "--tuiles") { Tab::Tiles } else { Tab::Sprites },
            status: "Prêt".into(),
            tiles: tiles::TilesTab::new(),
            sprites: sprites::SpritesTab::new(theme),
            job: None,
            repo,
        }
    }

    fn dirty(&self) -> bool {
        self.tiles.dirty() || self.sprites.dirty()
    }

    fn save(&mut self) -> bool {
        let result = self.sprites.save(self.theme).and_then(|()| self.tiles.save(self.theme));
        match result {
            Ok(()) => {
                self.status = "Enregistré".into();
                true
            }
            Err(err) => {
                self.status = format!("Échec de l'enregistrement: {err}");
                false
            }
        }
    }

    /// Enregistre, puis régénère toutes les planches du thème dans les assets du jeu.
    fn apply(&mut self, ctx: &egui::Context) {
        if self.job.is_some() || !self.save() {
            return;
        }
        let theme = self.theme;
        let out = self.repo.join("client/assets/art").join(theme.folder());
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        self.status = "Génération des planches...".into();
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let outcome = std::panic::catch_unwind(|| sprite_forge::generate(theme, &out, None));
            let message = match outcome {
                Ok(()) => format!("Planches régénérées en {:.1} s: relance le jeu pour les voir", started.elapsed().as_secs_f32()),
                Err(_) => "La génération a échoué (voir le terminal)".to_string(),
            };
            let _ = tx.send(message);
            ctx.request_repaint();
        });
        self.job = Some(rx);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(job) = &self.job {
            if let Ok(message) = job.try_recv() {
                self.status = message;
                self.job = None;
            }
        }
        let busy = self.job.is_some();
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) && !busy {
            self.save();
        }

        egui::Panel::top("bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Sprites, "Sprites");
                ui.selectable_value(&mut self.tab, Tab::Tiles, "Tuiles");
                ui.separator();
                let dirty = self.dirty();
                ui.add_enabled_ui(!dirty && !busy, |ui| {
                    egui::ComboBox::from_id_salt("theme").selected_text(theme_label(self.theme)).show_ui(ui, |ui| {
                        for theme in Theme::ALL {
                            if ui.selectable_label(self.theme == theme, theme_label(theme)).clicked() && theme != self.theme {
                                self.theme = theme;
                                self.tiles = tiles::TilesTab::new();
                                self.sprites = sprites::SpritesTab::new(theme);
                            }
                        }
                    });
                })
                .response
                .on_disabled_hover_text("Enregistre d'abord les changements en cours");
                ui.separator();
                if ui.add_enabled(dirty && !busy, egui::Button::new("Enregistrer (Ctrl+S)")).clicked() {
                    self.save();
                }
                if ui.add_enabled(!busy, egui::Button::new("Appliquer au jeu")).on_hover_text("Enregistre et régénère les planches du jeu (quelques secondes)").clicked() {
                    self.apply(&ctx);
                }
                if dirty {
                    ui.colored_label(egui::Color32::from_rgb(255, 190, 80), "● modifications non enregistrées");
                }
                if busy {
                    ui.spinner();
                }
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(&self.status);
        });
        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Sprites => self.sprites.ui(ui, self.theme, busy),
            Tab::Tiles => self.tiles.ui(ui, self.theme),
        });
    }
}

fn main() -> eframe::Result {
    // La génération lit `photos/` depuis la racine du dépôt.
    let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).canonicalize().expect("racine du dépôt");
    std::env::set_current_dir(&repo).expect("racine du dépôt");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1500.0, 920.0]).with_title("Castle - Éditeur d'art"),
        ..Default::default()
    };
    eframe::run_native("Castle - Éditeur d'art", options, Box::new(move |cc| {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Ok(Box::new(App::new(repo)))
    }))
}
