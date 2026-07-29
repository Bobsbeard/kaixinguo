#!/usr/bin/env python3
"""Pistachio Dictionary (开心果词典) — CC-CEDICT -> SQLite import tool.

Reads a CC-CEDICT UTF-8 text export (cedict_1_0_ts_utf-8_mdbg.txt) and builds
resources/dictionary.db, the read-only dictionary database bundled with the app.

Usage:
    python3 tools/import_cedict.py [input.txt] [output.db]

Defaults: data/cedict_1_0_ts_utf-8_mdbg.txt -> resources/dictionary.db
"""
import re
import sqlite3
import sys
import unicodedata
from datetime import datetime, timezone
from pathlib import Path

LINE_RE = re.compile(r"^(\S+)\s+(\S+)\s+\[([^\]]+)\]\s+/(.+)/\s*$")

TONE_MARKS = {
    "a": "āáǎà", "e": "ēéěè", "i": "īíǐì",
    "o": "ōóǒò", "u": "ūúǔù", "ü": "ǖǘǚǜ",
}


def syllable_to_marks(syllable: str) -> str:
    """Convert one numbered pinyin syllable (e.g. 'kai1', 'nu:3', 'lv4') to tone marks."""
    m = re.match(r"^([A-Za-züÜ:]+?)([1-5])?$", syllable)
    if not m:
        return syllable
    body, tone = m.group(1), m.group(2)
    body = body.replace("u:", "ü").replace("U:", "Ü").replace("v", "ü").replace("V", "Ü")
    if tone is None or tone == "5":
        return body
    tone = int(tone)
    lower = body.lower()
    # Placement rules: a/e take the mark; 'ou' -> o; otherwise the last vowel.
    if "a" in lower:
        idx = lower.index("a")
    elif "e" in lower:
        idx = lower.index("e")
    elif "ou" in lower:
        idx = lower.index("o")
    else:
        idx = -1
        for i in range(len(lower) - 1, -1, -1):
            if lower[i] in TONE_MARKS:
                idx = i
                break
        if idx == -1:
            return body
    ch = lower[idx]
    marked = TONE_MARKS[ch][tone - 1]
    return body[:idx] + marked + body[idx + 1:]


def pinyin_to_marks(pinyin_numbered: str) -> str:
    return " ".join(syllable_to_marks(s) for s in pinyin_numbered.split(" "))


def pinyin_flat(pinyin_numbered: str) -> str:
    """Lowercase, toneless, letters only ('ü' -> 'v'); for matching typed pinyin."""
    s = unicodedata.normalize("NFC", pinyin_numbered.lower())
    s = s.replace("u:", "v").replace("ü", "v")
    s = re.sub(r"[1-5]", "", s)
    s = re.sub(r"[^a-z]", "", s)
    return s


def build(input_path: Path, output_path: Path) -> None:
    if output_path.exists():
        output_path.unlink()
    conn = sqlite3.connect(output_path)
    cur = conn.cursor()
    cur.executescript(
        """
        PRAGMA journal_mode = WAL;
        CREATE TABLE entries (
            id            INTEGER PRIMARY KEY,
            traditional   TEXT NOT NULL,
            simplified    TEXT NOT NULL,
            pinyin        TEXT NOT NULL,   -- as published (tone numbers)
            pinyin_marks  TEXT NOT NULL,   -- tone marks, e.g. 'kāi xīn guǒ'
            pinyin_flat   TEXT NOT NULL,   -- 'kaixinguo' (toneless, letters only)
            definitions   TEXT NOT NULL,   -- senses separated by ' / '
            char_len      INTEGER NOT NULL -- length of simplified headword
        );
        CREATE INDEX idx_entries_simp ON entries(simplified);
        CREATE INDEX idx_entries_trad ON entries(traditional);
        CREATE INDEX idx_entries_flat ON entries(pinyin_flat);
        CREATE VIRTUAL TABLE fts_english USING fts5(english, entry_id UNINDEXED);
        """
    )

    count, skipped = 0, 0
    version_line = ""
    with input_path.open(encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            if line.startswith("#"):
                if "version" in line.lower() or "date" in line.lower():
                    version_line = line.lstrip("# ").strip()
                continue
            m = LINE_RE.match(line)
            if not m:
                skipped += 1
                continue
            trad, simp, pinyin, defs = m.groups()
            defs_display = defs.replace("/", " / ").strip()
            cur.execute(
                "INSERT INTO entries (traditional, simplified, pinyin, pinyin_marks,"
                " pinyin_flat, definitions, char_len) VALUES (?,?,?,?,?,?,?)",
                (trad, simp, pinyin, pinyin_to_marks(pinyin), pinyin_flat(pinyin),
                 defs_display, len(simp)),
            )
            entry_id = cur.lastrowid
            cur.execute(
                "INSERT INTO fts_english (english, entry_id) VALUES (?,?)",
                (defs_display, entry_id),
            )
            count += 1

    cur.executescript(
        """
        CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        """
    )
    cur.executemany(
        "INSERT INTO meta (key, value) VALUES (?,?)",
        [
            ("source", "CC-CEDICT (https://cc-cedict.org/wiki/) via MDBG"),
            ("license", "Creative Commons Attribution-ShareAlike"),
            ("imported_at", datetime.now(timezone.utc).isoformat()),
            ("cedict_release", version_line or "unknown"),
            ("entry_count", str(count)),
            ("schema_version", "1"),
        ],
    )
    conn.commit()
    conn.execute("PRAGMA optimize;")
    conn.close()
    print(f"Imported {count} entries ({skipped} skipped) -> {output_path}")


if __name__ == "__main__":
    root = Path(__file__).resolve().parent.parent
    src = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "data" / "cedict_1_0_ts_utf-8_mdbg.txt"
    dst = Path(sys.argv[2]) if len(sys.argv) > 2 else root / "resources" / "dictionary.db"
    dst.parent.mkdir(parents=True, exist_ok=True)
    build(src, dst)
