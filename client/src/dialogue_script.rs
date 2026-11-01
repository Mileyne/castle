//! Scènes de dialogue: les données (`assets/dialogue/castle.ron`) et le petit interpréteur qui les déroule.
//!
//! Une scène est une suite d'étapes: une réplique, ou un choix voté par l'équipe dont chaque option mène à ses répliques.
//! L'interpréteur ne connaît ni Bevy ni Fluent: il rend l'étape à afficher, `dialogue.rs` s'occupe du reste.

use std::collections::{HashMap, VecDeque};

use bevy::prelude::Resource;
use serde::Deserialize;

const SOURCE: &str = include_str!("../assets/dialogue/castle.ron");

#[derive(Deserialize, Clone, Debug)]
pub enum Step {
    /// (orateur, identifiant): le texte est la clé Fluent `dlg-<identifiant>`.
    Line(String, String),
    Choice(Vec<Choice>),
}

#[derive(Deserialize, Clone, Debug)]
pub struct Choice {
    pub id: String,
    /// Répliques jouées si l'option est retenue.
    pub then: Vec<Step>,
}

#[derive(Deserialize)]
struct Scene {
    id: String,
    steps: Vec<Step>,
}

/// Toutes les scènes du jeu, par nom.
#[derive(Resource)]
pub struct Script(HashMap<String, Vec<Step>>);

impl Script {
    pub fn parse(source: &str) -> Result<Self, ron::error::SpannedError> {
        let scenes: Vec<Scene> = ron::from_str(source)?;
        Ok(Self(scenes.into_iter().map(|scene| (scene.id, scene.steps)).collect()))
    }

    #[cfg(test)]
    pub fn has_scene(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    /// Tous les identifiants de répliques et d'options.
    #[cfg(test)]
    pub fn ids(&self) -> Vec<String> {
        fn collect(steps: &[Step], ids: &mut Vec<String>) {
            for step in steps {
                match step {
                    Step::Line(_, id) => ids.push(id.clone()),
                    Step::Choice(choices) => {
                        for choice in choices {
                            ids.push(choice.id.clone());
                            collect(&choice.then, ids);
                        }
                    }
                }
            }
        }
        let mut ids = Vec::new();
        self.0.values().for_each(|steps| collect(steps, &mut ids));
        ids
    }

    /// Le script embarqué dans le jeu.
    pub fn builtin() -> Self {
        Self::parse(SOURCE).expect("assets/dialogue/castle.ron is invalid")
    }
}

/// Ce que le joueur doit voir.
#[derive(Debug, PartialEq)]
pub enum Shown {
    Line { speaker: String, id: String },
    /// Les identifiants des options, dans l'ordre; la réplique qui les précède reste affichée.
    Options(Vec<String>),
    /// Plus rien à jouer.
    Finished,
}

/// Où en est la scène en cours.
#[derive(Resource, Default)]
pub struct Runner {
    queue: VecDeque<Step>,
    /// Les options proposées, avec leurs répliques, en attente du vote.
    choices: Vec<Choice>,
    running: bool,
}

impl Runner {
    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn is_waiting_for_option(&self) -> bool {
        !self.choices.is_empty()
    }

    /// Lance la scène `name`; `None` si elle n'existe pas. La première étape est rendue tout de suite.
    pub fn start(&mut self, script: &Script, name: &str) -> Option<Shown> {
        let steps = script.0.get(name)?;
        self.queue = steps.iter().cloned().collect();
        self.choices.clear();
        self.running = true;
        Some(self.advance())
    }

    /// Passe à l'étape suivante. Une réplique suivie d'un choix rend le choix à son tour, sans attendre.
    pub fn advance(&mut self) -> Shown {
        match self.queue.pop_front() {
            Some(Step::Line(speaker, id)) => Shown::Line { speaker, id },
            Some(Step::Choice(choices)) => {
                let ids = choices.iter().map(|choice| choice.id.clone()).collect();
                self.choices = choices;
                Shown::Options(ids)
            }
            None => {
                self.running = false;
                Shown::Finished
            }
        }
    }

    /// Retient l'option `index`; ses répliques passent avant la suite de la scène. À appeler avant `advance`.
    pub fn select(&mut self, index: usize) {
        if index < self.choices.len() {
            let choice = self.choices.swap_remove(index);
            for step in choice.then.into_iter().rev() {
                self.queue.push_front(step);
            }
        }
        self.choices.clear();
    }

    /// Abandonne la scène.
    pub fn stop(&mut self) {
        *self = Self::default();
    }

    /// La prochaine étape est un choix: la réplique en cours doit être affichée avec lui.
    pub fn next_is_choice(&self) -> bool {
        matches!(self.queue.front(), Some(Step::Choice(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(speaker: &str, id: &str) -> Shown {
        Shown::Line { speaker: speaker.into(), id: id.into() }
    }

    #[test]
    fn the_embedded_script_parses_and_has_every_scene() {
        let script = Script::builtin();
        assert_eq!(script.0.len(), 30);
        for scene in [
            "prologue",
            "intro",
            "alcove",
            "boss_muldant",
            // Le récit en cinq actes.
            "samelia_taken",
            "elves_dawn",
            "boss_gardener",
            "samelia_rescued",
            "gate_farewell",
            "heart_reveal",
        ] {
            assert!(script.0.contains_key(scene), "scène {scene} manquante");
        }
    }

    #[test]
    fn a_scene_plays_line_by_line_then_finishes() {
        let script = Script::builtin();
        let mut runner = Runner::default();
        assert_eq!(runner.start(&script, "puzzle_rune"), Some(line("sorcerer", "puzzle_rune_01")));
        assert!(runner.is_running());
        assert_eq!(runner.advance(), line("sorcerer", "puzzle_rune_02"));
        assert_eq!(runner.advance(), line("knight", "puzzle_rune_03"));
        assert_eq!(runner.advance(), Shown::Finished);
        assert!(!runner.is_running());
        assert!(runner.start(&script, "inconnue").is_none());
    }

    #[test]
    fn a_choice_follows_its_line_and_the_selected_branch_plays_next() {
        let script = Script::builtin();
        let mut runner = Runner::default();
        assert_eq!(runner.start(&script, "alcove"), Some(line("forgebraise", "alcove_01")));
        assert!(runner.next_is_choice());
        assert_eq!(runner.advance(), Shown::Options(vec!["alcove_opt_key".into(), "alcove_opt_boom".into()]));
        assert!(runner.is_waiting_for_option());
        runner.select(1);
        assert!(!runner.is_waiting_for_option());
        assert_eq!(runner.advance(), line("forgebraise", "alcove_03"));
        assert_eq!(runner.advance(), Shown::Finished);
    }

    #[test]
    fn stopping_forgets_the_scene() {
        let script = Script::builtin();
        let mut runner = Runner::default();
        runner.start(&script, "intro");
        runner.stop();
        assert!(!runner.is_running());
        assert_eq!(runner.advance(), Shown::Finished);
    }
}
