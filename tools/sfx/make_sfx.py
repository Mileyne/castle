#!/usr/bin/env python3
"""Bruitages du jeu, synthétisés sans dépendance (bibliothèque standard + ffmpeg).

Écrit `client/assets/audio/sfx/<nom>.ogg`. Pour remplacer un son par un enregistrement, il suffit de déposer un fichier du même
nom au même endroit: le client charge les bruitages par nom (voir `client/src/sfx.rs`).

Usage: python3 tools/sfx/make_sfx.py [nom ...]
"""
import math
import random
import struct
import subprocess
import sys
import tempfile
import wave
from pathlib import Path

SR = 32000
OUT = Path(__file__).resolve().parents[2] / "client/assets/audio/sfx"
rng = random.Random(7)


def n(dur):
    return int(SR * dur)


def noise(dur):
    return [rng.uniform(-1, 1) for _ in range(n(dur))]


def lp(x, fc):
    """Passe-bas à un pôle; `fc` est un nombre ou une fonction du temps (secondes)."""
    y, s = [], 0.0
    for i, v in enumerate(x):
        f = fc(i / SR) if callable(fc) else fc
        a = 1 - math.exp(-2 * math.pi * max(f, 20) / SR)
        s += a * (v - s)
        y.append(s)
    return y


def hp(x, fc):
    low = lp(x, fc)
    return [a - b for a, b in zip(x, low)]


def bp(x, lo, hi):
    return lp(hp(x, lo), hi)


def env(x, attack=0.005, decay=0.1, curve=3.0):
    """Enveloppe: montée linéaire puis chute exponentielle de durée `decay`."""
    out = []
    for i, v in enumerate(x):
        t = i / SR
        a = min(t / attack, 1.0) if attack > 0 else 1.0
        out.append(v * a * math.exp(-curve * t / decay))
    return out


def bell(x, peak=0.5):
    """Enveloppe en cloche: monte jusqu'à `peak` (part de la durée), redescend."""
    m = len(x)
    return [v * math.sin(math.pi * (i / m) ** (math.log(0.5) / math.log(peak))) for i, v in enumerate(x)]


def osc(f0, f1, dur, shape="sine", vib=0.0, vib_hz=6.0):
    """Oscillateur dont la fréquence glisse de f0 à f1 (exponentiellement)."""
    out, ph = [], 0.0
    m = n(dur)
    for i in range(m):
        t = i / m
        f = f0 * (f1 / f0) ** t
        f *= 1 + vib * math.sin(2 * math.pi * vib_hz * i / SR)
        ph += f / SR
        p = ph % 1.0
        if shape == "sine":
            v = math.sin(2 * math.pi * p)
        elif shape == "saw":
            v = 2 * p - 1
        elif shape == "square":
            v = 1.0 if p < 0.5 else -1.0
        else:  # triangle
            v = 4 * abs(p - 0.5) - 1
        out.append(v)
    return out


def mix(*tracks, at=None):
    at = at or [0.0] * len(tracks)
    length = max(int(a * SR) + len(t) for t, a in zip(tracks, at))
    out = [0.0] * length
    for t, a in zip(tracks, at):
        o = int(a * SR)
        for i, v in enumerate(t):
            out[o + i] += v
    return out


def gain(x, g):
    return [v * g for v in x]


def am(x, hz, depth=0.6):
    return [v * (1 - depth + depth * math.sin(2 * math.pi * hz * i / SR)) for i, v in enumerate(x)]


def sat(x, drive=2.0):
    return [math.tanh(v * drive) for v in x]


def ping(f, dur=0.35, g=1.0, shimmer=True):
    x = osc(f, f, dur)
    if shimmer:
        x = mix(x, gain(osc(f * 2.01, f * 2.01, dur), 0.35), gain(osc(f * 3.02, f * 3.02, dur), 0.12))
    return gain(env(x, 0.002, dur * 0.55, 3.5), g)


def clicks(count, span, f_lo=700, f_hi=2600, g=1.0):
    """Petits claquements secs (os, chaînes) répartis sur `span` secondes."""
    parts, at = [], []
    for _ in range(count):
        f = rng.uniform(f_lo, f_hi)
        d = rng.uniform(0.03, 0.07)
        parts.append(gain(env(mix(osc(f, f * 0.7, d), gain(noise(d), 0.3)), 0.0005, d * 0.5, 4), rng.uniform(0.4, 1.0) * g))
        at.append(rng.uniform(0, span))
    return mix(*parts, at=at)


# --- Armes -----------------------------------------------------------------------------------------------------------

def swing_knight():
    d = 0.3
    x = lp(noise(d), lambda t: 500 + 3500 * math.sin(math.pi * min(t / d, 1.0)) ** 2)
    return gain(bell(x, 0.4), 1.6)


def swing_hammer():
    d = 0.42
    w = gain(bell(lp(noise(d), lambda t: 250 + 1400 * math.sin(math.pi * min(t / d, 1.0)) ** 2), 0.6), 1.8)
    thump = gain(env(osc(110, 45, 0.3), 0.002, 0.18, 4), 1.2)
    return mix(w, thump, at=[0, 0.3])


def swing_arrow():
    d = 0.5
    twang = gain(env(mix(osc(310, 290, d), gain(osc(620, 580, d), 0.4)), 0.001, 0.25, 4), 1.0)
    fly = gain(bell(hp(lp(noise(0.25), lambda t: 1500 + 5000 * t), 1500), 0.3), 1.0)
    click = gain(env(noise(0.02), 0, 0.01, 4), 0.8)
    return mix(click, twang, fly, at=[0, 0.005, 0.02])


def swing_spell():
    d = 0.55
    a = osc(380, 1500, d, "sine", vib=0.03, vib_hz=14)
    b = osc(570, 2250, d, "triangle", vib=0.02, vib_hz=11)
    sparkle = gain(env(hp(noise(d), 4500), 0.01, 0.25, 3), 0.5)
    return mix(gain(env(mix(a, gain(b, 0.5)), 0.02, 0.35, 3), 0.9), sparkle)


# --- Impacts ---------------------------------------------------------------------------------------------------------

def hit_flesh():
    return mix(gain(env(lp(noise(0.14), 1800), 0.001, 0.07, 3.5), 1.7), gain(env(osc(140, 55, 0.18), 0.001, 0.1, 4), 1.2))


def hit_bone():
    return mix(gain(env(hp(noise(0.1), 1500), 0.0005, 0.04, 4), 1.0), gain(env(osc(950, 700, 0.12), 0.0005, 0.05, 4), 0.7),
               gain(env(osc(1500, 1200, 0.1), 0.0005, 0.04, 4), 0.5), gain(env(osc(180, 90, 0.12), 0.001, 0.06, 4), 0.8))


def hit_stone():
    return mix(gain(env(bp(noise(0.3), 300, 3000), 0.001, 0.13, 3.5), 1.6), gain(env(osc(210, 80, 0.3), 0.001, 0.16, 4), 1.2),
               clicks(4, 0.2, 1500, 4000, 0.4))


# --- Voix: héros ------------------------------------------------------------------------------------------------------

def hurt_hero():
    v = lp(osc(250, 150, 0.22, "saw", vib=0.04, vib_hz=18), 1100)
    return gain(env(mix(v, gain(lp(noise(0.22), 1500), 0.2)), 0.01, 0.14, 3), 1.4)


def downed():
    v = lp(osc(220, 70, 0.8, "saw", vib=0.05, vib_hz=7), lambda t: 1000 - 700 * t)
    return gain(env(mix(v, gain(lp(noise(0.8), 600), 0.15)), 0.03, 0.55, 3), 1.4)


# --- Voix: monstres (blessure `_hurt`, mort `_die`) ---------------------------------------------------------------------

def skeleton_hurt():
    return mix(clicks(5, 0.18, 900, 2800, 0.9), gain(env(hp(noise(0.12), 2200), 0.001, 0.05, 4), 0.5))


def skeleton_die():
    return mix(clicks(18, 0.7, 500, 2600, 0.9), gain(env(lp(noise(0.5), 700), 0.01, 0.3, 3), 0.5), at=[0, 0.05])


def bat_hurt():
    return gain(env(mix(osc(3800, 5200, 0.14, "sine", vib=0.05, vib_hz=40), gain(osc(1900, 2600, 0.14), 0.4)), 0.003, 0.08, 3), 0.9)


def bat_die():
    return gain(env(mix(osc(4800, 1200, 0.35, "sine", vib=0.06, vib_hz=35), gain(osc(2400, 600, 0.35, "triangle"), 0.4)), 0.003, 0.22, 3), 0.9)


def goblin_hurt():
    return gain(env(lp(osc(650, 1000, 0.16, "saw", vib=0.05, vib_hz=25), 2500), 0.004, 0.1, 3), 1.0)


def goblin_die():
    return gain(env(lp(osc(900, 180, 0.5, "saw", vib=0.06, vib_hz=20), lambda t: 3000 - 4000 * t + 300), 0.004, 0.32, 3), 1.0)


def orc_hurt():
    return gain(env(lp(osc(135, 95, 0.25, "saw", vib=0.03, vib_hz=12), 700), 0.01, 0.16, 3), 2.0)


def orc_die():
    v = lp(osc(120, 42, 0.8, "saw", vib=0.05, vib_hz=8), lambda t: 700 - 400 * t)
    return gain(env(mix(v, gain(lp(noise(0.8), 400), 0.25)), 0.02, 0.5, 3), 2.0)


def wolf_hurt():
    return gain(env(lp(osc(720, 380, 0.25, "saw", vib=0.04, vib_hz=16), 2200), 0.005, 0.15, 3), 1.0)


def wolf_die():
    growl = am(lp(osc(95, 70, 0.4, "saw"), 600), 32, 0.7)
    howl = lp(osc(330, 520, 0.7, "triangle", vib=0.02, vib_hz=6), 1500)
    return mix(gain(env(growl, 0.01, 0.25, 3), 2.0), gain(env(howl, 0.15, 0.5, 2.5), 0.8), at=[0, 0.2])


def ghoul_hurt():
    return gain(env(mix(lp(osc(210, 150, 0.35, "saw", vib=0.06, vib_hz=9), 1200), gain(lp(noise(0.35), 2000), 0.25)), 0.03, 0.22, 3), 1.2)


def ghoul_die():
    v = lp(osc(240, 85, 1.0, "saw", vib=0.08, vib_hz=7), lambda t: 1500 - 1100 * t)
    return gain(env(mix(v, gain(lp(noise(1.0), 1800), 0.3)), 0.06, 0.65, 3), 1.2)


# --- Objets ----------------------------------------------------------------------------------------------------------

def pickup_coin():
    return mix(ping(1568, 0.3, 0.9), ping(2093, 0.45, 0.9), at=[0, 0.07])


def pickup_food():
    bite = gain(env(bp(noise(0.12), 500, 3500), 0.001, 0.05, 4), 1.4)
    pop = gain(env(osc(420, 220, 0.12), 0.001, 0.06, 4), 0.9)
    return mix(bite, pop, gain(env(bp(noise(0.1), 700, 3000), 0.001, 0.04, 4), 1.0), at=[0, 0, 0.1])


def pickup_potion():
    bubbles = [gain(env(osc(rng.uniform(250, 420), rng.uniform(600, 900), 0.12), 0.003, 0.07, 3.5), 0.8) for _ in range(5)]
    return mix(gain(env(lp(noise(0.5), 900), 0.02, 0.3, 3), 0.25), *bubbles, at=[0] + [0.03 + i * 0.07 for i in range(5)])


def pickup_key():
    def tinkle(f, d):
        return gain(env(mix(osc(f, f, d), gain(osc(f * 2.76, f * 2.76, d), 0.5), gain(osc(f * 5.4, f * 5.4, d), 0.25)), 0.001, d * 0.45, 3.5), 0.7)
    return mix(tinkle(2350, 0.5), tinkle(3140, 0.55), gain(clicks(2, 0.08, 2000, 4000, 0.5), 1), at=[0, 0.06, 0])


def pickup_item():
    return mix(ping(660, 0.3, 0.7), ping(880, 0.3, 0.7), ping(1320, 0.5, 0.8), at=[0, 0.08, 0.16])


# --- Événements ------------------------------------------------------------------------------------------------------

def freed():
    notes = [523, 659, 784, 1047, 1319]
    parts = [ping(f, 0.7, 0.8) for f in notes]
    pad = gain(env(mix(osc(262, 262, 1.2), gain(osc(392, 392, 1.2), 0.7), gain(osc(523, 523, 1.2), 0.5)), 0.05, 0.8, 3), 0.35)
    return mix(pad, *parts, at=[0] + [0.08 * i for i in range(5)])


def puzzle():
    low = gain(env(lp(noise(0.6), 280), 0.02, 0.35, 3), 1.6)
    thump = gain(env(osc(90, 45, 0.4), 0.003, 0.25, 4), 1.2)
    return mix(low, thump, ping(880, 0.6, 0.8), ping(1175, 0.8, 0.8), at=[0, 0, 0.12, 0.28])


def door():
    grind = am(lp(noise(1.1), lambda t: 260 + 120 * t), 9, 0.5)
    return mix(gain(env(grind, 0.05, 1.0, 1.6), 2.2), gain(env(osc(80, 40, 0.4), 0.003, 0.25, 4), 1.4),
               gain(env(osc(70, 35, 0.3), 0.003, 0.2, 4), 1.2), at=[0, 0, 0.95])


def portcullis():
    chain = clicks(26, 1.0, 1200, 3600, 0.7)
    grind = gain(env(am(lp(noise(1.0), 400), 14, 0.6), 0.05, 0.8, 1.6), 1.0)
    return mix(chain, grind, gain(env(osc(80, 40, 0.3), 0.003, 0.2, 4), 1.2), at=[0, 0, 0.95])


def exit_open():
    chord = [262, 330, 392, 523]
    parts = [gain(env(mix(osc(f, f, 1.6), gain(osc(f * 2, f * 2, 1.6), 0.3)), 0.25, 1.0, 2.2), 0.45) for f in chord]
    return mix(*parts, at=[0.0, 0.12, 0.24, 0.36])


def crash():
    boom = gain(env(osc(120, 38, 0.7), 0.002, 0.4, 4), 1.6)
    rubble = gain(env(bp(noise(0.8), 200, 2500), 0.002, 0.4, 3), 1.8)
    return mix(boom, rubble, clicks(10, 0.5, 600, 2500, 0.5), at=[0, 0, 0.1])


SOUNDS = {name: fn for name, fn in globals().items()
          if callable(fn) and name.startswith(("swing_", "hit_", "hurt_", "pickup_")) or name in
          ("downed", "freed", "puzzle", "door", "portcullis", "exit_open", "crash")
          or name.endswith(("_hurt", "_die"))}
SOUNDS = {k: v for k, v in SOUNDS.items() if callable(v) and k not in ("ping", "clicks")}


def write(name, samples):
    peak = max(1e-6, max(abs(v) for v in samples))
    scale = 0.85 / peak
    # Léger fondu final pour éviter un claquement à la coupure.
    fade = n(0.01)
    data = []
    for i, v in enumerate(samples):
        v *= scale
        if i >= len(samples) - fade:
            v *= (len(samples) - i) / fade
        data.append(int(max(-1, min(1, v)) * 32767))
    OUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        wav = Path(tmp) / f"{name}.wav"
        with wave.open(str(wav), "wb") as w:
            w.setnchannels(2)
            w.setsampwidth(2)
            w.setframerate(SR)
            w.writeframes(b"".join(struct.pack("<hh", s, s) for s in data))
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(wav), "-c:a", "vorbis", "-strict", "-2", "-q:a", "5",
                        str(OUT / f"{name}.ogg")], check=True)


if __name__ == "__main__":
    wanted = sys.argv[1:] or sorted(SOUNDS)
    for name in wanted:
        write(name, SOUNDS[name]())
        print(name)
