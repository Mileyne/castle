//! Ce que l'équipe a trouvé pour l'énigme de la porte royale, affiché sous l'objectif du niveau. Chaque pièce porte la couleur
//! de la classe qui doit s'en servir: violet (Ensorceleur), orange (Forgebraise), vert (Rôdeur).

use bevy::{color::palettes::css, prelude::*};
use castle_shared::{INGREDIENTS_NEEDED, InteractKind, Interactable, PuzzleKit};

use crate::i18n::Tr;
use crate::ui::ui_font;

pub struct PuzzlePlugin;

impl Plugin for PuzzlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_puzzle_hud).add_systems(Update, update_puzzle_hud);
    }
}

#[derive(Component)]
struct PuzzleHud;

/// Une pièce: son texte de valeur (le nom est un `Tr` à côté), et l'obstacle qu'elle sert.
#[derive(Component, Clone, Copy)]
struct PieceValue(InteractKind);

const PIECES: [(InteractKind, &str, Srgba); 3] = [
    (InteractKind::Rune, "puzzle-ingredients", css::MEDIUM_ORCHID),
    (InteractKind::Mechanism, "puzzle-gear", css::ORANGE),
    (InteractKind::Target, "puzzle-target", css::LIMEGREEN),
];

fn spawn_puzzle_hud(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            PuzzleHud,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(30.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(22.0),
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|hud| {
            for (kind, key, color) in PIECES {
                hud.spawn(Node { column_gap: Val::Px(6.0), ..default() }).with_children(|piece| {
                    piece.spawn((Text::default(), Tr::new(key), ui_font(&assets, 10.0), TextColor(color.into())));
                    piece.spawn((Text::default(), PieceValue(kind), ui_font(&assets, 10.0), TextColor(Color::WHITE)));
                });
            }
        });
}

/// Le texte d'une pièce: ce qu'on a trouvé sur ce qu'il faut, « ok » une fois l'obstacle vaincu, ou « visez ! » pour la cible
/// garnie qui n'attend plus que la flèche.
fn piece_text(kind: InteractKind, kit: &PuzzleKit, panels: &[Interactable]) -> String {
    let panel = panels.iter().find(|p| p.kind == kind);
    if panel.is_some_and(|p| p.done) {
        return "OK".into();
    }
    match kind {
        InteractKind::Rune => format!("{}/{}", kit.ingredients.min(INGREDIENTS_NEEDED), INGREDIENTS_NEEDED),
        InteractKind::Mechanism => format!("{}/1", kit.gear.min(1)),
        InteractKind::Target if panel.is_some_and(|p| p.mounted) => "!".into(),
        InteractKind::Target => format!("{}/1", kit.target.min(1)),
        InteractKind::Push => String::new(),
    }
}

fn update_puzzle_hud(
    kit: Option<Single<&PuzzleKit>>,
    panels: Query<&Interactable>,
    mut hud: Single<&mut Visibility, With<PuzzleHud>>,
    mut values: Query<(&PieceValue, &mut Text)>,
) {
    // L'énigme n'a plus d'objet une fois la porte royale ouverte.
    let door_open = !panels.iter().any(|p| p.kind == InteractKind::Push);
    let Some(kit) = kit.filter(|_| !door_open) else {
        **hud = Visibility::Hidden;
        return;
    };
    **hud = Visibility::Inherited;
    let panels: Vec<Interactable> = panels.iter().copied().collect();
    for (value, mut text) in &mut values {
        let shown = piece_text(value.0, &kit, &panels);
        if text.0 != shown {
            text.0 = shown;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_piece_shows_what_is_missing_then_ok() {
        let kit = PuzzleKit { ingredients: 2, gear: 0, target: 1 };
        let mut panels = [Interactable::new(InteractKind::Rune), Interactable::new(InteractKind::Mechanism), Interactable::new(InteractKind::Target)];
        assert_eq!(piece_text(InteractKind::Rune, &kit, &panels), "2/3");
        assert_eq!(piece_text(InteractKind::Mechanism, &kit, &panels), "0/1");
        assert_eq!(piece_text(InteractKind::Target, &kit, &panels), "1/1");
        // Une cible garnie n'attend plus que la flèche.
        panels[2].mounted = true;
        assert_eq!(piece_text(InteractKind::Target, &PuzzleKit::default(), &panels), "!");
        panels[2].done = true;
        panels[0].done = true;
        assert_eq!(piece_text(InteractKind::Target, &kit, &panels), "OK");
        assert_eq!(piece_text(InteractKind::Rune, &kit, &panels), "OK");
        // Jamais plus que nécessaire (une réserve qui déborde ne fausse pas l'affichage).
        let full = PuzzleKit { ingredients: 7, gear: 3, target: 2 };
        let fresh = [Interactable::new(InteractKind::Rune), Interactable::new(InteractKind::Mechanism)];
        assert_eq!(piece_text(InteractKind::Rune, &full, &fresh), "3/3");
        assert_eq!(piece_text(InteractKind::Mechanism, &full, &fresh), "1/1");
    }
}
