#!/usr/bin/env python3
"""Export the legacy donghua-stream catalog to JSON for Tsuki to import.

The legacy app keeps its catalog in PostgreSQL.  Tsuki ships with SQLite and
must not link a Postgres client, so migration is staged through this export:

    ./venv/bin/python tools/export_legacy.py

Reads the database block from ``~/.config/donghua-stream/config.json`` and
writes ``~/.local/share/com.alaricsenpai.tsuki/legacy-export.json`` (override
with ``--out``).  Tsuki imports that file automatically on the next launch,
once, and records the result in its own config.

Nothing here is imported by the desktop app; it is a one-off operator tool.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List

DEFAULT_CONFIG = Path.home() / ".config" / "donghua-stream" / "config.json"
DEFAULT_OUT = (
    Path.home() / ".local" / "share" / "com.alaricsenpai.tsuki" / "legacy-export.json"
)


def load_config(path: Path) -> Dict[str, Any]:
    if not path.exists():
        raise SystemExit(f"legacy config not found: {path}")
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def connect(config: Dict[str, Any]):
    try:
        import psycopg2
    except ImportError as exc:  # pragma: no cover - environment specific
        raise SystemExit(
            "psycopg2 is required to read the legacy catalog; "
            "run this from the legacy virtualenv"
        ) from exc

    database = config.get("database") or {}
    return psycopg2.connect(
        host=os.environ.get("LEGACY_DB_HOST", database.get("host", "localhost")),
        port=int(os.environ.get("LEGACY_DB_PORT", database.get("port", 5432))),
        user=os.environ.get("LEGACY_DB_USER", database.get("user", "postgres")),
        password=os.environ.get("LEGACY_DB_PASSWORD", database.get("password", "")),
        dbname=os.environ.get("LEGACY_DB_NAME", database.get("dbname", "donghua_db")),
    )


def _iso(value: Any) -> str:
    if isinstance(value, datetime):
        return value.astimezone().isoformat()
    return ""


def fetch_catalog(conn) -> Dict[str, Any]:
    payload: Dict[str, Any] = {"source": "donghua-stream", "series": []}
    with conn.cursor() as cursor:
        cursor.execute(
            "SELECT id, key, name, save_dir, description, created_at, updated_at "
            "FROM series ORDER BY id ASC"
        )
        series_rows = cursor.fetchall()
        for row in series_rows:
            series_id, key, name, save_dir, description, created_at, updated_at = row
            cursor.execute(
                "SELECT video_id, url, episode_number, title, status, "
                "downloaded_at, created_at "
                "FROM episodes WHERE series_id = %s ORDER BY id ASC",
                (series_id,),
            )
            episodes: List[Dict[str, Any]] = []
            for ep in cursor.fetchall():
                video_id, url, episode_number, title, status, downloaded_at, ep_created = ep
                episodes.append(
                    {
                        "videoId": video_id,
                        "url": url,
                        "episodeNumber": episode_number,
                        "title": title or "",
                        "status": status or "pending",
                        "downloadedAt": _iso(downloaded_at),
                        "createdAt": _iso(ep_created),
                    }
                )
            poster_video_id = next(
                (ep["videoId"] for ep in episodes if ep.get("videoId")), ""
            )
            payload["series"].append(
                {
                    "key": key,
                    "name": name,
                    "saveDir": save_dir,
                    "description": description or "",
                    "createdAt": _iso(created_at),
                    "updatedAt": _iso(updated_at),
                    "posterVideoId": poster_video_id,
                    "episodes": episodes,
                }
            )
    return payload


def write_export(payload: Dict[str, Any], out: Path) -> None:
    out.parent.mkdir(parents=True, exist_ok=True)
    target = out.with_suffix(out.suffix + ".tmp")
    with target.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
    os.chmod(target, 0o600)
    os.replace(target, out)
    os.chmod(out, 0o600)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    args = parser.parse_args()

    config = load_config(args.config)
    conn = connect(config)
    try:
        payload = fetch_catalog(conn)
    finally:
        conn.close()

    write_export(payload, args.out)

    series_count = len(payload["series"])
    episode_count = sum(len(series["episodes"]) for series in payload["series"])
    print(f"exported {series_count} series / {episode_count} episodes -> {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
