# Seer

A native Mac app for media file management, metadata editing, codec detection, and re-encoding.

**Seer** (Tamil: சீர்) — to prune, order, uniformity, neatness.

Seer is written in Rust with [GPUI](https://www.gpui.rs). FFmpeg is linked
in-process, so nothing shells out to `ffmpeg` or `ffprobe`. The rules it is
built to are in [DESIGN.md](DESIGN.md).

## Features

### Available now

- **File browser** — Finder's keyboard and selection rules, back and forward history, sidebar locations, sorting, hidden files, and live reload when a folder changes
- **Media columns** — Codec and duration for every media file, probed in the background
- **Inspector** — Info, Streams and Bit Rate tabs for the selected file
- **Bit rate analysis** — Per-stream bit rate chart and statistics
- **Lossless stream removal** — Drop unwanted audio, subtitle and other streams without re-encoding; Seer suggests duplicates and says why, and the original goes to the Trash
- **Stream name clean-up** — Review every video under a folder for stream titles that carry a web address or don't describe their stream, and fix them in one go
- **Name suggestions** — Clean `Title (Year)` and `Show S01E02` names, worked out offline from the file name, with optional suggestions from Anthropic, OpenAI or OpenRouter using your own API key (stored in the Keychain; only text is sent)
- **File operations** — Rename, new folder, Reveal in Finder, Move to Trash
- **Cache** — Analysis results persist in SQLite, keyed by a file fingerprint, so a file that changes is analysed again
- **Restored window state** and light and dark appearance

### Not ported yet

Bulk rename, filters and search, folders from selection, tag editing, and bit rate export.

### Planned

- **Metadata editing** — EXIF, ID3 and Matroska tags
- **Re-encoding** — Transcoding with presets and hardware acceleration
- **File organisation** — Naming templates and automatic organisation rules

## Getting Started

### Prerequisites

- macOS 13 or later
- [Rust](https://rustup.rs/)
- FFmpeg libraries and pkg-config: `brew install ffmpeg pkgconf`

### Development

```bash
make run                      # run the app; `cargo run -p seer -- <folder>` opens a folder
make test                     # core tests (probe, bitrate, stream removal, cache)
make app                      # build target/release/Seer.app
```

Force an appearance with `SEER_APPEARANCE=light` or `SEER_APPEARANCE=dark`.

### Linting & Formatting

```bash
cargo clippy --workspace --all-targets
cargo fmt --all
```

## Project Structure

| Path | What it is |
|---|---|
| `crates/seer-core` | The engine, with no UI: listing, probing, bit rate analysis, lossless stream removal, naming, cache. Synchronous and unit-tested. |
| `crates/seer` | The GPUI app: sidebar, file list, inspector, background jobs. |
| `docs/` | The documentation website (VitePress). |
| `scripts/bundle.sh` | Builds `target/release/Seer.app`. |

## Roadmap

See [ROADMAP.md](./ROADMAP.md) for planned features and development milestones.

## License

MIT
