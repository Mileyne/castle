//! Retouches de tuiles faites à la main dans l'éditeur.
//!
//! Chaque tuile retouchée est un PNG de 48x48 (`art_overrides/<thème>/tiles/r<région>_c<colonne>.png`) qui remplace la
//! tuile générée au même emplacement. Les retouches vivent à part des planches générées: relancer la génération ne
//! les écrase jamais.

use std::path::PathBuf;

use crate::shapes::Theme;

/// Côté d'une tuile, en pixels.
pub const TILE_PX: usize = 48;

/// Dossier des retouches et des réglages (`CASTLE_ART_OVERRIDES` pour le changer).
pub fn root() -> PathBuf {
    match std::env::var_os("CASTLE_ART_OVERRIDES") {
        Some(dir) => dir.into(),
        None => PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/art_overrides")),
    }
}

fn tiles_dir(theme: Theme) -> PathBuf {
    root().join(theme.folder()).join("tiles")
}

fn tile_path(theme: Theme, region: usize, col: usize) -> PathBuf {
    tiles_dir(theme).join(format!("r{region}_c{col:02}.png"))
}

/// Pixels RGBA (48x48) de la tuile retouchée, s'il y en a une.
pub fn load_tile(theme: Theme, region: usize, col: usize) -> Option<Vec<u8>> {
    let file = std::fs::File::open(tile_path(theme, region, col)).ok()?;
    let mut reader = png::Decoder::new(std::io::BufReader::new(file)).read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let ok = info.width as usize == TILE_PX && info.height as usize == TILE_PX;
    (ok && info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight).then(|| {
        buf.truncate(TILE_PX * TILE_PX * 4);
        buf
    })
}

/// Toutes les tuiles retouchées d'un thème, par (région, colonne), avec des pixels en virgule flottante.
pub(crate) fn load_all(theme: Theme) -> std::collections::HashMap<(usize, usize), Vec<[f32; 4]>> {
    let mut all = std::collections::HashMap::new();
    let Ok(entries) = std::fs::read_dir(tiles_dir(theme)) else {
        return all;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix('r').and_then(|n| n.strip_suffix(".png")) else {
            continue;
        };
        let Some((region, col)) = rest.split_once("_c") else {
            continue;
        };
        let (Ok(region), Ok(col)) = (region.parse::<usize>(), col.parse::<usize>()) else {
            continue;
        };
        if let Some(rgba) = load_tile(theme, region, col) {
            let pixels = rgba.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]].map(|c| c as f32 / 255.0)).collect();
            all.insert((region, col), pixels);
        }
    }
    all
}

/// Enregistre une tuile retouchée (48x48 RGBA).
pub fn save_tile(theme: Theme, region: usize, col: usize, rgba: &[u8]) -> std::io::Result<()> {
    assert_eq!(rgba.len(), TILE_PX * TILE_PX * 4, "une tuile fait 48x48 pixels");
    std::fs::create_dir_all(tiles_dir(theme))?;
    let file = std::fs::File::create(tile_path(theme, region, col))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), TILE_PX as u32, TILE_PX as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().map_err(std::io::Error::other)?.write_image_data(rgba).map_err(std::io::Error::other)
}

/// Retire la retouche d'une tuile: la tuile générée reprend sa place.
pub fn delete_tile(theme: Theme, region: usize, col: usize) {
    let _ = std::fs::remove_file(tile_path(theme, region, col));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_tile_comes_back_identical_and_can_be_deleted() {
        let dir = std::env::temp_dir().join(format!("castle_overrides_{}", std::process::id()));
        // Un seul test touche à l'environnement.
        unsafe { std::env::set_var("CASTLE_ART_OVERRIDES", &dir) };
        let rgba: Vec<u8> = (0..TILE_PX * TILE_PX * 4).map(|i| (i * 7 % 251) as u8).collect();
        save_tile(Theme::Medieval, 2, 17, &rgba).unwrap();
        assert_eq!(load_tile(Theme::Medieval, 2, 17).as_deref(), Some(rgba.as_slice()));
        assert!(load_all(Theme::Medieval).contains_key(&(2, 17)));
        assert!(load_tile(Theme::Manga, 2, 17).is_none());
        delete_tile(Theme::Medieval, 2, 17);
        assert!(load_tile(Theme::Medieval, 2, 17).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
