//! Localisation: charge les fichiers Fluent de `assets/locales/<langue>/`.
//!
//! Le code ne manipule que des clés. `tr()` donne le texte dans la langue courante.

use bevy::{asset::LoadState, asset::LoadedFolder, prelude::*};
use bevy_fluent::prelude::*;
use fluent_content::Content;
use unic_langid::{LanguageIdentifier, langid};

/// Langues livrées avec le jeu, dans l'ordre du cycle F1.
pub fn supported() -> [LanguageIdentifier; 2] {
    [langid!("fr-FR"), langid!("en-US")]
}

pub struct I18nPlugin;

impl Plugin for I18nPlugin {
    fn build(&self, app: &mut App) {
        let system = sys_locale::get_locale()
            .and_then(|l| l.parse().ok())
            .unwrap_or_else(|| langid!("en-US"));
        app.add_plugins(FluentPlugin)
            .insert_resource(Locale::new(system).with_default(langid!("en-US")))
            .add_systems(Startup, load_locales)
            .add_systems(Update, (build_localization, cycle_language, refresh_texts).chain());
    }
}

/// Texte localisé: `prefix` (non traduit) suivi de la traduction de `key`.
#[derive(Component)]
pub struct Tr {
    pub key: &'static str,
    pub prefix: &'static str,
}

impl Tr {
    pub fn new(key: &'static str) -> Self {
        Self { key, prefix: "" }
    }

    pub fn with_prefix(key: &'static str, prefix: &'static str) -> Self {
        Self { key, prefix }
    }
}

#[derive(Resource)]
struct LocaleFolder(Handle<LoadedFolder>);

fn load_locales(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(LocaleFolder(assets.load_folder("locales")));
}

fn build_localization(
    mut commands: Commands,
    builder: LocalizationBuilder,
    assets: Res<AssetServer>,
    folder: Res<LocaleFolder>,
    locale: Res<Locale>,
    mut built: Local<bool>,
) {
    if !matches!(assets.get_load_state(&folder.0), Some(LoadState::Loaded)) {
        return;
    }
    if *built && !locale.is_changed() {
        return;
    }
    *built = true;
    info!("language: {}", locale.requested);
    commands.insert_resource(builder.build(&folder.0));
}

/// F1 passe à la langue suivante.
fn cycle_language(keys: Res<ButtonInput<KeyCode>>, mut locale: ResMut<Locale>) {
    if !keys.just_pressed(KeyCode::F1) {
        return;
    }
    let langs = supported();
    let current = langs
        .iter()
        .position(|l| l.language == locale.requested.language)
        .unwrap_or(0);
    locale.requested = langs[(current + 1) % langs.len()].clone();
}

fn refresh_texts(localization: Option<Res<Localization>>, mut texts: Query<(&Tr, &mut Text)>) {
    let Some(localization) = localization else {
        return;
    };
    for (tr, mut text) in &mut texts {
        // Une clé absente reste visible à l'écran: plus simple à repérer qu'un texte vide.
        let translated = localization.content(tr.key).unwrap_or_else(|| tr.key.to_string());
        let full = format!("{}{translated}", tr.prefix);
        if text.0 != full {
            text.0 = full;
        }
    }
}
