//! Généré par `tools/sprite_forge` (ne pas modifier à la main): taille des cases rognées de chaque planche, et position de cette
//! zone dans la case d'origine de CELL pixels.

/// Côté de la case d'origine, en pixels.
pub const CELL: u32 = 288;

/// (dossier, nom, x, y, largeur, hauteur) de la zone conservée dans chaque planche.
pub const SHEETS: &[(&str, &str, u32, u32, u32, u32)] = &[
    ("heroes", "knight", 0, 0, 288, 288),
    ("heroes", "sorcerer", 0, 0, 288, 288),
    ("heroes", "ranger", 32, 0, 224, 288),
    ("heroes", "forgebraise", 0, 0, 288, 288),
    ("monsters", "skeleton", 0, 12, 288, 276),
    ("monsters", "bat", 31, 144, 228, 144),
    ("monsters", "goblin", 25, 116, 240, 172),
    ("monsters", "orc", 0, 0, 288, 288),
    ("monsters", "ghoul", 0, 40, 288, 248),
    ("monsters", "wolf", 0, 52, 288, 236),
    ("monsters", "warg", 0, 104, 288, 184),
    ("monsters", "troll", 0, 28, 288, 260),
    ("monsters", "lich", 0, 12, 288, 276),
    ("objects", "generator", 15, 96, 260, 192),
    ("objects", "turret", 67, 140, 156, 148),
    ("objects", "coin", 82, 188, 124, 100),
    ("objects", "food", 82, 192, 144, 96),
    ("objects", "key", 82, 192, 124, 96),
    ("objects", "lever", 82, 136, 124, 152),
    ("objects", "rune", 48, 176, 192, 112),
    ("objects", "mechanism", 63, 112, 168, 176),
    ("objects", "traparmed", 67, 176, 156, 112),
    ("objects", "trapoff", 67, 220, 156, 68),
    ("objects", "doorlocked", 57, 28, 176, 260),
    ("objects", "doorroyal", 65, 44, 160, 244),
    ("objects", "doorroyalemblem", 65, 44, 160, 244),
    ("objects", "stairs", 48, 108, 192, 180),
    ("objects", "potion", 82, 152, 124, 136),
    ("objects", "wallrune", 73, 73, 144, 144),
    ("objects", "wallgearsjammed", 73, 73, 144, 144),
    ("objects", "wallgearsfixed", 73, 73, 144, 144),
    ("objects", "walltargetempty", 73, 73, 144, 144),
    ("objects", "walltargetmounted", 73, 73, 144, 144),
    ("objects", "walltargethit", 73, 73, 144, 144),
    ("objects", "portcullis", 65, 40, 160, 248),
    ("objects", "ingredient", 82, 192, 124, 96),
    ("objects", "gearpart", 82, 187, 124, 100),
    ("objects", "targetdisc", 82, 180, 124, 108),
    ("objects", "captivecage", 73, 44, 144, 244),
];
