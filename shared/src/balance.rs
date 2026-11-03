//! Équilibrage du jeu: toutes les valeurs de jeu réglables (points de vie, dégâts, portées, apparitions, butin, pièges, pouvoirs,
//! compagnons) sont réunies dans `balance.ron`, que l'éditeur (onglet « Équilibrage ») modifie.
//!
//! Le fichier est compilé dans le programme. Pour essayer des réglages sans recompiler, `CASTLE_BALANCE_FILE=<fichier>` fait lire ce
//! fichier à la place (un fichier absent ou illisible retombe sur celui du programme). Ce qui est de l'ordre de la structure (quel
//! héros tire ou frappe, quel pouvoir a chaque classe, les noms de traduction) reste dans le code.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::{MonsterKind, MonsterStats};

const EMBEDDED: &str = include_str!("../balance.ron");

/// Toutes les valeurs d'équilibrage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    pub heroes: Heroes,
    pub monsters: Monsters,
    pub spawn: Spawn,
    pub drops: Drops,
    pub pickups: Pickups,
    pub traps: Traps,
    pub abilities: Abilities,
    pub rules: Rules,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Heroes {
    pub knight: Hero,
    pub sorcerer: Hero,
    pub ranger: Hero,
    pub forgebraise: Hero,
}

/// Valeurs d'un héros. Une classe qui frappe au corps à corps lit `attack_range` (portée du coup); une classe qui tire lit `shot_speed`
/// et `shot_lifetime` (la portée du tir est leur produit). L'autre jeu de valeurs est ignoré.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hero {
    pub max_hp: u32,
    pub speed: f32,
    pub max_mana: f32,
    /// Mana regagné par seconde.
    pub mana_regen: f32,
    pub special_cost: f32,
    /// Temps de recharge du pouvoir spécial, en secondes.
    pub special_cooldown: f32,
    pub attack_damage: f32,
    pub attack_cooldown: f32,
    pub attack_range: f32,
    pub shot_speed: f32,
    pub shot_lifetime: f32,
    // Quand le héros n'est pas dirigé par le joueur (compagnon).
    /// Distance de détection des ennemis, portée de tir et retour auprès du chef.
    pub companion_aggro: f32,
    pub companion_shot_reach: f32,
    pub companion_leash: f32,
    /// Il n'engage que les ennemis à cette distance du héros dirigé.
    pub companion_group_radius: f32,
    /// Ses coups font moins mal et sont plus lents que ceux d'un héros dirigé.
    pub companion_damage_factor: f32,
    pub companion_cooldown_factor: f32,
    /// Distance à laquelle il va chercher la nourriture quand il peut manger.
    pub companion_food_radius: f32,
    /// Très loin du chef, il est ramené tout de suite; entre `companion_leash` et `companion_leash_far`, après `companion_leash_patience` secondes.
    pub companion_leash_far: f32,
    pub companion_leash_patience: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Monsters {
    pub skeleton: MonsterStats,
    pub bat: MonsterStats,
    pub goblin: MonsterStats,
    pub orc: MonsterStats,
    pub ghoul: MonsterStats,
    pub wolf: MonsterStats,
    pub warg: MonsterStats,
    pub troll: MonsterStats,
    pub muldant: MonsterStats,
    pub gardener: MonsterStats,
    pub dragon: MonsterStats,
}

/// Un monstre d'une table d'apparition, avec son poids relatif.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpawnWeight {
    pub monster: MonsterKind,
    pub weight: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spawn {
    /// Tables des générateurs: l'intérieur du château selon le niveau, puis les régions extérieures.
    pub castle_level_1: Vec<SpawnWeight>,
    pub castle_level_2: Vec<SpawnWeight>,
    pub castle_level_3_and_more: Vec<SpawnWeight>,
    pub forest: Vec<SpawnWeight>,
    pub marsh: Vec<SpawnWeight>,
    pub approach: Vec<SpawnWeight>,
    /// La faune du royaume des elfes.
    pub elf_fauna: Vec<SpawnWeight>,
    /// Monstres posés d'avance dans chaque salle de combat: `base + niveau / every_levels`, au plus `max`.
    pub monsters_per_room_base: u32,
    pub monsters_per_room_every_levels: u32,
    pub monsters_per_room_max: u32,
    /// Potions posées dans un niveau: `base + min(niveau, levels_cap) / every_levels`.
    pub potions_per_level_base: u32,
    pub potions_levels_cap: u32,
    pub potions_every_levels: u32,
    /// Les premiers niveaux sont plus doux: facteur (vitesse et vue des monstres, rythme des générateurs) au niveau 1, et gain par niveau
    /// jusqu'à 1.
    pub easing_start: f32,
    pub easing_step: f32,
    /// Chance qu'une salle de combat ait des pièges, et nombre de pièges posés dans ce cas.
    pub trap_room_chance: f32,
    pub traps_per_room: u32,
    /// Nombre maximal de monstres vivants dans la salle.
    pub max_monsters: u32,
    pub generator_health: f32,
    /// Un générateur ne produit que si un héros est à moins de cette distance.
    pub generator_activation: f32,
    pub generator_interval: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drops {
    /// Chances (de 0 à 1) qu'un monstre laisse, en mourant, une pièce, de la nourriture ou une potion.
    pub monster_coin_chance: f32,
    pub monster_food_chance: f32,
    pub monster_potion_chance: f32,
    pub generator_coins: u32,
    pub boss_coins: u32,
    pub rescue_coins: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pickups {
    /// Distance à laquelle un héros ramasse un objet.
    pub pickup_range: f32,
    pub food_heal: f32,
    pub potion_duration: f32,
    /// Part des PV maximum rendue chaque seconde par une potion.
    pub potion_heal_per_second: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Traps {
    pub damage: f32,
    pub cooldown: f32,
    pub trigger_radius: f32,
    pub disarm_time: f32,
    /// Un héros à moins de cette distance réveille le piège; il se rendort `sleep_delay` secondes après son départ.
    pub wake_radius: f32,
    pub sleep_delay: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Abilities {
    pub bulwark_radius: f32,
    pub bulwark_duration: f32,
    /// Part des dégâts qui passe encore sous le Rempart.
    pub bulwark_damage_factor: f32,
    pub arcane_offset: f32,
    pub arcane_radius: f32,
    pub arcane_damage: f32,
    pub rain_offset: f32,
    pub rain_radius: f32,
    pub rain_damage: f32,
    pub rain_interval: f32,
    pub rain_duration: f32,
    pub turret_lifetime: f32,
    pub turret_range: f32,
    pub turret_cooldown: f32,
    pub turret_damage: f32,
    pub turret_shot_speed: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rules {
    /// Distance à laquelle un allié relève un héros à terre, et temps nécessaire.
    pub revive_range: f32,
    pub revive_time: f32,
    pub door_open_range: f32,
    /// Les corps se bloquent: marge ajoutée à la portée de frappe d'un monstre.
    pub monster_reach_slack: f32,
    /// La sortie s'active quand tous les héros debout sont dans ce rayon, pendant ce délai.
    pub exit_radius: f32,
    pub exit_delay: f32,
}

impl Balance {
    /// Lit un fichier d'équilibrage (RON).
    pub fn parse(text: &str) -> Result<Balance, ron::error::SpannedError> {
        ron::from_str(text)
    }

    /// Le fichier compilé dans le programme.
    pub fn embedded() -> Balance {
        Self::parse(EMBEDDED).unwrap_or_else(|e| panic!("balance.ron: {e}"))
    }

    /// Le texte du fichier, mis en forme comme `balance.ron`.
    pub fn to_text(&self) -> String {
        let config = ron::ser::PrettyConfig::default().indentor("  ");
        ron::ser::to_string_pretty(self, config).expect("équilibrage sérialisable")
    }
}

/// L'équilibrage en vigueur, lu une fois: `CASTLE_BALANCE_FILE` s'il est lisible, sinon le fichier du programme.
pub fn balance() -> &'static Balance {
    static BALANCE: OnceLock<Balance> = OnceLock::new();
    BALANCE.get_or_init(|| {
        std::env::var_os("CASTLE_BALANCE_FILE")
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| Balance::parse(&text).ok())
            .unwrap_or_else(Balance::embedded)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_file_parses_and_round_trips() {
        let balance = Balance::embedded();
        assert_eq!(Balance::parse(&balance.to_text()).unwrap(), balance);
    }

    #[test]
    fn every_spawn_table_has_monsters_with_a_positive_weight() {
        let spawn = &Balance::embedded().spawn;
        for table in [&spawn.castle_level_1, &spawn.castle_level_2, &spawn.castle_level_3_and_more, &spawn.forest, &spawn.marsh, &spawn.approach, &spawn.elf_fauna] {
            assert!(!table.is_empty() && table.iter().all(|w| w.weight > 0.0));
        }
    }
}
