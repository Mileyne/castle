//! Choix de la case de la planche des tuiles pour chaque tuile d'un niveau (identique à `tools/sprite_forge`).
//!
//! Le sol et les faces de mur sont de grands motifs (4x4 et 4x1 tuiles): on choisit un motif par bloc, et la tuile à sa
//! place dans le bloc, pour que les dalles et les joints se prolongent d'une tuile à l'autre. Le jeu et l'éditeur de
//! niveaux s'en servent tous deux, pour que la carte montre ce que le joueur verra.

/// Colonnes d'une ligne de la planche des tuiles (une ligne par région): 3 sols, 2 dessus de mur, 2 faces de mur, 2 faces décorées.
pub const TILE_COLUMNS: usize = 90;
/// Lignes de la planche: une par région (six du château, trois extérieures).
pub const TILE_ROWS: u32 = 9;
/// Côté d'une tuile dans la planche, en pixels.
pub const TILE_CELL: u32 = 48;
const MACRO: u32 = 4;
const FLOOR_PATTERNS: u32 = 3;
const TOP_START: u32 = 48;
const TOP_PATTERNS: u32 = 2;
const FACE_PATTERNS: u32 = 2;
const FACE_START: usize = 80;
const DECOR_START: usize = 88;

/// Colonne de la planche pour la tuile (`x`, `y`, `y` croissant vers le haut) de la région `region_row`. `wall` dit si
/// c'est un mur, `floor_below` si la tuile du dessous est un sol (le mur montre alors sa face).
pub fn tile_column(x: u32, y: u32, region_row: usize, wall: bool, floor_below: bool) -> usize {
    let block = (x / MACRO).wrapping_mul(73856093)
        ^ (y / MACRO).wrapping_mul(19349663)
        ^ (region_row as u32).wrapping_mul(83492791);
    let face_block = (x / MACRO).wrapping_mul(73856093)
        ^ y.wrapping_mul(19349663)
        ^ (region_row as u32).wrapping_mul(83492791);
    let noise = (x * 31 + y * 17 + x * y * 3) % 23;
    if !wall {
        let pattern = (block >> 5) % FLOOR_PATTERNS;
        // La ligne 0 du motif est en haut, alors que `y` croît vers le haut.
        (pattern * MACRO * MACRO + (MACRO - 1 - y % MACRO) * MACRO + x % MACRO) as usize
    } else if floor_below {
        if noise.is_multiple_of(9) {
            DECOR_START + (noise as usize / 9) % 2
        } else {
            FACE_START + (((face_block >> 5) % FACE_PATTERNS) * MACRO + x % MACRO) as usize
        }
    } else {
        let pattern = (block >> 7) % TOP_PATTERNS;
        (TOP_START + pattern * MACRO * MACRO + (MACRO - 1 - y % MACRO) * MACRO + x % MACRO) as usize
    }
}
