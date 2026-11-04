#!/usr/bin/env python3
"""Génère, pour chaque bruitage du jeu, dix variantes à comparer (voir `app/`).

Écrit `tools/sfx/variants/<nom>/<n>.wav` (n de 0 à 10): 0 est le son actuel du jeu, 1 à 10 sont les variantes. Les sons actuels sont
copiés une fois dans `tools/sfx/original/` (pour pouvoir revenir en arrière même après avoir appliqué un choix).

- Pour chaque son, les variantes sont des traitements du son actuel (plus court, plus filtré, plus aigu, plus grave, saturé, avec écho...),
  chacun avec son propre tirage de bruit.
- L'épée du Chevalier a en plus dix conceptions propres (durée, bande de fréquences, attaque, métal...), parce qu'elle était jugée trop longue
  et pas assez filtrée.

Usage: python3 tools/sfx/variants.py [nom ...]   (sans argument: tous les sons)
"""
import math
import shutil
import struct
import subprocess
import sys
import wave
from multiprocessing import Pool
from pathlib import Path

import make_sfx as m
from make_sfx import SR, bell, bp, env, gain, hp, lp, mix, n, noise, osc, sat

HERE = Path(__file__).resolve().parent
OUT = HERE / "variants"
ORIGINAL = HERE / "original"
GAME = m.OUT

GROUPS = [
    ("Armes", [("swing_knight", "Épée du Chevalier"), ("swing_spell", "Sort de l'Ensorceleur"), ("swing_arrow", "Arc du Rôdeur"),
               ("swing_hammer", "Marteau du Forgebraise")]),
    ("Impacts", [("hit_flesh", "Coup sur la chair"), ("hit_bone", "Coup sur les os"), ("hit_stone", "Coup sur la pierre")]),
    ("Héros", [("hurt_hero", "Héros blessé"), ("downed", "Héros à terre")]),
    ("Monstres", [("skeleton_hurt", "Squelette blessé"), ("skeleton_die", "Squelette détruit"), ("bat_hurt", "Chauve-souris blessée"),
                  ("bat_die", "Chauve-souris morte"), ("goblin_hurt", "Gobelin blessé"), ("goblin_die", "Gobelin mort"),
                  ("orc_hurt", "Orque blessé"), ("orc_die", "Orque mort"), ("wolf_hurt", "Loup blessé"), ("wolf_die", "Loup mort"),
                  ("ghoul_hurt", "Goule blessée"), ("ghoul_die", "Goule morte")]),
    ("Objets", [("pickup_coin", "Pièce"), ("pickup_food", "Nourriture"), ("pickup_potion", "Potion"), ("pickup_key", "Clé"),
                ("pickup_item", "Objet de quête")]),
    ("Événements", [("freed", "Prisonnier délivré"), ("puzzle", "Énigme résolue"), ("door", "Porte"), ("portcullis", "Herse"),
                    ("exit_open", "Sortie ouverte"), ("crash", "Éboulement")]),
]

# --- Traitements --------------------------------------------------------------------------------------------------------


def speed(x, s):
    """Lit le son `s` fois plus vite (plus court et plus aigu si s > 1)."""
    out = []
    for i in range(int(len(x) / s)):
        p = i * s
        j = int(p)
        f = p - j
        a = x[j]
        b = x[j + 1] if j + 1 < len(x) else 0.0
        out.append(a + (b - a) * f)
    return out


def tighten(x, seconds):
    """Coupe le son en `seconds` secondes avec une chute rapide."""
    k = min(len(x), n(seconds * 1.4))
    return [v * math.exp(-3.2 * i / n(seconds)) for i, v in enumerate(x[:k])]


def echo(x, delay=0.085, feedback=0.38, taps=4):
    out = list(x) + [0.0] * n(delay * taps + 0.05)
    for t in range(1, taps + 1):
        off = n(delay * t)
        g = feedback**t
        for i, v in enumerate(x):
            out[off + i] += v * g
    return lp(out, 4500)


def shorter(x, factor):
    return speed(x, factor)


PRESETS = [
    ("Plus court", lambda x: speed(x, 1.35)),
    ("Très court et sec", lambda x: tighten(speed(x, 1.6), 0.2)),
    ("Filtré (sourd)", lambda x: lp(x, 2600)),
    ("Très filtré (étouffé)", lambda x: lp(speed(x, 1.1), 1300)),
    ("Brillant", lambda x: mix(x, gain(hp(x, 1800), 0.7))),
    ("Plus grave", lambda x: lp(speed(x, 0.8), 5000)),
    ("Plus aigu", lambda x: hp(speed(x, 1.3), 250)),
    ("Épais (saturé)", lambda x: lp(sat(x, 3.0), 4200)),
    ("Étroit (radio)", lambda x: tighten(bp(x, 450, 3200), max(0.25, len(x) / SR * 0.6))),
    ("Ample (écho)", lambda x: echo(x)),
]

# --- L'épée du Chevalier ---------------------------------------------------------------------------------------------------


def sword(d, lo, peak, hp_cut=0.0, lp_cut=0.0, bell_peak=0.4, ring=0.0, thump=0.0, g=1.6):
    x = lp(noise(d), lambda t: lo + (peak - lo) * math.sin(math.pi * min(t / d, 1.0)) ** 2)
    if hp_cut:
        x = hp(x, hp_cut)
    if lp_cut:
        x = lp(x, lp_cut)
    x = gain(bell(x, bell_peak), g)
    tracks, at = [x], [0.0]
    if ring:
        tracks.append(gain(env(mix(osc(2300, 2250, 0.25), gain(osc(3400, 3350, 0.25), 0.4)), 0.001, 0.12, 4), ring))
        at.append(d * 0.35)
    if thump:
        tracks.append(gain(env(osc(130, 60, 0.15), 0.002, 0.08, 4), thump))
        at.append(d * 0.55)
    return mix(*tracks, at=at)


SWORDS = [
    ("Tranchant court", dict(d=0.14, lo=800, peak=5000, hp_cut=700)),
    ("Filtré net", dict(d=0.18, lo=400, peak=2800, hp_cut=400, lp_cut=3200)),
    ("Sourd et rapide", dict(d=0.16, lo=300, peak=1800, lp_cut=2200)),
    ("Lame fine", dict(d=0.12, lo=1500, peak=6000, hp_cut=1500)),
    ("Souffle moyen filtré", dict(d=0.22, lo=400, peak=2500, hp_cut=250, lp_cut=3000)),
    ("Lame avec éclat métallique", dict(d=0.18, lo=600, peak=3800, hp_cut=500, ring=0.35)),
    ("Fouet aigu", dict(d=0.10, lo=2000, peak=7000, hp_cut=1200)),
    ("Large et grave", dict(d=0.20, lo=200, peak=1500, lp_cut=2000)),
    ("Sec, attaque nette", dict(d=0.15, lo=500, peak=3500, hp_cut=350, bell_peak=0.22)),
    ("Épée lourde", dict(d=0.26, lo=250, peak=2000, lp_cut=2600, thump=0.8, bell_peak=0.5)),
]

# --- Écriture ----------------------------------------------------------------------------------------------------------------


def write_wav(path, samples):
    peak = max(1e-6, max(abs(v) for v in samples))
    scale = 0.85 / peak
    fade = n(0.01)
    data = []
    for i, v in enumerate(samples):
        v *= scale
        if i >= len(samples) - fade:
            v *= (len(samples) - i) / fade
        data.append(int(max(-1, min(1, v)) * 32767))
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(b"".join(struct.pack("<h", s) for s in data))


def decode_current(name):
    """Le son actuel du jeu (ou, à défaut, sa synthèse) en WAV, et sa copie de référence."""
    ORIGINAL.mkdir(exist_ok=True)
    kept = ORIGINAL / f"{name}.ogg"
    if not kept.exists():
        shutil.copy(GAME / f"{name}.ogg", kept)
    target = OUT / name / "0.wav"
    target.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(kept), "-ac", "1", "-ar", str(SR), str(target)], check=True)


def build(name):
    decode_current(name)
    sound = m.SOUNDS[name]
    for k in range(10):
        m.rng.seed(1000 * (k + 1) + sum(map(ord, name)))
        if name == "swing_knight":
            x = sword(**SWORDS[k][1])
        else:
            x = PRESETS[k][1](sound())
        write_wav(OUT / name / f"{k + 1}.wav", x)
    return name


def labels(name):
    if name == "swing_knight":
        return [label for label, _ in SWORDS]
    return [label for label, _ in PRESETS]


if __name__ == "__main__":
    names = sys.argv[1:] or [name for _, items in GROUPS for name, _ in items]
    unknown = [x for x in names if x not in m.SOUNDS]
    if unknown:
        sys.exit(f"sons inconnus: {unknown}")
    with Pool() as pool:
        for done in pool.imap_unordered(build, names):
            print(done, flush=True)
