//! Onglet « Équilibrage »: toutes les valeurs de jeu réglables (`shared/balance.ron`): points de vie et attaques des héros, monstres,
//! tables d'apparition, butin, pièges, pouvoirs, compagnons. Le formulaire se construit à partir des valeurs elles-mêmes (une
//! valeur ajoutée à `Balance` apparaît donc toute seule); les champs modifiés sont en orange, avec un bouton pour revenir à la valeur
//! enregistrée. « Essayer dans le jeu » enregistre puis lance le client avec ces réglages.

use std::path::PathBuf;

use castle_shared::Balance;
use eframe::egui::{self, Color32};
use serde_json::{Map, Value};

const HEADER: &str = "// Équilibrage de Castle: points de vie, dégâts, portées, apparitions, butin, pièges, pouvoirs, compagnons.\n\
// Édité dans l'éditeur (onglet « Équilibrage »). `CASTLE_BALANCE_FILE=<fichier>` lance le jeu avec un autre fichier, sans recompiler.\n";

/// Les monstres qu'une table d'apparition peut contenir.
const MONSTERS: [&str; 11] = ["Skeleton", "Bat", "Goblin", "Orc", "Ghoul", "Wolf", "Warg", "Troll", "Vaelor", "Gardener", "Dragon"];

const CHANGED: Color32 = Color32::from_rgb(255, 190, 80);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Heroes,
    Monsters,
    Spawn,
    Loot,
    Hazards,
    Rules,
}

impl Page {
    const ALL: [Page; 6] = [Page::Heroes, Page::Monsters, Page::Spawn, Page::Loot, Page::Hazards, Page::Rules];

    fn title(self) -> &'static str {
        match self {
            Page::Heroes => "Héros",
            Page::Monsters => "Monstres",
            Page::Spawn => "Apparitions et niveaux",
            Page::Loot => "Butin et objets",
            Page::Hazards => "Pièges et pouvoirs",
            Page::Rules => "Règles",
        }
    }

    /// Une phrase pour situer la page.
    fn intro(self) -> &'static str {
        match self {
            Page::Heroes => "Une colonne par héros. Les distances sont en pixels (une tuile en fait 16), les durées en secondes. Les cases grisées ne servent pas à cette classe. Les lignes « Compagnon » règlent le héros quand le joueur ne le dirige pas.",
            Page::Monsters => "Une colonne par monstre; les boss sont à droite. Les distances sont en pixels (une tuile en fait 16), les durées en secondes.",
            Page::Spawn => "Quels monstres sortent des générateurs, et combien il y en a. Le poids d'un monstre est relatif: seule sa part dans la table compte.",
            Page::Loot => "Ce que laissent les monstres, générateurs, boss et prisonniers, et ce que rendent les objets ramassés.",
            Page::Hazards => "Les pièges des salles et les pouvoirs spéciaux des héros.",
            Page::Rules => "Règles générales de relève, de portes à clé, de portée des monstres et de sortie du niveau.",
        }
    }

    /// Les sections de `Balance` montrées sur cette page.
    fn sections(self) -> &'static [&'static str] {
        match self {
            Page::Heroes => &["heroes"],
            Page::Monsters => &["monsters"],
            Page::Spawn => &["spawn"],
            Page::Loot => &["drops", "pickups"],
            Page::Hazards => &["traps", "abilities"],
            Page::Rules => &["rules"],
        }
    }
}

pub struct BalanceTab {
    path: PathBuf,
    repo: PathBuf,
    /// Les valeurs en cours d'édition, et celles du fichier enregistré.
    current: Value,
    saved: Value,
    page: Page,
    message: String,
}

impl BalanceTab {
    pub fn new(repo: PathBuf) -> Self {
        let path = repo.join("shared/balance.ron");
        let (balance, message) = match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Balance::parse(&t).map_err(|e| e.to_string())) {
            Ok(balance) => (balance, String::new()),
            Err(err) => (Balance::embedded(), format!("balance.ron illisible, valeurs du programme: {err}")),
        };
        let value = serde_json::to_value(&balance).expect("équilibrage sérialisable");
        Self { path, repo, current: value.clone(), saved: value, page: Page::Heroes, message }
    }

    pub fn dirty(&self) -> bool {
        self.current != self.saved
    }

    pub fn save(&mut self) -> Result<(), String> {
        if !self.dirty() {
            return Ok(());
        }
        let balance: Balance = serde_json::from_value(self.current.clone()).map_err(|e| format!("valeurs invalides: {e}"))?;
        std::fs::write(&self.path, format!("{HEADER}{}\n", balance.to_text())).map_err(|e| e.to_string())?;
        self.saved = self.current.clone();
        self.message = "Équilibrage enregistré (shared/balance.ron)".into();
        Ok(())
    }

    /// Enregistre, puis lance le jeu avec ces réglages (sans attendre une recompilation des valeurs).
    fn test_in_game(&mut self) {
        if let Err(err) = self.save() {
            self.message = format!("Échec de l'enregistrement: {err}");
            return;
        }
        let spawned = std::process::Command::new("cargo")
            .args(["run", "--release", "-p", "castle_client"])
            .current_dir(&self.repo)
            .env("CASTLE_BALANCE_FILE", &self.path)
            .spawn();
        self.message = match spawned {
            Ok(_) => "Jeu lancé avec ces réglages.".into(),
            Err(err) => format!("Impossible de lancer le jeu: {err}"),
        };
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for page in Page::ALL {
                ui.selectable_value(&mut self.page, page, page.title());
            }
            ui.separator();
            if ui.button("▶ Essayer dans le jeu").on_hover_text("Enregistre puis lance le jeu avec ces réglages").clicked() {
                self.test_in_game();
            }
            if ui.add_enabled(self.dirty(), egui::Button::new("↺ Tout rétablir")).on_hover_text("Revient aux valeurs enregistrées").clicked() {
                self.current = self.saved.clone();
            }
        });
        ui.label(egui::RichText::new(self.page.intro()).weak());
        if !self.message.is_empty() {
            ui.label(&self.message);
        }
        ui.separator();
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            for key in self.page.sections() {
                let (Some(current), Some(saved)) = (self.current.get_mut(*key), self.saved.get(*key)) else {
                    continue;
                };
                match self.page {
                    Page::Heroes | Page::Monsters => matrix(ui, key, current, saved),
                    _ => {
                        ui.add_space(6.0);
                        ui.heading(label(key));
                        edit(ui, key, current, Some(saved));
                    }
                }
            }
        });
    }
}

/// Le nom affiché d'un champ ou d'une section.
fn label(key: &str) -> String {
    let known = match key {
        "heroes" => "Héros",
        "monsters" => "Monstres",
        "spawn" => "Apparitions et niveaux",
        "drops" => "Butin",
        "pickups" => "Objets ramassés",
        "traps" => "Pièges",
        "abilities" => "Pouvoirs spéciaux",
        "rules" => "Règles",
        "knight" => "Chevalier",
        "sorcerer" => "Ensorceleur",
        "ranger" => "Rôdeur",
        "forgebraise" => "Forgebraise",
        "skeleton" => "Squelette",
        "bat" => "Chauve-souris",
        "goblin" => "Gobelin",
        "orc" => "Orc",
        "ghoul" => "Goule",
        "wolf" => "Loup",
        "warg" => "Warg (boss)",
        "troll" => "Troll (boss)",
        "vaelor" => "Vaelor (boss)",
        "gardener" => "Jardinier (boss)",
        "dragon" => "Dragon (boss)",
        "max_hp" => "Points de vie max",
        "speed" => "Vitesse",
        "max_mana" => "Mana max",
        "mana_regen" => "Mana par seconde",
        "special_cost" => "Coût du pouvoir",
        "special_cooldown" => "Recharge du pouvoir (s)",
        "attack_damage" => "Dégâts du coup",
        "attack_cooldown" => "Délai entre deux coups (s)",
        "attack_range" => "Portée du coup (corps à corps)",
        "shot_speed" => "Vitesse du tir (tireurs)",
        "shot_lifetime" => "Durée de vie du tir (s)",
        "damage" => "Dégâts",
        "cooldown" => "Délai (s)",
        "half_size" => "Demi-taille",
        "sight" => "Portée de vue",
        "castle_level_1" => "Château, niveau 1",
        "castle_level_2" => "Château, niveau 2",
        "castle_level_3_and_more" => "Château, niveau 3 et suivants",
        "forest" => "Forêt",
        "marsh" => "Marais",
        "approach" => "Abords du château",
        "elf_fauna" => "Royaume des elfes",
        "monsters_per_room_base" => "Monstres par salle: base",
        "monsters_per_room_every_levels" => "Monstres par salle: +1 tous les … niveaux",
        "monsters_per_room_max" => "Monstres par salle: maximum",
        "potions_per_level_base" => "Potions par niveau: base",
        "potions_levels_cap" => "Potions par niveau: plafond du niveau compté",
        "potions_every_levels" => "Potions par niveau: +1 tous les … niveaux",
        "easing_start" => "Douceur du niveau 1 (facteur)",
        "easing_step" => "Douceur: gain par niveau",
        "trap_room_chance" => "Chance qu'une salle ait des pièges",
        "traps_per_room" => "Pièges par salle concernée",
        "max_monsters" => "Monstres vivants au maximum",
        "generator_health" => "Générateur: points de vie",
        "generator_activation" => "Générateur: distance d'activation",
        "generator_interval" => "Générateur: délai entre deux monstres (s)",
        "monster_coin_chance" => "Monstre: chance de pièce",
        "monster_food_chance" => "Monstre: chance de nourriture",
        "monster_potion_chance" => "Monstre: chance de potion",
        "generator_coins" => "Générateur détruit: pièces",
        "boss_coins" => "Boss vaincu: pièces",
        "rescue_coins" => "Prisonnier délivré: pièces",
        "pickup_range" => "Portée de ramassage",
        "food_heal" => "Nourriture: PV rendus",
        "potion_duration" => "Potion: durée (s)",
        "potion_heal_per_second" => "Potion: part des PV max rendue par seconde",
        "trigger_radius" => "Rayon de déclenchement",
        "disarm_time" => "Temps de désamorçage (s)",
        "wake_radius" => "Rayon de réveil",
        "sleep_delay" => "Délai avant de se rendormir (s)",
        "aegis_duration" => "Égide: durée de l'immunité (s)",
        "freeze_radius" => "Gel: rayon",
        "freeze_duration" => "Gel: durée (s)",
        "freeze_boss_factor" => "Gel: part de la durée pour un boss",
        "volley_arrows" => "Salve: nombre de flèches",
        "volley_spread" => "Salve: écart de l'éventail (degrés)",
        "volley_damage_factor" => "Salve: dégâts par flèche (x)",
        "smash_offset" => "Frappe titanesque: distance devant",
        "smash_radius" => "Frappe titanesque: rayon",
        "smash_factor" => "Frappe titanesque: dégâts (x le marteau)",
        "companion_aggro" => "Compagnon: distance de détection des ennemis",
        "companion_shot_reach" => "Compagnon: portée de tir",
        "companion_leash" => "Compagnon: distance de laisse (retour auprès du chef)",
        "companion_group_radius" => "Compagnon: n'engage que les ennemis à cette distance du chef",
        "companion_damage_factor" => "Compagnon: part des dégâts d'un héros dirigé",
        "companion_cooldown_factor" => "Compagnon: lenteur des coups (× le délai)",
        "companion_food_radius" => "Compagnon: distance où il va chercher la nourriture",
        "companion_leash_far" => "Compagnon: très loin du chef, ramené tout de suite",
        "companion_leash_patience" => "Compagnon: entre les deux, ramené après (s)",
        "revive_range" => "Relever un allié: portée",
        "revive_time" => "Relever un allié: durée (s)",
        "door_open_range" => "Porte à clé: distance d'ouverture",
        "monster_reach_slack" => "Marge de portée des monstres",
        "exit_radius" => "Sortie: rayon",
        "exit_delay" => "Sortie: délai (s)",
        _ => return key.to_string(),
    };
    known.to_string()
}


/// Ce que mesure une valeur, en une ou deux phrases (les distances sont en pixels, 16 pixels font une tuile).
fn help(key: &str) -> &'static str {
    match key {
        // Héros et monstres
        "max_hp" => "Points de vie au maximum. À zéro, un héros tombe à terre (un allié peut le relever) et un monstre meurt.",
        "speed" => "Vitesse de déplacement, en pixels par seconde.",
        "max_mana" => "Réserve de mana. Chaque pouvoir spécial en consomme.",
        "mana_regen" => "Mana regagné chaque seconde.",
        "special_cost" => "Mana dépensé à chaque pouvoir spécial.",
        "special_cooldown" => "Délai minimal entre deux pouvoirs spéciaux, en secondes.",
        "attack_damage" => "Dégâts d'un coup ou d'un tir de base.",
        "attack_cooldown" => "Délai entre deux coups ou deux tirs, en secondes. Plus il est petit, plus le héros frappe vite.",
        "attack_range" => "Corps à corps seulement (Chevalier, Forgebraise): portée du coup, en pixels.",
        "shot_speed" => "Tireurs seulement (Ensorceleur, Rôdeur): vitesse du projectile, en pixels par seconde.",
        "shot_lifetime" => "Tireurs seulement: durée de vol du projectile, en secondes. Sa portée est vitesse × durée.",
        "damage" => "Dégâts infligés à un héros à chaque coup (un coup est divisé par deux sous le Rempart).",
        "cooldown" => "Délai entre deux coups du monstre, en secondes.",
        "half_size" => "Demi-taille du corps, en pixels: elle décide des collisions et de la portée de frappe.",
        "sight" => "Distance à laquelle le monstre repère un héros, en pixels (réduite dans les premiers niveaux).",
        // Apparitions
        "castle_level_1" | "castle_level_2" | "castle_level_3_and_more" => "Monstres produits par les générateurs des salles du château à ces niveaux.",
        "forest" | "marsh" | "approach" => "Monstres produits par les générateurs de cette région extérieure.",
        "elf_fauna" => "Monstres des niveaux du monde des elfes.",
        "monsters_per_room_base" => "Monstres posés d'avance dans une salle de combat au niveau 0, avant les ajouts par niveau.",
        "monsters_per_room_every_levels" => "Un monstre de plus par salle tous les … niveaux.",
        "monsters_per_room_max" => "Nombre maximal de monstres posés d'avance dans une salle.",
        "potions_per_level_base" => "Potions de régénération posées dans un niveau, avant les ajouts par niveau.",
        "potions_levels_cap" => "Au-delà de ce niveau, il n'y a plus de potion supplémentaire.",
        "potions_every_levels" => "Une potion de plus tous les … niveaux.",
        "easing_start" => "Facteur de douceur au niveau 1: il réduit la vitesse et la vue des monstres et le rythme des générateurs (1 = pleine force).",
        "easing_step" => "Gain de ce facteur à chaque niveau, jusqu'à 1.",
        "trap_room_chance" => "Chance (0 à 1) qu'une salle de combat ait des pièges.",
        "traps_per_room" => "Pièges posés dans une salle qui en a.",
        "max_monsters" => "Nombre maximal de monstres vivants dans une salle: les générateurs s'arrêtent au-delà.",
        "generator_health" => "Points de vie d'un générateur de monstres.",
        "generator_activation" => "Un générateur ne produit que si un héros est plus près que cette distance (pixels).",
        "generator_interval" => "Délai entre deux monstres produits par un générateur, en secondes.",
        // Butin
        "monster_coin_chance" => "Chance (0 à 1) qu'un monstre tué laisse une pièce.",
        "monster_food_chance" => "Chance (0 à 1) qu'un monstre tué laisse de la nourriture.",
        "monster_potion_chance" => "Chance (0 à 1) qu'un monstre tué laisse une potion. Les trois chances s'additionnent: leur somme reste sous 1.",
        "generator_coins" => "Pièces laissées par un générateur détruit.",
        "boss_coins" => "Pièces laissées par un boss vaincu.",
        "rescue_coins" => "Pièces laissées par un prisonnier délivré.",
        "pickup_range" => "Distance à laquelle un héros ramasse un objet, en pixels.",
        "food_heal" => "PV rendus par une ration. Un héros ne la mange que s'il lui manque au moins ce nombre de PV.",
        "potion_duration" => "Durée de la régénération d'une potion, en secondes (pour toute l'équipe).",
        "potion_heal_per_second" => "Part des PV maximum rendue chaque seconde par une potion (0,06 = 6 %).",
        // Pièges
        "trigger_radius" => "Distance à laquelle un piège réveillé blesse, en pixels.",
        "disarm_time" => "Temps que le Rôdeur doit maintenir l'action près d'un piège pour le désamorcer, en secondes.",
        "wake_radius" => "Un héros plus près que cette distance réveille le piège (pointes sorties), en pixels.",
        "sleep_delay" => "Délai après le départ du dernier héros avant que le piège se rendorme, en secondes.",
        // Pouvoirs
        "aegis_duration" => "Égide du Chevalier: durée pendant laquelle toute l'équipe ne subit aucun dégât, en secondes.",
        "freeze_radius" => "Gel de l'Ensorceleur: rayon autour de lui où les monstres sont figés, en pixels.",
        "freeze_duration" => "Gel: durée pendant laquelle un monstre est figé, en secondes.",
        "freeze_boss_factor" => "Gel: part de cette durée qui s'applique à un boss (0,3 = 30 %).",
        "volley_arrows" => "Salve du Rôdeur: nombre de flèches tirées d'un coup.",
        "volley_spread" => "Salve: angle total de l'éventail, en degrés.",
        "volley_damage_factor" => "Salve: dégâts de chaque flèche, en multiple d'une flèche normale.",
        "smash_offset" => "Frappe titanesque du Forgebraise: distance où elle tombe devant lui, en pixels.",
        "smash_radius" => "Frappe titanesque: rayon de l'impact, en pixels.",
        "smash_factor" => "Frappe titanesque: dégâts, en multiple d'un coup de marteau normal.",
        // Compagnons et règles
        "companion_aggro" => "Un héros non dirigé engage un ennemi plus près que cette distance (pixels), s'il le voit.",
        "companion_shot_reach" => "Tireurs seulement: portée à laquelle il tire quand il n'est pas dirigé, en pixels.",
        "companion_leash" => "Distance au-delà de laquelle il cherche à rejoindre le héros dirigé, en pixels.",
        "companion_group_radius" => "Il n'engage que les ennemis à cette distance du héros dirigé, en pixels.",
        "companion_damage_factor" => "Part des dégâts d'un héros dirigé que font ses coups (0,6 = 60 %).",
        "companion_cooldown_factor" => "Il frappe plus lentement: son délai est celui d'un héros dirigé multiplié par cette valeur.",
        "companion_food_radius" => "Quand il peut manger, il va chercher la nourriture à cette distance au plus, en pixels.",
        "companion_leash_far" => "Plus loin que cela du héros dirigé, il est téléporté tout de suite auprès de lui, en pixels.",
        "companion_leash_patience" => "Entre la distance de laisse et la précédente, s'il n'arrive pas à rejoindre le groupe, il est ramené après ce délai, en secondes.",
        "revive_range" => "Distance à laquelle un allié relève un héros à terre, en pixels.",
        "revive_time" => "Temps nécessaire pour relever un héros à terre, en secondes. Le héros relevé repart avec la moitié de ses PV.",
        "door_open_range" => "Distance à laquelle une porte à clé s'ouvre devant un héros muni d'une clé, en pixels.",
        "monster_reach_slack" => "Marge ajoutée à la portée de frappe d'un monstre, en pixels: les corps se bloquent avant de se toucher.",
        "exit_radius" => "La sortie s'active quand tous les héros debout sont dans ce rayon, en pixels.",
        "exit_delay" => "Temps passé dans ce rayon avant que le niveau se termine, en secondes.",
        _ => "",
    }
}

/// Les cases qui ne servent pas à une classe: la portée de coup des tireurs et les valeurs de tir des combattants au corps à corps.
fn applicable(section: &str, column: &str, row: &str) -> bool {
    if section != "heroes" {
        return true;
    }
    let melee = matches!(column, "knight" | "forgebraise");
    match row {
        "attack_range" => melee,
        "shot_speed" | "shot_lifetime" | "companion_shot_reach" => !melee,
        _ => true,
    }
}

/// Un tableau: une colonne par personnage (héros ou monstre), une ligne par caractéristique, avec son explication.
fn matrix(ui: &mut egui::Ui, id: &str, value: &mut Value, original: &Value) {
    let Value::Object(columns) = value else {
        return;
    };
    let original = original.as_object();
    let column_keys: Vec<String> = columns.keys().cloned().collect();
    let row_keys: Vec<String> = columns.values().next().and_then(Value::as_object).map(|m| m.keys().cloned().collect()).unwrap_or_default();
    egui::Grid::new(format!("matrix-{id}")).striped(true).spacing([18.0, 10.0]).show(ui, |ui| {
        ui.label("");
        for column in &column_keys {
            ui.label(egui::RichText::new(label(column)).strong());
        }
        ui.end_row();
        for row in &row_keys {
            // Un intitulé avant le premier réglage de compagnon.
            if row == "companion_aggro" {
                ui.label(egui::RichText::new("Quand le héros n'est pas dirigé par le joueur (compagnon)").strong().color(Color32::LIGHT_BLUE));
                for _ in &column_keys {
                    ui.label("");
                }
                ui.end_row();
            }
            let changed = column_keys.iter().any(|c| {
                let now = columns.get(c).and_then(|v| v.get(row));
                let before = original.and_then(|o| o.get(c)).and_then(|v| v.get(row));
                before.is_some() && now != before
            });
            ui.vertical(|ui| {
                ui.set_width(340.0);
                let name = egui::RichText::new(label(row)).strong();
                ui.label(if changed { name.color(CHANGED) } else { name });
                ui.label(egui::RichText::new(help(row)).small().weak());
            });
            for column in &column_keys {
                let before = original.and_then(|o| o.get(column)).and_then(|v| v.get(row));
                let usable = applicable(id, column, row);
                if let Some(cell) = columns.get_mut(column).and_then(|v| v.get_mut(row)) {
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(usable, |ui| leaf(ui, &format!("{id}-{column}-{row}"), cell, before));
                    });
                }
            }
            ui.end_row();
        }
    });
}

/// Édite une valeur: nombre, objet (champs en grille, sous-objets en dessous) ou liste (table d'apparition).
fn edit(ui: &mut egui::Ui, key: &str, value: &mut Value, original: Option<&Value>) {
    match value {
        Value::Object(map) => edit_object(ui, key, map, original),
        Value::Array(list) => edit_list(ui, key, list, original),
        _ => {
            ui.horizontal(|ui| leaf(ui, key, value, original));
        }
    }
}

fn edit_object(ui: &mut egui::Ui, key: &str, map: &mut Map<String, Value>, original: Option<&Value>) {
    let original = original.and_then(Value::as_object);
    // Les champs simples d'abord, en grille (nom et explication, puis la valeur); les listes en dessous.
    egui::Grid::new(format!("grid-{key}")).striped(true).spacing([18.0, 10.0]).show(ui, |ui| {
        for (name, value) in map.iter_mut().filter(|(_, v)| !v.is_object() && !v.is_array()) {
            let before = original.and_then(|o| o.get(name));
            let changed = before.is_some_and(|b| b != &*value);
            ui.vertical(|ui| {
                ui.set_width(420.0);
                let text = egui::RichText::new(label(name)).strong();
                ui.label(if changed { text.color(CHANGED) } else { text });
                ui.label(egui::RichText::new(help(name)).small().weak());
            });
            ui.horizontal(|ui| leaf(ui, &format!("{key}-{name}"), value, before));
            ui.end_row();
        }
    });
    for (name, value) in map.iter_mut().filter(|(_, v)| v.is_object() || v.is_array()) {
        let before = original.and_then(|o| o.get(name));
        let changed = before.is_some_and(|b| b != &*value);
        ui.add_space(8.0);
        let title = egui::RichText::new(label(name)).strong();
        ui.label(if changed { title.color(CHANGED) } else { title });
        ui.label(egui::RichText::new(help(name)).small().weak());
        edit(ui, &format!("{key}.{name}"), value, before);
    }
}

/// Un nombre ou le nom d'un monstre, avec un bouton pour revenir à la valeur enregistrée.
fn leaf(ui: &mut egui::Ui, name: &str, value: &mut Value, original: Option<&Value>) {
    match value {
        Value::Number(n) if n.is_u64() || n.is_i64() => {
            let mut v = n.as_i64().unwrap_or(0);
            if ui.add(egui::DragValue::new(&mut v).speed(1.0).range(0..=100_000)).changed() {
                *value = Value::from(v.max(0));
            }
        }
        Value::Number(n) => {
            let mut v = n.as_f64().unwrap_or(0.0);
            let speed = (v.abs() * 0.01).max(0.01);
            if ui.add(egui::DragValue::new(&mut v).speed(speed).range(0.0..=100_000.0).max_decimals(3)).changed() {
                // Trois décimales suffisent (et évitent des valeurs comme 0.30000001).
                *value = serde_json::Number::from_f64((v * 1000.0).round() / 1000.0).map_or(Value::Null, Value::Number);
            }
        }
        Value::String(text) => {
            egui::ComboBox::from_id_salt(name).selected_text(label(&text.to_lowercase())).show_ui(ui, |ui| {
                for monster in MONSTERS {
                    ui.selectable_value(text, monster.to_string(), label(&monster.to_lowercase()));
                }
            });
        }
        _ => {
            ui.label(value.to_string());
        }
    }
    if let Some(orig) = original {
        if orig != &*value && ui.small_button("↺").on_hover_text(format!("Valeur enregistrée: {orig}")).clicked() {
            *value = orig.clone();
        }
    }
}

/// Une table d'apparition: un tableau (monstre, poids, part dans le tirage).
fn edit_list(ui: &mut egui::Ui, key: &str, list: &mut Vec<Value>, original: Option<&Value>) {
    let original = original.and_then(Value::as_array);
    let total: f64 = list.iter().filter_map(|w| w.get("weight").and_then(Value::as_f64)).sum();
    let mut remove = None;
    egui::Grid::new(format!("table-{key}")).striped(true).spacing([18.0, 6.0]).show(ui, |ui| {
        ui.label(egui::RichText::new("Monstre").strong());
        ui.label(egui::RichText::new("Poids").strong());
        ui.label(egui::RichText::new("Part").strong());
        ui.label("");
        ui.end_row();
        for (i, entry) in list.iter_mut().enumerate() {
            let before = original.and_then(|o| o.get(i));
            let Value::Object(map) = entry else {
                continue;
            };
            let changed = before.is_some_and(|b| b != &Value::Object(map.clone()));
            if let Some(monster) = map.get_mut("monster") {
                leaf(ui, &format!("{key}-{i}"), monster, before.and_then(|o| o.get("monster")));
            }
            if let Some(weight) = map.get_mut("weight") {
                ui.horizontal(|ui| leaf(ui, &format!("{key}-{i}-w"), weight, before.and_then(|o| o.get("weight"))));
                let share = weight.as_f64().unwrap_or(0.0) / total.max(f64::EPSILON) * 100.0;
                let text = egui::RichText::new(format!("{share:.0} %"));
                ui.label(if changed { text.color(CHANGED) } else { text.weak() });
            }
            if ui.small_button("✖").on_hover_text("Retirer de la table").clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        list.remove(i);
    }
    if ui.small_button("+ Ajouter un monstre").clicked() {
        list.push(serde_json::json!({ "monster": "Goblin", "weight": 0.1 }));
    }
    if list.iter().all(|w| w.get("weight").and_then(Value::as_f64).unwrap_or(0.0) <= 0.0) {
        ui.colored_label(Color32::LIGHT_RED, "Une table doit contenir au moins un monstre de poids positif.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le formulaire travaille sur du JSON: les valeurs doivent revenir telles quelles en `Balance`, y compris un entier ou un décimal modifié.
    #[test]
    fn the_form_values_round_trip_and_edits_are_read_back() {
        let balance = Balance::embedded();
        let mut value = serde_json::to_value(&balance).unwrap();
        assert_eq!(serde_json::from_value::<Balance>(value.clone()).unwrap(), balance);
        value["heroes"]["knight"]["max_hp"] = Value::from(300);
        value["monsters"]["orc"]["damage"] = serde_json::Number::from_f64(7.5).map_or(Value::Null, Value::Number);
        value["spawn"]["forest"][0]["monster"] = Value::from("Orc");
        let edited: Balance = serde_json::from_value(value).unwrap();
        assert_eq!(edited.heroes.knight.max_hp, 300);
        assert_eq!(edited.monsters.orc.damage, 7.5);
        assert_eq!(edited.spawn.forest[0].monster, castle_shared::MonsterKind::Orc);
    }
}
