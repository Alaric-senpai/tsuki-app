# Tsuki design tokens

Source of truth: Google Stitch project **Donghua Streaming Hub**
(`projects/9300912284796071368`), design system asset
`assets/08fbfcc7b72c4c30a327af397a47387d`.

Tokens are implemented in [`app/globals.css`](../../app/globals.css) as CSS
variables and exposed through Tailwind v4's `@theme inline`. Components must use
the generated utilities (`bg-surface`, `text-mint`, `text-headline-md`,
`shadow-glow-sky`, `glass`, `hairline`, `telemetry`) and never raw hex values.

## Color

The app is dark-first. `.dark` holds the canonical palette; `:root` is a light
fallback kept for accessibility and OS theme switching.

| Token | Dark value | Role |
|---|---|---|
| `--background` | `#0d1419` | App canvas |
| `--surface-lowest` | `#080f14` | Wells, player backdrop |
| `--surface-low` | `#151c22` | Sidebar, popovers |
| `--surface` | `#192026` | Panels |
| `--surface-high` | `#242b30` | Raised panels |
| `--surface-highest` | `#2f363b` | Hover / active |
| `--surface-bright` | `#333a40` | Highlights |
| `--card` | `#1c2734` | L2 cards |
| `--foreground` | `#dce3ea` | Primary text |
| `--on-surface-variant` | `#bbcac0` | Secondary text |
| `--muted-foreground` | `#94a3b8` | Placeholder / tertiary |
| `--outline` | `#86948a` | Strong borders |
| `--border` | `rgb(61 79 99 / .4)` | Hairlines |
| `--primary` / `--emerald` | `#4edea3` | Actions, active, verified |
| `--mint` | `#6ffbbe` | Primary text on dark |
| `--secondary` / `--sky` | `#38bdf8` | Buffering, links, focus |
| `--amber` | `#fbbf24` | Warning / stalled |
| `--rose` | `#f4a5b8` | Error / destructive |
| `--ring` | `#38bdf8` | Focus ring |

Status chips: 22px pill, `telemetry` type, translucent `#1a2a3a` background,
hairline `#3a506a` border, 6px glowing dot (emerald active, sky buffering,
amber stalled, rose error).

## Typography

| Family | Variable | Use |
|---|---|---|
| Plus Jakarta Sans | `--font-heading` | Headlines, poster titles |
| Geist | `--font-sans` | Body copy, UI |
| JetBrains Mono | `--font-mono` | Telemetry, code, logs |

| Utility | Size / line / tracking / weight |
|---|---|
| `text-hero` | 2.75rem / 3.25rem / -0.03em / 800 |
| `text-headline-lg` | 2rem / 2.5rem / -0.02em / 700 |
| `text-headline-md` | 1.375rem / 1.875rem / -0.01em / 600 |
| `text-headline-sm` | 1.125rem / 1.5rem / — / 600 |
| `text-body-lg` | 1rem / 1.5rem |
| `text-body-md` | 0.875rem / 1.375rem |
| `text-body-sm` | 0.75rem / 1.125rem |
| `text-metric` | 1.25rem / 1.5rem / -0.01em / 600 |
| `text-code` | 0.8125rem / 1.25rem / — / 500 |
| `text-micro` | 0.6875rem / 1rem / +0.04em / 500 |

## Radius

`sm` 4px · `md` 8px (buttons, inputs) · `lg` 16px (cards, posters) · `xl` 24px ·
`2xl` 32px · `full` 9999px.

## Spacing

Base unit 4px. Gutter 16px (24px desktop), page margin 16px (32px desktop),
`xs` 4px · `sm` 8px · `md` 16px · `lg` 24px · `xl` 40px. Desktop layout is a
12-column grid at 1200px+.

## Elevation

| Level | Treatment |
|---|---|
| L0 | canvas `#0d1419` |
| L1 | `#10171c` panels |
| L2 | `#1c2734` cards + 1px `rgb(61 79 99 / .4)` |
| L3 | `#2a3645` focused + `shadow-glow-sky` or `shadow-glow-emerald` |
| Overlay | `glass` — `rgb(16 23 28 / .85)` + 16px backdrop blur |

## Component recipes

- **Primary button** — emerald background, `#0f0f0f` text, 36px tall (44px in
  playback/mobile contexts), radius `md`, hover brightness + emerald halo.
- **Secondary button** — `#1c2734` background, hairline border, `#cbd5e1` text.
- **Destructive** — transparent, rose border and text, rose-tinted hover.
- **Inputs** — `#10171c` background, hairline border, sky focus ring (2px,
  `rgb(56 189 248 / .2)`).
- **Media cards** — 16:9 or 2:3 artwork, radius `lg`, lower-third gradient,
  title in Plus Jakarta Sans, codec tags in mono. Queue cards carry a bottom
  progress bar (sky → emerald when verified).
- **Terminal** — header `#16202c`, body `#10171c`, syntax-highlighted mono.
- **Player dock** — floating pill 24px above the bottom edge, frosted
  `rgb(28 39 52 / .75)` with 12px blur; scrubber rail 4px (8px on hover),
  played portion sky with a glowing handle.

## Motion

Fast, restrained: 150–200ms ease-out for hover/press, 250ms for panels,
`tw-animate-css` for enter/exit. No bouncing, no parallax.
