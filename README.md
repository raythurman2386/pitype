# Pitype

A dead-simple typing practice app built with [GPUI Kit](https://github.com/longbridge/gpui-kit).

Open it, pick a mode, type. Live words-per-minute, live accuracy, and a calm, minimal
interface that follows system dark/light mode and picks up Omarchy theme colors when present.

## Modes

- **Time** — 15s / 30s / 60s / 120s sprints against the clock.
- **Words** — 10 / 25 / 50 / 100 word runs, ends on the last keystroke.
- **Quote** — type a short quote end to end.

Every run ends on a results screen: net WPM, raw WPM, accuracy, consistency, errors, and a
per-second WPM sparkline. Best scores per mode are tracked locally.

## Install

User-local install (binary, icon, launcher). No root:

```sh
./scripts/install.sh
```

That puts `pitype` on `~/.local/bin`, a desktop entry in the app launcher, and the icon in
hicolor. Then:

```sh
pitype
```

Uninstall with `./scripts/uninstall.sh` (session history is kept).

Tagged releases (`v*`) build a Linux x86_64 tarball on GitHub Actions. Unpack it and run
`./install.sh` inside.

## Run from source

```sh
cargo run --release
```

## Shortcuts

- `Enter` starts a run from the menu (or any key, on the menu).
- `Ctrl+R` restarts the current run.
- `Escape` returns to the menu from results or mid-run.
- `F11` or `Super+F` toggles fullscreen.
- `Ctrl+Q` quits.

While typing, only printable keys and Backspace count. Wrong keystrokes are counted as errors
and flash the expected character red; the cursor does not advance. Backspace steps back within
the typed text.

## Scoring

- **Net WPM** counts correct characters (5 per word) per minute.
- **Raw WPM** counts every keystroke per minute.
- **Accuracy** is correct keystrokes over all printable keystrokes.
- **Consistency** rewards steady per-second speed over the run.

History (last 50 sessions) and bests are stored in `~/.local/share/pitype/stats.json`.

## Theme

Colors come from `~/.local/state/omarchy/current/theme/colors.toml` (or
`~/.local/state/pimarchy/current/theme/colors.toml`) when present, and are re-read live when
the theme changes. Dark/light also follows `gsettings` `color-scheme`; text follows the
desktop text size (`gsettings` `text-scaling-factor`). Set `PITYPE_THEME_DIR` to load a theme
from elsewhere.

## Fonts

The iA Writer Mono font is bundled under the SIL Open Font License 1.1; see `fonts/OFL.txt`.
The font is copyright Information Architects Inc. and based on IBM Plex, copyright IBM Corp.