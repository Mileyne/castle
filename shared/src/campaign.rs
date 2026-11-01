//! La campagne: vingt niveaux fixes, en cinq actes de quatre niveaux, le quatrième étant un combat de boss. Les niveaux sont des fichiers RON
//! (`shared/levels/`) compilés dans le programme, pour que le serveur et les clients aient la même carte.

use std::sync::OnceLock;

use crate::LevelDef;

/// Nombre de niveaux de la campagne, et rythme des boss (trois niveaux, puis un boss).
pub const LEVEL_COUNT: u32 = 20;
/// Niveaux par acte: le dernier de chaque acte est un combat de boss.
pub const BOSS_EVERY: u32 = 4;
/// Nombre d'actes.
pub const ACT_COUNT: u32 = LEVEL_COUNT / BOSS_EVERY;

/// L'acte d'un niveau (de 1 à `ACT_COUNT`) et son rang dans l'acte (de 1 à `BOSS_EVERY`).
pub fn act_of(level: u32) -> (u32, u32) {
    let index = level.clamp(1, LEVEL_COUNT) - 1;
    (index / BOSS_EVERY + 1, index % BOSS_EVERY + 1)
}

/// Les niveaux 4, 8, 12, 16 et 20 sont des combats de boss: le dernier de chaque acte.
pub fn is_boss_level(level: u32) -> bool {
    level >= BOSS_EVERY && level.is_multiple_of(BOSS_EVERY)
}

const FILES: [&str; LEVEL_COUNT as usize] = [
    include_str!("../levels/01.ron"),
    include_str!("../levels/02.ron"),
    include_str!("../levels/03.ron"),
    include_str!("../levels/04.ron"),
    include_str!("../levels/05.ron"),
    include_str!("../levels/06.ron"),
    include_str!("../levels/07.ron"),
    include_str!("../levels/08.ron"),
    include_str!("../levels/09.ron"),
    include_str!("../levels/10.ron"),
    include_str!("../levels/11.ron"),
    include_str!("../levels/12.ron"),
    include_str!("../levels/13.ron"),
    include_str!("../levels/14.ron"),
    include_str!("../levels/15.ron"),
    include_str!("../levels/16.ron"),
    include_str!("../levels/17.ron"),
    include_str!("../levels/18.ron"),
    include_str!("../levels/19.ron"),
    include_str!("../levels/20.ron"),
];

/// Nom du fichier d'un niveau, dans `shared/levels/`.
pub fn level_file(level: u32) -> String {
    format!("{:02}.ron", level.clamp(1, LEVEL_COUNT))
}

/// Le niveau `level` de la campagne (au-delà du dernier, on reste sur le dernier).
///
/// Pour essayer des niveaux en cours d'édition sans recompiler, `CASTLE_LEVELS_DIR=<dossier>` fait lire les fichiers de ce
/// dossier à la place de ceux du programme (le serveur lancé par le client hérite de la variable); un fichier absent ou
/// illisible retombe sur celui du programme.
pub fn level_def(level: u32) -> &'static LevelDef {
    static LEVELS: OnceLock<Vec<LevelDef>> = OnceLock::new();
    let levels = LEVELS.get_or_init(|| {
        let dir = std::env::var_os("CASTLE_LEVELS_DIR").map(std::path::PathBuf::from);
        FILES
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let number = i as u32 + 1;
                let from_dir = dir.as_ref().and_then(|dir| {
                    let text = std::fs::read_to_string(dir.join(level_file(number))).ok()?;
                    LevelDef::parse(&text).ok()
                });
                from_dir.unwrap_or_else(|| LevelDef::parse(text).unwrap_or_else(|e| panic!("niveau {number} ({}): {e}", level_file(number))))
            })
            .collect()
    });
    &levels[level.clamp(1, LEVEL_COUNT) as usize - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_boss_ends_every_act() {
        let bosses: Vec<u32> = (1..=LEVEL_COUNT).filter(|l| is_boss_level(*l)).collect();
        assert_eq!(bosses, [4, 8, 12, 16, 20]);
    }

    #[test]
    fn the_campaign_has_five_acts_of_four_levels() {
        assert_eq!(ACT_COUNT, 5);
        assert_eq!((act_of(1), act_of(4), act_of(5), act_of(20)), ((1, 1), (1, 4), (2, 1), (5, 4)));
    }

    #[test]
    fn every_campaign_level_is_valid() {
        for level in 1..=LEVEL_COUNT {
            let def = level_def(level);
            assert!(def.validate().is_empty(), "niveau {level}: {:?}", def.validate());
        }
    }
}
