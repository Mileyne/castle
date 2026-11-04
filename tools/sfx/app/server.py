#!/usr/bin/env python3
"""Petite application pour choisir, son par son, la variante qui convient (voir `tools/sfx/variants.py`).

Sert la page `index.html` et les variantes (`../variants/<nom>/<n>.wav`), garde les choix dans `tools/sfx/choices.json`, et les applique au jeu
(`client/assets/audio/sfx/<nom>.ogg`) quand on le demande: la variante 0 est le son actuel (copie de `tools/sfx/original/`).

Usage: python3 tools/sfx/app/server.py [port]    (ou ./sons.sh)
"""
import json
import subprocess
import sys
import threading
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote

APP = Path(__file__).resolve().parent
SFX = APP.parent
sys.path.insert(0, str(SFX))
import variants  # noqa: E402

CHOICES = SFX / "choices.json"
GAME = variants.GAME


def catalog():
    groups = []
    for title, items in variants.GROUPS:
        sounds = []
        for name, label in items:
            sounds.append({"name": name, "label": label, "variants": ["Actuel"] + variants.labels(name)})
        groups.append({"title": title, "sounds": sounds})
    return groups


def load_choices():
    try:
        return json.loads(CHOICES.read_text())
    except (OSError, ValueError):
        return {}


def apply_choices(choices):
    done = []
    for name, index in choices.items():
        if name not in variants.m.SOUNDS:
            continue
        target = GAME / f"{name}.ogg"
        if index == 0:
            source = variants.ORIGINAL / f"{name}.ogg"
            if source.exists():
                target.write_bytes(source.read_bytes())
        else:
            wav = variants.OUT / name / f"{index}.wav"
            subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(wav), "-ac", "2", "-c:a", "vorbis", "-strict", "-2", "-q:a", "5", str(target)],
                           check=True)
        done.append(name)
    return done


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send(self, body, kind="application/json", status=200):
        if isinstance(body, (dict, list)):
            body = json.dumps(body).encode()
        elif isinstance(body, str):
            body = body.encode()
        total = len(body)
        start, end = 0, total - 1
        # Safari ne lit un son que si le serveur sait répondre aux requêtes d'intervalle (Range): sans cela, aucun son.
        spec = self.headers.get("Range", "")
        if spec.startswith("bytes=") and status == 200:
            first, _, last = spec[6:].partition("-")
            try:
                start = int(first) if first else max(0, total - int(last))
                end = min(int(last), total - 1) if first and last else total - 1
                if start > end or start >= total:
                    raise ValueError
                status = 206
                body = body[start:end + 1]
            except ValueError:
                self.send_response(416)
                self.send_header("Content-Range", f"bytes */{total}")
                self.end_headers()
                return
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Accept-Ranges", "bytes")
        if status == 206:
            self.send_header("Content-Range", f"bytes {start}-{end}/{total}")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        path = unquote(self.path.split("?")[0])
        if path in ("/", "/index.html"):
            self.send((APP / "index.html").read_bytes(), "text/html; charset=utf-8")
        elif path == "/api/state":
            self.send({"groups": catalog(), "choices": load_choices()})
        elif path.startswith("/variants/") and path.endswith(".wav") and ".." not in path:
            file = variants.OUT / path[len("/variants/"):]
            if file.exists():
                self.send(file.read_bytes(), "audio/wav")
            else:
                self.send("absent", "text/plain", 404)
        else:
            self.send("introuvable", "text/plain", 404)

    def do_POST(self):
        size = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(size) or b"{}")
        if self.path == "/api/choices":
            CHOICES.write_text(json.dumps(body, indent=1, sort_keys=True))
            self.send({"ok": True})
        elif self.path == "/api/apply":
            CHOICES.write_text(json.dumps(body, indent=1, sort_keys=True))
            try:
                self.send({"ok": True, "applied": apply_choices(body)})
            except subprocess.CalledProcessError as err:
                self.send({"ok": False, "error": str(err)}, status=500)
        else:
            self.send("introuvable", "text/plain", 404)


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8765
    if not variants.OUT.exists():
        print("Génération des variantes...", flush=True)
        subprocess.run([sys.executable, str(SFX / "variants.py")], check=True)
    server = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    url = f"http://127.0.0.1:{port}/"
    print(f"Choix des sons: {url}  (Ctrl+C pour quitter)", flush=True)
    threading.Timer(0.6, lambda: webbrowser.open(url)).start()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
