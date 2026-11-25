# AGENTS.md — moved

This file used to hold a second copy of the project rules. It drifted from the
real one while the Tauri app lived in a nested git repository.

**The source of truth is [`../AGENTS.md`](../AGENTS.md)** at the repository root.
Read that before touching anything here.

Quick orientation for this directory:

- `dstauri/` is the Tauri v2 + Rust + Next.js app (part of the Donghua-stream repo).
- The legacy PyQt6 app it ports lives at the repository root (`../app_config.py`, `../db.py`, …).
- Rust code: `src-tauri/src/`. Frontend: `app/`, `components/`, `lib/`, `hooks/`
  (there is no `src/` directory on the frontend side).
- CI and release workflows: `../.github/workflows/`.
