# Tsuki (Tauri app)

This directory contains the Tauri v2 + Rust + Next.js rewrite of Donghua Stream.
See the [root README](../README.md) for installation, configuration, build
instructions, and the project disclaimer.

Quick start:

```bash
bun install --frozen-lockfile
bun run sidecars
bun run tauri dev
```

Quality gates:

```bash
bun run typecheck && bun run lint && bun run test && bun run build
cd src-tauri && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test
```

The UI is a static Next.js export; all filesystem, network, database, and
process work happens in Rust commands. Add shadcn components with
`bunx shadcn@latest` — do not hand-edit `components/ui/`.
