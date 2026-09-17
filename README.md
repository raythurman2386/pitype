# Pitype

Typing practice for the [pi suite](https://github.com/raythurman2386), built
with [GPUI Kit](https://github.com/longbridge/gpui-kit) — a small, native,
theme-following desktop app written for Raspberry Pi 5-class hardware (and
happy on any Linux desktop). Open it, pick a mode, type.

## Features

- **Three modes** — Time (15s / 30s / 60s / 120s sprints against the clock),
  Words (10 / 25 / 50 / 100 word runs that end on the last keystroke), and
  Quote (type a short quote end to end; a bank of 31 quotes with author
  attributions shown under the text and in the menu picker).
- **Results screen** for every run: net WPM, raw WPM, accuracy, consistency,
  errors, and a per-second WPM sparkline. Bests per mode and the last 50
  sessions are tracked locally.
- **Live feedback**: rolling WPM and accuracy while you type; wrong
  keystrokes count as errors and flash the expected character red while the
  cursor stays put; Backspace steps back within the typed text.
- **Aesthetic**: keyboard-first, follows the desktop dark/light mode and
  text scale, and live re-tints from the Omarchy theme palette.

The scoring math and prompt generation are pure Rust with no UI imports, so
WPM, accuracy, consistency, and prompt generation are covered by unit tests
(48 across the suite).

## Install

User-local install from a tagged release (no root, Ed25519-verified,
fail-closed):

```sh
curl -fsSL https://raw.githubusercontent.com/raythurman2386/pitype/main/scripts/netinstall.sh | bash
```

Or build and install from source:

```sh
cargo build --release
./scripts/install.sh
```

Uninstall with `./scripts/uninstall.sh` (session history is kept). The
netinstaller accepts a `--prefix` directory, an optional version argument,
and `--force`; the source install honors `PREFIX=DIR`.

Tagged `v*` releases also build x86_64 + aarch64 tarballs on GitHub Actions
(glibc 2.39+ — e.g. Raspberry Pi OS / Debian 13). Unpack the one for your
architecture and run `./install.sh` inside.

Releases are authenticated with Ed25519 signatures over `checksums.txt`; the
public key is committed as `pitype-signing-key.pub` and pinned in the
installer, which refuses anything it cannot verify.

## Keyboard

| Keys | Action |
|---|---|
| Any printable key / `Enter` | Start a run from the menu |
| `Enter` twice | Next run from results (a single press only arms the prompt) |
| `Ctrl+R` | Restart the current run |
| `Esc` | Back to the menu from results or mid-run |
| `Tab` / `Shift+Tab` | Cycle mode (Time/Words/Quote) · cycle its value |
| `F11` / `Super+F` | Fullscreen · `Ctrl+Q` quit |

While typing, only printable keys and Backspace count. Enter does nothing
mid-run, so it cannot end a timed run early.

## Scoring

- **Net WPM** counts correct characters (5 per word) per minute.
- **Raw WPM** counts every keystroke per minute.
- **Accuracy** is correct keystrokes over all printable keystrokes.
- **Consistency** rewards steady per-second speed over the run.

History (last 50 sessions) and bests are stored in
`~/.local/share/pitype/stats.json`.

## State and theming

Colors follow the desktop theme —
`~/.local/state/pimarchy/current/theme/colors.toml` first, then Omarchy —
re-tinting live on theme switches; dark/light mode follows `gsettings`
`color-scheme`; text follows the desktop text scale (`gsettings`
`text-scaling-factor`). `PITYPE_THEME_DIR` overrides the search for tests.

## Fonts

The iA Writer Mono font is bundled under the SIL Open Font License 1.1; see
`fonts/OFL.txt`. The font is copyright Information Architects Inc. and based
on IBM Plex, copyright IBM Corp.

## Development

```sh
cargo fmt --check          # formatting
cargo clippy --all-targets -- -D warnings
cargo test                 # 48 tests
cargo run --release        # practice
```

CI runs fmt, clippy, and tests on every push; tagged `v*` releases build
x86_64 + aarch64 tarballs (glibc 2.39+) with an install smoke test, and the
netinstall integrity harness can be run locally with
`bash scripts/test-netinstall.sh`.

## License

MIT — see [LICENSE](LICENSE). Bundled fonts: SIL OFL 1.1 (see above).