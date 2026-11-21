# AGENTS.md

Seer (Tamil சீர்: prune, order, neatness) is a desktop app for looking at,
pruning and tidying media files, written in Rust and drawn with GPUI. Only
macOS is implemented; other platforms exist as data in `platform.rs`.

It is a Cargo workspace: `seer-core` is the engine (no UI, synchronous,
unit-tested) and `seer` is the GPUI app. Each module's `//!` header says what
it owns. `docs/` is a VitePress site and the only JavaScript in the repo
(`bun install`, `bun run docs:dev`).

## Commands

```bash
brew install ffmpeg pkgconf               # FFmpeg libraries, needed to build
cargo run -p seer -- <folder>             # run, optionally opening a folder
cargo test --workspace
cargo clippy --workspace --all-targets && cargo fmt --all
scripts/bundle.sh                         # target/release/Seer.app
```

`SEER_APPEARANCE=light|dark` forces the appearance. In debug builds,
`SEER_PLATFORM=windows|gnome` shows another OS's titlebar, wording and menus
on the Mac.

## How the code is held together

- Sizes, colours and timings come from `theme.rs`, controls from `ui.rs`,
  per-OS differences from `platform.rs`, and user-facing text for sizes,
  durations and bitrates from `seer_core::format`. Call sites don't use
  literals, and views don't branch on the OS.
- Each command is defined in `actions.rs` and listed in its menu table.
- The app calls `seer-core` from `cx.background_spawn`, not from `render`.
  `native.rs` (AppKit) runs on the main thread only.
- FFmpeg is linked in-process through `ffmpeg-the-third`. Only the
  integration test in `seer-core/tests` runs the `ffmpeg` binary, and it
  skips itself when ffmpeg isn't installed.
- GPUI comes only through `gpui-kit`, pinned at `=0.10.0`. Its API changes
  between releases, so bump the pin as a separate, deliberate change.

## Before UI or behaviour work

Read the relevant section of `DESIGN.md`: Principles, Metrics, Components,
Behaviour, Platforms, or "Tells we refuse". When a user-visible feature
changes, update `README.md` and `docs/features.md` too.
