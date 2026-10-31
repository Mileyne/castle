//! Les sprites du jeu, chargés depuis `client/assets/art/medieval/`, pour que la carte des niveaux montre ce que le joueur verra.

use std::path::Path;

use castle_shared::tileart::{TILE_CELL, TILE_COLUMNS, TILE_ROWS};
use castle_shared::{ExitKind, MonsterKind, PickupKind, Region, Thing};
use eframe::egui::{self, Color32, Rect, TextureHandle, TextureOptions, pos2};
use image::RgbaImage;

/// Case d'une planche d'objets et de monstres (96 px), et d'une planche de sorties (192 px).
const CELL: u32 = 288;
const EXIT_CELL: u32 = 576;
/// Colonnes des planches de personnages (une direction fait trois lignes de 17 images).
const SHEET_COLUMNS: u32 = 17;

// Taille des cases rognées de chaque planche, écrite par `tools/sprite_forge` pour le jeu.
#[allow(dead_code)]
#[path = "../../../client/src/sprite_meta.rs"]
mod sprite_meta;
const FRAMES: u32 = 51;
const EXIT_FRAMES: u32 = 10;

/// Un sprite à poser sur la carte: texture, partie utilisée, taille et décalage vers le haut, en tuiles.
pub struct Sprite<'a> {
    pub texture: &'a TextureHandle,
    pub uv: Rect,
    pub size: f32,
    /// Hauteur du centre du sprite au-dessus de la position de l'élément, en tuiles.
    pub lift: f32,
    pub tint: Color32,
}

/// Fichiers des planches d'objets, dans l'ordre des lignes d'objets du jeu.
const OBJECT_FILES: [&str; 28] = [
    "generator", "turret", "coin", "food", "key", "lever", "rune", "mechanism", "traparmed", "trapoff", "doorlocked", "doorroyal",
    "doorroyalemblem", "stairs", "potion", "wallrune", "wallgearsjammed", "wallgearsfixed", "walltargetempty", "walltargetmounted",
    "walltargethit", "portcullis", "ingredient", "gearpart", "targetdisc", "captivecage", "samelia", "sameliacaged",
];

pub struct LevelArt {
    tiles: TextureHandle,
    /// La première image de chaque objet (dans l'ordre des lignes `object::*` du client).
    objects: Vec<TextureHandle>,
    exits: TextureHandle,
    /// Les grands motifs (une image chacun).
    landmarks: Vec<(castle_shared::LandmarkKind, TextureHandle)>,
    /// La planche du décor au sol (sortes en colonnes, variantes en lignes, cases de deux tuiles).
    decor: TextureHandle,
    /// Un monstre de face (repos), dans l'ordre de `MonsterKind`.
    monsters: Vec<(MonsterKind, TextureHandle)>,
}

fn load(path: &Path) -> Option<RgbaImage> {
    image::open(path).ok().map(|img| img.to_rgba8())
}

/// Lit une texture compressée (`.basis`) en RGBA.
fn load_basis(path: &Path) -> Option<RgbaImage> {
    use basis_universal::{DecodeFlags, TranscodeParameters, Transcoder, TranscoderTextureFormat};
    let data = std::fs::read(path).ok()?;
    let mut transcoder = Transcoder::new();
    let info = transcoder.image_info(&data, 0)?;
    transcoder.prepare_transcoding(&data).ok()?;
    let pixels = transcoder
        .transcode_image_level(
            &data,
            TranscoderTextureFormat::RGBA32,
            TranscodeParameters { image_index: 0, level_index: 0, decode_flags: Some(DecodeFlags::HIGH_QUALITY), ..Default::default() },
        )
        .ok()?;
    RgbaImage::from_raw(info.m_orig_width, info.m_orig_height, pixels)
}

/// Une case d'une planche rognée, remise à sa place dans une case entière transparente.
fn full_cell(dir: &str, name: &str, sheet: &RgbaImage, index: u32, columns: u32) -> Option<RgbaImage> {
    let &(_, _, x, y, w, h) = sprite_meta::SHEETS.iter().find(|(d, n, ..)| *d == dir && *n == name)?;
    let (sx, sy) = (index % columns * w, index / columns * h);
    let piece = image::imageops::crop_imm(sheet, sx, sy, w, h).to_image();
    let mut cell = RgbaImage::new(CELL, CELL);
    image::imageops::replace(&mut cell, &piece, x as i64, y as i64);
    Some(cell)
}

fn texture(
    ctx: &egui::Context,
    name: &str,
    img: &RgbaImage,
    options: TextureOptions,
) -> TextureHandle {
    let color = egui::ColorImage::from_rgba_unmultiplied(
        [img.width() as usize, img.height() as usize],
        img.as_raw(),
    );
    ctx.load_texture(name, color, options)
}

fn cell_uv(column: u32, row: u32, cell: u32, columns: u32, rows: u32) -> Rect {
    let (w, h) = (columns * cell, rows * cell);
    Rect::from_min_max(
        pos2(
            (column * cell) as f32 / w as f32,
            (row * cell) as f32 / h as f32,
        ),
        pos2(
            ((column + 1) * cell) as f32 / w as f32,
            ((row + 1) * cell) as f32 / h as f32,
        ),
    )
}

impl LevelArt {
    pub fn load(ctx: &egui::Context, repo: &Path) -> Option<Self> {
        let dir = repo.join("client/assets/art/medieval");
        let tiles = load(&dir.join("tiles.png"))?;
        let decor = load(&dir.join("decor.png"))?;
        let landmarks = castle_shared::LandmarkKind::ALL
            .into_iter()
            .map(|kind| {
                let image = load_basis(&dir.join("landmarks").join(format!("{}.basis", kind.file())))?;
                Some((kind, texture(ctx, "level_landmark", &image, TextureOptions::LINEAR)))
            })
            .collect::<Option<Vec<_>>>()?;
        let exits = load_basis(&dir.join("exits.basis"))?;
        let monsters = [
            (MonsterKind::Skeleton, "skeleton"),
            (MonsterKind::Bat, "bat"),
            (MonsterKind::Goblin, "goblin"),
            (MonsterKind::Orc, "orc"),
            (MonsterKind::Ghoul, "ghoul"),
            (MonsterKind::Wolf, "wolf"),
            (MonsterKind::Warg, "warg"),
            (MonsterKind::Troll, "troll"),
            (MonsterKind::Muldant, "muldant"),
        ]
        .into_iter()
        .map(|(kind, file)| {
            let sheet = load_basis(&dir.join("monsters").join(format!("{file}.basis")))?;
            // Direction 6 (sud), première image (le monstre au repos, de face).
            let face = full_cell("monsters", file, &sheet, 6 * FRAMES, SHEET_COLUMNS)?;
            Some((kind, texture(ctx, "level_monster", &face, TextureOptions::LINEAR)))
        })
        .collect::<Option<Vec<_>>>()?;
        let objects = OBJECT_FILES
            .iter()
            .map(|file| {
                let sheet = load_basis(&dir.join("objects").join(format!("{file}.basis")))?;
                let first = full_cell("objects", file, &sheet, 0, 20)?;
                Some(texture(ctx, "level_object", &first, TextureOptions::LINEAR))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            tiles: texture(ctx, "level_tiles", &tiles, TextureOptions::NEAREST),
            objects,
            landmarks,
            decor: texture(ctx, "level_decor", &decor, TextureOptions::NEAREST),
            exits: texture(ctx, "level_exits", &exits, TextureOptions::LINEAR),
            monsters,
        })
    }

    /// Le dessin d'un élément de décor.
    pub fn decor(&self, kind: castle_shared::DecorKind, variant: u8) -> (&TextureHandle, Rect) {
        let (kinds, variants) = (castle_shared::DecorKind::ALL.len() as f32, castle_shared::DECOR_VARIANTS as f32);
        let (column, row) = (kind.column() as f32, f32::from(variant % castle_shared::DECOR_VARIANTS));
        let uv = Rect::from_min_max(pos2(column / kinds, row / variants), pos2((column + 1.0) / kinds, (row + 1.0) / variants));
        (&self.decor, uv)
    }

    /// Une tuile de la planche (`column dans la ligne de la région).
    pub fn tile(&self, region: Region, column: usize) -> (&TextureHandle, Rect) {
        let row = Region::ALL.iter().position(|r| *r == region).unwrap_or(0) as u32;
        (
            &self.tiles,
            cell_uv(
                column as u32,
                row,
                TILE_CELL,
                TILE_COLUMNS as u32,
                TILE_ROWS,
            ),
        )
    }

    fn object(&self, row: usize, tint: Color32) -> Sprite<'_> {
        Sprite {
            texture: &self.objects[row],
            uv: Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            size: 2.0,
            lift: 0.83,
            tint,
        }
    }

    fn monster(&self, kind: MonsterKind) -> Sprite<'_> {
        let texture = &self
            .monsters
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, t)| t)
            .unwrap_or(&self.monsters[0].1);
        // Les loups et les orques sont dessinés plus grands dans le jeu.
        let size = match kind {
            MonsterKind::Orc => 2.8,
            MonsterKind::Wolf => 2.4,
            MonsterKind::Warg => 6.2,
            MonsterKind::Troll => 6.0,
            MonsterKind::Muldant => 6.0,
            _ => 2.0,
        };
        Sprite {
            texture,
            uv: Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            size,
            lift: 0.417 * size,
            tint: Color32::WHITE,
        }
    }

    pub fn exit(&self, kind: ExitKind) -> Sprite<'_> {
        let row = match kind {
            ExitKind::Stairs => 0,
            ExitKind::Gate => 1,
            ExitKind::Bridge => 3,
            ExitKind::Drawbridge => 5,
        };
        Sprite {
            texture: &self.exits,
            uv: cell_uv(0, row, EXIT_CELL, EXIT_FRAMES, 7),
            size: 4.0,
            lift: 0.0,
            tint: Color32::WHITE,
        }
    }

    /// Le sprite d'un élément, quand il en a un (les autres n'ont qu'un repère sur la carte).
    pub fn thing(&self, thing: &Thing) -> Option<Sprite<'_>> {
        let white = Color32::WHITE;
        Some(match thing {
            Thing::Monster { kind, .. } => self.monster(*kind),
            Thing::Generator { .. } => self.object(0, white),
            Thing::Trap { .. } => self.object(8, white),
            Thing::Relic { .. } => self.object(4, Color32::from_rgb(140, 255, 255)),
            Thing::Captive { .. } => self.object(25, white),
            Thing::Guide { caged, .. } => self.object(if *caged { 27 } else { 26 }, white),
            Thing::Portcullis { .. } => self.object(21, white),
            Thing::Panel { .. } => self.object(15, white),
            Thing::Pickup { kind, .. } => self.object(
                match kind {
                    PickupKind::Coin => 2,
                    PickupKind::Food => 3,
                    PickupKind::Key => 4,
                    PickupKind::Potion => 14,
                    PickupKind::Ingredient => 22,
                    PickupKind::GearPart => 23,
                    PickupKind::TargetDisc => 24,
                },
                white,
            ),
            Thing::Exit { kind, .. } => self.exit(*kind),
            Thing::Landmark { kind, .. } => {
                let texture = &self.landmarks.iter().find(|(k, _)| k == kind)?.1;
                // La case fait 64 unités (quatre tuiles), le pied du motif est à 92 % de sa hauteur.
                let size = 4.0 * kind.display_scale();
                Sprite { texture, uv: Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), size, lift: 0.4167 * size, tint: white }
            }
            Thing::Ward { .. } | Thing::Trigger { .. } => return None,
        })
    }

    /// Le sprite d'un élément de la palette, sans position.
    pub fn palette(&self, thing: &Thing) -> Option<Sprite<'_>> {
        self.thing(thing)
    }
}
