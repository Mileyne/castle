//! Objectif du niveau: affichage en haut de l'écran, et sprites des reliques et des prisonniers.

use bevy::{color::palettes::css, prelude::*};
use castle_shared::{Captive, Position, Quest, Relic};

use crate::art::{Art, PROP_ANCHOR, Prop, SPRITE_SIZE, object, object_index};
use crate::i18n::Tr;
use crate::ui::ui_font;

pub struct QuestPlugin;

impl Plugin for QuestPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_quest_hud)
            .add_systems(Update, (dress_quest_entities, update_quest_hud));
    }
}

#[derive(Component)]
struct QuestHud;

#[derive(Component)]
struct QuestName;

#[derive(Component)]
struct QuestCount;

fn spawn_quest_hud(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            QuestHud,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(12.0),
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|hud| {
            hud.spawn((Text::default(), Tr::new("quest-hunt"), QuestName, ui_font(&assets, 14.0), TextColor(css::GOLD.into())));
            hud.spawn((Text::default(), QuestCount, ui_font(&assets, 14.0), TextColor(css::WHITE.into())));
        });
}

/// L'objectif à montrer: le premier à accomplir (la chasse avant les autres), ou le dernier une fois tous remplis.
fn current_quest<'a>(quests: impl Iterator<Item = &'a Quest>) -> Option<&'a Quest> {
    let mut all: Vec<&Quest> = quests.collect();
    all.sort_by_key(|quest| quest.kind != castle_shared::QuestKind::Hunt);
    all.iter().copied().find(|quest| !quest.is_done()).or_else(|| all.last().copied())
}

fn update_quest_hud(
    quests: Query<&Quest>,
    mut hud: Single<&mut Visibility, With<QuestHud>>,
    mut name: Single<(&mut Tr, &mut TextColor), (With<QuestName>, Without<QuestCount>)>,
    mut count: Single<&mut Text, With<QuestCount>>,
) {
    let Some(quest) = current_quest(quests.iter()) else {
        **hud = Visibility::Hidden;
        return;
    };
    // « Objectif accompli » seulement quand tous les objectifs le sont.
    let all_done = quests.iter().all(|q| q.is_done());
    **hud = Visibility::Inherited;
    let (tr, color) = &mut *name;
    let (key, shown, tint) = if all_done {
        ("quest-done", String::new(), css::LIME)
    } else {
        (quest.kind.name_key(), format!("{}/{}", quest.progress, quest.goal), css::GOLD)
    };
    if tr.key != key {
        tr.key = key;
    }
    if color.0 != Color::from(tint) {
        color.0 = tint.into();
    }
    if count.0 != shown {
        count.0 = shown;
    }
}

/// Donne un sprite aux reliques (provisoire) et aux prisonniers (un prisonnier dans une cage).
fn dress_quest_entities(
    mut commands: Commands,
    art: Res<Art>,
    relics: Query<(Entity, &Position), Added<Relic>>,
    captives: Query<(Entity, &Position), Added<Captive>>,
) {
    let size = Vec2::splat(SPRITE_SIZE);
    for (entity, pos) in &relics {
        let mut sprite = art.objects.sprite_sized(object_index(object::KEY, 0), size);
        sprite.color = Color::srgb(0.55, 1.0, 1.0);
        commands.entity(entity).insert((sprite, Prop(object::KEY), PROP_ANCHOR, Transform::from_translation(pos.0.extend(0.6))));
    }
    for (entity, pos) in &captives {
        let sprite = art.objects.sprite_sized(object_index(object::CAPTIVE, 0), size);
        commands.entity(entity).insert((sprite, Prop(object::CAPTIVE), PROP_ANCHOR, Transform::from_translation(pos.0.extend(0.6))));
    }
}
