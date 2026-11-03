//! Construction à l'avance des poses des modèles 3D d'un niveau, derrière un écran « Veuillez patienter » avec son avancement.
//!
//! Chaque modèle (héros, monstre, objet, motif, sortie) n'a que quelques actions: repos, marche, coup. Toutes leurs images sont
//! construites au lancement du niveau (voir `merged::Baking`), par petits morceaux pour que le pourcentage avance, et gardées en
//! mémoire: en jeu, aucun modèle ne se fige le temps de se construire. Seuls les modèles du niveau sont construits, ce qui garde la
//! mémoire graphique raisonnable sur les petits appareils.

use std::time::Duration;

use bevy::{color::palettes::css, prelude::*};
use castle_shared::{Expedition, HeroClass, MonsterKind, Thing, level_def};

use crate::i18n::Tr;
use crate::live3d::{Caches, exit_models, hero_model, landmark_model, monster_model, prop_models};
use crate::merged::{Baking, ModelSource};
use crate::ui::ui_font;

/// Temps de construction par image: assez court pour que le jeu et l'écran restent fluides.
const BUDGET: Duration = Duration::from_millis(10);

/// Les renforts qu'appelle un boss (voir `server::boss`): leurs modèles doivent être prêts avant le combat.
fn reinforcement(boss: MonsterKind) -> Option<MonsterKind> {
    match boss {
        MonsterKind::Warg => Some(MonsterKind::Wolf),
        MonsterKind::Troll | MonsterKind::Gardener => Some(MonsterKind::Goblin),
        MonsterKind::Dragon => Some(MonsterKind::Skeleton),
        MonsterKind::Muldant => Some(MonsterKind::Skeleton),
        _ => None,
    }
}

/// Tous les modèles dont le niveau a besoin.
pub fn level_models(level: u32) -> Vec<ModelSource> {
    let def = level_def(level);
    let mut models: Vec<ModelSource> = HeroClass::ALL.into_iter().map(hero_model).collect();
    models.extend(prop_models());
    // Sans sortie posée à la main, le niveau a un escalier.
    models.extend(exit_models(castle_shared::ExitKind::Stairs));
    for thing in &def.things {
        match thing {
            Thing::Monster { kind, .. } => {
                models.push(monster_model(*kind));
                models.extend(reinforcement(*kind).map(monster_model));
            }
            // Un générateur fait apparaître des monstres de la région: les plus courants.
            Thing::Generator { .. } => models.extend([MonsterKind::Skeleton, MonsterKind::Goblin].map(monster_model)),
            Thing::Landmark { kind, .. } => models.extend(landmark_model(*kind)),
            Thing::Exit { kind, .. } | Thing::Barrier { kind, .. } => models.extend(exit_models(*kind)),
            _ => {}
        }
    }
    models
}

#[derive(Resource, Default)]
struct Prebake {
    baking: Option<Baking>,
    screen: Option<Entity>,
}

#[derive(Component)]
struct PrebakeScreen;

#[derive(Component)]
struct PrebakePercent;

pub struct PrebakePlugin;

impl Plugin for PrebakePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Prebake>().add_systems(Update, (plan_new_level, run_prebake).chain());
    }
}

/// Un nouveau niveau: on oublie les poses du précédent et on prépare celles de ce niveau.
fn plan_new_level(fresh: Query<&Expedition, Added<Expedition>>, mut caches: ResMut<Caches>, mut prebake: ResMut<Prebake>) {
    let Some(expedition) = fresh.iter().last() else {
        return;
    };
    caches.poses.clear();
    prebake.baking = Some(Baking::plan(level_models(expedition.level), &caches.poses));
}

fn run_prebake(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut prebake: ResMut<Prebake>,
    mut caches: ResMut<Caches>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut percent: Query<&mut Text, With<PrebakePercent>>,
) {
    if prebake.baking.is_none() {
        return;
    }
    // L'écran d'attente apparaît avant la construction: c'est lui qu'on voit pendant que le travail se fait.
    if prebake.screen.is_none() {
        prebake.screen = Some(spawn_screen(&mut commands, &assets));
        return;
    }
    let baking = prebake.baking.as_mut().expect("vérifié plus haut");
    baking.run(BUDGET, &mut caches.poses, &mut meshes);
    let shown = format!("{} %", baking.percent());
    for mut text in &mut percent {
        if text.0 != shown {
            text.0 = shown.clone();
        }
    }
    if baking.is_finished() {
        info!("poses built: {} ({} vertices, {:.1} MB)", caches.poses.len(), caches.poses.vertices, caches.poses.bytes() as f64 / 1e6);
        prebake.baking = None;
        if let Some(screen) = prebake.screen.take() {
            commands.entity(screen).despawn();
        }
    }
}

fn spawn_screen(commands: &mut Commands, assets: &AssetServer) -> Entity {
    commands
        .spawn((
            PrebakeScreen,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(110),
        ))
        .with_children(|screen| {
            screen.spawn((Text::default(), Tr::new("loading-wait"), ui_font(assets, 24.0), TextColor(css::WHITE.into())));
            screen.spawn((Text::new("0 %"), PrebakePercent, ui_font(assets, 24.0), TextColor(css::GOLD.into())));
        })
        .id()
}
