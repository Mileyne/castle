//! La campagne: douze niveaux fixes, en trois actes de trois niveaux suivis d'un boss. Les niveaux sont des fichiers RON
//! (`shared/levels/`) compilés dans le programme, pour que le serveur et les clients aient la même carte.

use std::sync::OnceLock;

use crate::LevelDef;

/// Nombre de niveaux de la campagne, et rythme des boss (trois niveaux, puis un boss).
pub const LEVEL_COUNT: u32 = 12;
pub const BOSS_EVERY: u32 = 4;

/// Les niveaux 4, 8 et 12 sont des combats de boss.
pub fn is_boss_level(level: u32) -> bool {
    level >= BOSS_EVERY && level % BOSS_EVERY == 0
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
    fn a_boss_comes_after_every_three_levels() {
        let bosses: Vec<u32> = (1..=LEVEL_COUNT).filter(|l| is_boss_level(*l)).collect();
        assert_eq!(bosses, [4, 8, 12]);
    }

    #[test]
    fn every_campaign_level_is_valid() {
        for level in 1..=LEVEL_COUNT {
            let def = level_def(level);
            assert!(def.validate().is_empty(), "niveau {level}: {:?}", def.validate());
        }
    }
}
