//! Les éléments que l'éditeur sait poser, et leurs noms.

use bevy::math::Vec2 as V2;
use bevy_egui::egui;
use castle_shared::{
    DIALOGUE_NODES, ExitKind, InteractKind, MonsterKind, PickupKind, QuestKind, Region, RoomKind, Thing,
};

/// Ce que l'outil « Poser » dépose.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Item {
    Monster(MonsterKind),
    Generator,
    Trap,
    Pickup(PickupKind),
    Relic,
    Captive,
    Exit(ExitKind),
    /// Obstacle qui barre un accès jusqu'à ce que l'objectif soit rempli (pas la sortie).
    Barrier(ExitKind),
    Landmark(castle_shared::LandmarkKind),
    /// Panneau d'énigme dans le mur: Rôdeur (cible), Ensorceleur (rune), Forgebraise (mécanisme).
    Panel(InteractKind),
    /// Herse; `true` quand elle s'étend en hauteur.
    Portcullis(bool),
    Ward,
    /// Samélia; `true` quand elle est prisonnière.
    Guide(bool),
    /// Déclencheur de scène de dialogue.
    Trigger,
}

impl Item {
    pub(super) const MONSTERS: [Item; 11] = [
        Item::Monster(MonsterKind::Skeleton),
        Item::Monster(MonsterKind::Bat),
        Item::Monster(MonsterKind::Goblin),
        Item::Monster(MonsterKind::Orc),
        Item::Monster(MonsterKind::Ghoul),
        Item::Monster(MonsterKind::Wolf),
        Item::Monster(MonsterKind::Warg),
        Item::Monster(MonsterKind::Troll),
        Item::Monster(MonsterKind::Muldant),
        Item::Monster(MonsterKind::Gardener),
        Item::Monster(MonsterKind::Dragon),
    ];

    pub(super) const ELEMENTS: [Item; 4] = [Item::Generator, Item::Trap, Item::Relic, Item::Captive];

    pub(super) const PICKUPS: [Item; 7] = [
        Item::Pickup(PickupKind::Coin),
        Item::Pickup(PickupKind::Food),
        Item::Pickup(PickupKind::Key),
        Item::Pickup(PickupKind::Potion),
        Item::Pickup(PickupKind::Ingredient),
        Item::Pickup(PickupKind::GearPart),
        Item::Pickup(PickupKind::TargetDisc),
    ];

    pub(super) const LANDMARKS: [Item; 4] = [
        Item::Landmark(castle_shared::LandmarkKind::Boulder),
        Item::Landmark(castle_shared::LandmarkKind::Pillar),
        Item::Landmark(castle_shared::LandmarkKind::Statue),
        Item::Landmark(castle_shared::LandmarkKind::Fountain),
    ];

    pub(super) const BARRIERS: [Item; 1] = [Item::Barrier(ExitKind::Bridge)];

    pub(super) const EXITS: [Item; 4] = [Item::Exit(ExitKind::Stairs), Item::Exit(ExitKind::Gate), Item::Exit(ExitKind::Drawbridge), Item::Exit(ExitKind::Hidden)];

    pub(super) const PANELS: [Item; 6] = [
        Item::Panel(InteractKind::Target),
        Item::Panel(InteractKind::Rune),
        Item::Panel(InteractKind::Mechanism),
        Item::Portcullis(false),
        Item::Portcullis(true),
        Item::Ward,
    ];

    pub(super) const SCENES: [Item; 3] = [Item::Guide(false), Item::Guide(true), Item::Trigger];

    pub(super) fn label(self) -> &'static str {
        match self {
            Item::Monster(MonsterKind::Skeleton) => "Squelette",
            Item::Monster(MonsterKind::Bat) => "Chauve-souris",
            Item::Monster(MonsterKind::Goblin) => "Gobelin",
            Item::Monster(MonsterKind::Orc) => "Orque",
            Item::Monster(MonsterKind::Ghoul) => "Goule",
            Item::Monster(MonsterKind::Wolf) => "Loup",
            Item::Monster(MonsterKind::Warg) => "Boss: Croc-Noir (chef de meute)",
            Item::Monster(MonsterKind::Troll) => "Boss: Grogoth (roi-troll)",
            Item::Monster(MonsterKind::Muldant) => "Boss: Vaelor",
            Item::Monster(MonsterKind::Gardener) => "Boss: le Jardinier Noir",
            Item::Monster(MonsterKind::Dragon) => "Boss: le Dragon de la Forge",
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
            Item::Landmark(kind) => kind.label(),
            Item::Exit(ExitKind::Stairs) => "Sortie: escalier",
            Item::Exit(ExitKind::Gate) => "Sortie: barrière",
            Item::Exit(ExitKind::Bridge) => "Sortie: pont cassé (trou)",
            Item::Exit(ExitKind::Drawbridge) => "Sortie: pont-levis",
            Item::Exit(ExitKind::Hidden) => "Sortie invisible (fin de chemin)",
            Item::Barrier(_) => "Pont cassé (obstacle à réparer)",
            Item::Panel(kind) => panel_label(kind),
            Item::Portcullis(false) => "Herse (horizontale)",
            Item::Portcullis(true) => "Herse (verticale)",
            Item::Ward => "Enchantement de la porte royale",
            Item::Guide(false) => "Samélia (debout)",
            Item::Guide(true) => "Samélia (prisonnière)",
            Item::Trigger => "Déclencheur de scène",
        }
    }

    pub(super) fn thing(self, at: V2) -> Thing {
        match self {
            Item::Monster(kind) => Thing::Monster { kind, at },
            Item::Generator => Thing::Generator { at },
            Item::Trap => Thing::Trap { at },
            Item::Pickup(kind) => Thing::Pickup { kind, at },
            Item::Relic => Thing::Relic { at },
            Item::Captive => Thing::Captive { at },
            // Un pont cassé couvre un pont de deux tuiles de large: il se cale sur un coin de tuiles.
            Item::Exit(kind) => Thing::Exit { kind, at: if kind == ExitKind::Bridge { at.round() } else { at } },
            Item::Barrier(kind) => Thing::Barrier { kind, at: if kind == ExitKind::Bridge { at.round() } else { at } },
            Item::Landmark(kind) => Thing::Landmark { kind, at },
            Item::Panel(kind) => Thing::Panel { kind, at, node: default_node(kind).to_string() },
            Item::Portcullis(vertical) => Thing::Portcullis { at, vertical },
            Item::Ward => Thing::Ward { at },
            Item::Guide(caged) => Thing::Guide { at, caged },
            Item::Trigger => Thing::Trigger { node: "intro".to_string(), at, radius: 200.0, delay: 1.0 },
        }
    }
}

pub(super) fn panel_label(kind: InteractKind) -> &'static str {
    match kind {
        InteractKind::Target => "Panneau: cible (Rôdeur)",
        InteractKind::Rune => "Panneau: rune (Ensorceleur)",
        InteractKind::Mechanism => "Panneau: mécanisme (Forgebraise)",
        InteractKind::Push => "Porte poussée (Chevalier)",
    }
}

/// La scène que joue d'ordinaire un panneau.
pub(super) fn default_node(kind: InteractKind) -> &'static str {
    match kind {
        InteractKind::Target => "puzzle_target",
        InteractKind::Rune => "puzzle_rune",
        InteractKind::Mechanism => "puzzle_gears",
        InteractKind::Push => "royal_door",
    }
}

/// Le nom d'un élément, pour les listes et les liens.
pub(super) fn thing_label(thing: &Thing) -> String {
    match thing {
        Thing::Monster { kind, .. } => Item::Monster(*kind).label().to_string(),
        Thing::Pickup { kind, .. } => Item::Pickup(*kind).label().to_string(),
        Thing::Exit { kind, .. } => Item::Exit(*kind).label().to_string(),
        Thing::Barrier { kind, .. } => Item::Barrier(*kind).label().to_string(),
        Thing::Landmark { kind, .. } => kind.label().to_string(),
        Thing::Panel { kind, .. } => panel_label(*kind).to_string(),
        Thing::Portcullis { vertical, .. } => Item::Portcullis(*vertical).label().to_string(),
        Thing::Guide { caged, .. } => Item::Guide(*caged).label().to_string(),
        Thing::Generator { .. } => Item::Generator.label().to_string(),
        Thing::Trap { .. } => Item::Trap.label().to_string(),
        Thing::Relic { .. } => Item::Relic.label().to_string(),
        Thing::Captive { .. } => Item::Captive.label().to_string(),
        Thing::Ward { .. } => Item::Ward.label().to_string(),
        Thing::Trigger { node, .. } => format!("Déclencheur « {node} »"),
    }
}

/// Une liste déroulante qui choisit parmi des valeurs; vrai si le choix a changé.
pub(super) fn choose<T: Copy + PartialEq>(ui: &mut egui::Ui, id: &str, value: &mut T, options: &[(T, &str)]) -> bool {
    let current = options.iter().find(|(v, _)| v == value).map_or("?", |(_, label)| *label);
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(current).show_ui(ui, |ui| {
        for (option, label) in options {
            if ui.selectable_label(value == option, *label).clicked() && value != option {
                *value = *option;
                changed = true;
            }
        }
    });
    changed
}

/// Le choix d'une scène de dialogue (ou aucune).
pub(super) fn scene_picker(ui: &mut egui::Ui, id: &str, node: &mut String) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(if node.is_empty() { "(aucune)" } else { node.as_str() }).show_ui(ui, |ui| {
        if ui.selectable_label(node.is_empty(), "(aucune)").clicked() && !node.is_empty() {
            node.clear();
            changed = true;
        }
        for name in DIALOGUE_NODES {
            if ui.selectable_label(node == name, name).clicked() && node != name {
                *node = name.to_string();
                changed = true;
            }
        }
    });
    changed
}

pub(super) fn region_label(region: Region) -> &'static str {
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

pub(super) fn room_label(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::Start => "Départ",
        RoomKind::Combat => "Combat",
        RoomKind::Treasure => "Trésor",
        RoomKind::Exit => "Sortie",
    }
}

pub(super) fn quest_label(kind: Option<QuestKind>) -> &'static str {
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

