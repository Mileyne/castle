//! Interface: statut de connexion, choix de classe, aide aux touches.

use bevy::{color::palettes::css, prelude::*};
use bevy_replicon::prelude::*;
use castle_shared::{Downed, Gold, HeroClass, Health, Keys, Mana, PartyId, Player};

use crate::{LocalParty, LocalPlayer};
use crate::i18n::Tr;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui)
            .add_systems(
                Update,
                (
                    update_status,
                    update_party_panel,
                    update_portraits,
                    update_loot_counters,
                    update_special_label,
                    update_region_label,
                ),
            );
    }
}

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct SpecialLabel;

#[derive(Component)]
struct RegionLabel;

#[derive(Component)]
struct GoldCount;

#[derive(Component)]
struct KeyCount;

#[derive(Component)]
struct ClassEntry(HeroClass);

#[derive(Component)]
struct HpFill(HeroClass);

/// Vignette d'un héros dans le panneau d'équipe.
#[derive(Component)]
struct PortraitSlot(HeroClass);

#[derive(Component)]
struct ManaFill(HeroClass);

/// Police pixel-art (Press Start 2P, licence OFL) qui couvre les accents latins.
pub fn ui_font(assets: &AssetServer, size: f32) -> TextFont {
    TextFont {
        font: assets.load("fonts/PressStart2P-Regular.ttf").into(),
        font_size: size.into(),
        ..default()
    }
}

fn spawn_ui(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Text::default(),
        Tr::new("status-connecting"),
        StatusText,
        ui_font(&assets, 16.0),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
    ));

    commands.spawn((
        Text::default(),
        Tr::new("region-crypts"),
        RegionLabel,
        Visibility::Hidden,
        ui_font(&assets, 12.0),
        TextColor(css::SILVER.into()),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(32.0),
            left: Val::Px(8.0),
            ..default()
        },
    ));

    // Or et clés du joueur, en haut à droite: le libellé est traduit, le nombre ne l'est pas.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(8.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|parent| {
            for (key, is_gold) in [("hud-gold", true), ("hud-keys", false)] {
                parent
                    .spawn(Node {
                        column_gap: Val::Px(12.0),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((Text::default(), Tr::new(key), ui_font(&assets, 16.0)));
                        let count = (Text::new("0"), ui_font(&assets, 16.0), TextColor(css::GOLD.into()));
                        if is_gold {
                            row.spawn((count, GoldCount));
                        } else {
                            row.spawn((count, KeyCount));
                        }
                    });
            }
        });

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                Text::default(),
                Tr::with_prefix("special-knight", "%special%  "),
                SpecialLabel,
                ui_font(&assets, 16.0),
                TextColor(css::DEEP_SKY_BLUE.into()),
            ));
            parent.spawn((
                Text::default(),
                Tr::new("hint-controls"),
                ui_font(&assets, 12.0),
                TextColor(css::GRAY.into()),
            ));
        });

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            top: Val::Px(64.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(14.0),
            ..default()
        })
        .with_children(|row| {
            for (i, (class, key)) in [
                (HeroClass::Knight, "class-knight"),
                (HeroClass::Sorcerer, "class-sorcerer"),
                (HeroClass::Ranger, "class-ranger"),
                (HeroClass::Forgebraise, "class-forgebraise"),
            ]
            .into_iter()
            .enumerate()
            {
                let prefix = ["1 ", "2 ", "3 ", "4 "][i];
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|slot| {
                    slot.spawn((
                        PortraitSlot(class),
                        ImageNode::default(),
                        Node { width: Val::Px(80.0), height: Val::Px(80.0), ..default() },
                    ));
                    slot.spawn((
                        Text::default(),
                        Tr::with_prefix(key, prefix),
                        ClassEntry(class),
                        ui_font(&assets, 12.0),
                    ));
                    // Barre de vie du héros, remplie selon ses points de vie.
                    slot.spawn((
                        Node {
                            width: Val::Px(80.0),
                            height: Val::Px(6.0),
                            ..default()
                        },
                        BackgroundColor(css::DARK_RED.into()),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            HpFill(class),
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(css::LIME.into()),
                        ));
                    });
                    // Barre de mana, plus fine, sous la barre de vie.
                    slot.spawn((
                        Node {
                            width: Val::Px(80.0),
                            height: Val::Px(4.0),
                            ..default()
                        },
                        BackgroundColor(css::MIDNIGHT_BLUE.into()),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            ManaFill(class),
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(css::DEEP_SKY_BLUE.into()),
                        ));
                    });
                });
            }
        });
}

fn update_status(state: Res<State<ClientState>>, mut status: Single<&mut Tr, With<StatusText>>) {
    status.key = match **state {
        ClientState::Disconnected => "status-disconnected",
        ClientState::Connecting => "status-connecting",
        ClientState::Connected => "status-connected",
    };
}

/// Met les vignettes du thème courant dans le panneau d'équipe.
fn update_portraits(art: Res<crate::art::Art>, mut slots: Query<(&PortraitSlot, &mut ImageNode)>) {
    if !art.is_changed() {
        return;
    }
    for (slot, mut image) in &mut slots {
        image.image = art.portraits.image.clone();
        image.texture_atlas = Some(TextureAtlas { layout: art.portraits.layout.clone(), index: crate::art::class_row(slot.0) });
    }
}

/// Panneau d'équipe: le héros dirigé en doré, un héros à terre en rouge sombre, et les points de vie de chacun.
fn update_party_panel(
    party: Option<Res<LocalParty>>,
    active: Option<Single<&HeroClass, With<LocalPlayer>>>,
    heroes: Query<(&HeroClass, &PartyId, &Health, &Mana, Has<Downed>), With<Player>>,
    mut entries: Query<(&ClassEntry, &mut TextColor)>,
    mut fills: Query<(&HpFill, &mut Node), Without<ManaFill>>,
    mut mana_fills: Query<(&ManaFill, &mut Node), Without<HpFill>>,
) {
    let Some(party) = party else {
        return;
    };
    let active = active.map(|h| **h);
    let mine = |class: HeroClass| {
        heroes
            .iter()
            .find(|(c, id, ..)| **c == class && id.0 == party.0)
    };
    for (entry, mut color) in &mut entries {
        let target: Color = match mine(entry.0) {
            Some((_, _, _, _, true)) => css::DARK_RED.into(),
            _ if Some(entry.0) == active => css::GOLD.into(),
            _ => css::GRAY.into(),
        };
        if color.0 != target {
            color.0 = target;
        }
    }
    for (fill, mut node) in &mut fills {
        let ratio = mine(fill.0).map_or(0.0, |(_, _, health, ..)| (health.current / health.max).clamp(0.0, 1.0));
        let width = Val::Percent(ratio * 100.0);
        if node.width != width {
            node.width = width;
        }
    }
    for (fill, mut node) in &mut mana_fills {
        let ratio = mine(fill.0).map_or(0.0, |(_, _, _, mana, _)| (mana.current / mana.max).clamp(0.0, 1.0));
        let width = Val::Percent(ratio * 100.0);
        if node.width != width {
            node.width = width;
        }
    }
}

/// Affiche le nom du pouvoir spécial de la classe du joueur.
fn update_special_label(
    hero: Option<Single<&HeroClass, With<LocalPlayer>>>,
    mut label: Single<&mut Tr, With<SpecialLabel>>,
) {
    if let Some(class) = hero {
        let key = class.stats().special_key;
        if label.key != key {
            label.key = key;
        }
    }
}

/// Or et clés sont mis en commun: on affiche le total de l'équipe.
fn update_loot_counters(
    party: Option<Res<LocalParty>>,
    heroes: Query<(&PartyId, &Gold, &Keys), With<Player>>,
    mut gold: Single<&mut Text, (With<GoldCount>, Without<KeyCount>)>,
    mut keys: Single<&mut Text, (With<KeyCount>, Without<GoldCount>)>,
) {
    let Some(party) = party else {
        return;
    };
    let (g, k) = heroes
        .iter()
        .filter(|(id, ..)| id.0 == party.0)
        .fold((0, 0), |(g, k), (_, gold, keys)| (g + gold.0, k + keys.0));
    let (g, k) = (g.to_string(), k.to_string());
    if gold.0 != g {
        gold.0 = g;
    }
    if keys.0 != k {
        keys.0 = k;
    }
}

/// Affiche le nom de la région du château en cours d'exploration.
fn update_region_label(
    expedition: Option<Single<&castle_shared::Expedition>>,
    mut label: Single<(&mut Tr, &mut Visibility), With<RegionLabel>>,
) {
    let Some(expedition) = expedition else {
        return;
    };
    let (tr, visibility) = &mut *label;
    let key = expedition.region.name_key();
    if tr.key != key {
        tr.key = key;
    }
    if **visibility != Visibility::Inherited {
        **visibility = Visibility::Inherited;
    }
}
