# Seer

A desktop application for media file management, metadata editing, codec detection, and re-encoding.

**Seer** (Tamil: சீர்) — to prune, order, uniformity, neatness.

## Rewrite in progress (branch `gpui-rewrite`)

Seer is being rewritten as a native Mac app: Rust and [GPUI](https://www.gpui.rs)
instead of Tauri and React, with FFmpeg linked in-process instead of spawning
`ffprobe`. The rules it is built to are in [DESIGN.md](DESIGN.md).

```bash
brew install ffmpeg pkgconf   # FFmpeg libraries and pkg-config, needed to build
make run                      # run the app; `cargo run -p seer -- <folder>` opens a folder
make test                     # core tests (probe, bitrate, stream removal, cache)
make app                      # build target/release/Seer.app
```

| Crate | What it is |
|---|---|
| `crates/seer-core` | The engine, with no UI: listing, probing, bitrate analysis, lossless stream removal, cache. |
| `crates/seer` | The GPUI app: sidebar, file list, inspector. |

Working in the new app: browsing with Finder's keyboard and selection rules,
codec and duration columns, the Info, Streams and Bit Rate inspector tabs,
lossless stream removal (the original goes to the Trash), rename, new folder,
Move to Trash, live reload when a folder changes, and restored window state.

Not ported yet: bulk rename, filters and search, folders from selection, bulk
stream cleanup, tag editing, bit rate export, settings. Until they are, the
Tauri app below stays in the tree (`src/`, `src-tauri/`).

## Features

### ✅ Available Now

- **Advanced File Browser** — Navigate directories with context menus, bulk selection, and file operations (rename, move, copy, delete, new folder, reveal in Finder/Explorer)
- **Media Analysis** — View detailed codec information and stream analysis for video, audio, subtitles, and attachments
- **Bitrate Analysis** — Interactive bitrate graphs with per-stream visualization, statistics, and peak detection
- **Export Options** — Export bitrate charts as PNG (theme-aware), or data as JSON/CSV
- **Stream Management** — Inspect stream metadata (language, title, flags) and remove unwanted streams
- **Metadata Display** — View file metadata using ffprobe
- **SQLite Caching** — Persistent database for job tracking, analysis caching, and file metadata caching with hash-based validation
- **Cross-Platform** — Native experience on macOS, Windows, and Linux with Tauri
- **Theme Support** — System, light, and dark themes

### 🚧 Coming Soon

- **Metadata Editing** — Edit EXIF, ID3, and Matroska tags
- **Re-encoding** — FFmpeg-based transcoding with presets and hardware acceleration
- **Batch Processing** — Queue operations for multiple files
- **File Organization** — Custom naming templates and automatic organization rules

## Tech Stack

- **Frontend**: React + Vite + TypeScript
- **Backend**: Tauri (Rust)
- **Database**: SQLite (via tauri-plugin-sql)
- **State Management**: Zustand
- **Media Processing**: FFmpeg / FFprobe
- **Code Quality**: Biome (formatting & linting)

## Architecture

### Caching System

Seer uses a SQLite database for persistent caching with automatic invalidation:

- **File Metadata Cache** — Caches file metadata and ffprobe data with hash-based validation
- **Bitrate Analysis Cache** — Stores computed bitrate analysis results
- **Job Tracking** — Persistent queue for background processing tasks

Cache validation uses a file hash computed from:
- File size
- Modification time (mtime)
- First and last 8KB of file content (SHA-256)

When a file changes, the hash changes, and cached data is automatically invalidated.

## Getting Started

### Prerequisites

- [Bun](https://bun.sh/) (or Node.js)
- [Rust](https://rustup.rs/)
- [FFmpeg](https://ffmpeg.org/) (required for media analysis)
- [ExifTool](https://exiftool.org/) (optional, for image metadata)

### Installation

```bash
bun install
```

### Development

```bash
bun run tauri dev
```

### Build

```bash
bun run tauri build
```

### Linting & Formatting

```bash
# Check for issues
bun run check

# Fix issues automatically
bun run fix
```

## Project Structure

```
seer/
├── src/                 # Frontend source (TypeScript/React)
│   ├── components/      # UI components
│   ├── stores/          # Zustand state stores
│   ├── lib/             # Utilities and services
│   │   ├── database.ts      # SQLite database operations
│   │   ├── fileMetadataCache.ts  # File metadata caching
│   │   └── useDatabase.ts   # React hooks for database
│   ├── types/           # TypeScript type definitions
│   └── index.css        # Global styles
├── src-tauri/           # Tauri backend (Rust)
│   └── src/
│       ├── lib.rs       # Main entry point
│       ├── database.rs  # SQLite migrations
│       ├── commands/    # Tauri commands
│       ├── bitrate/     # Bitrate analysis module
│       ├── media/       # Media processing
│       └── files/       # File operations
├── docs/                # Documentation website
├── public/              # Static assets
└── index.html           # Entry point
```

## Database Schema

### Cache Table

| Column | Type | Description |
|--------|------|-------------|
| cache_type | TEXT | Type of cached data (file_metadata, bitrate_analysis, etc.) |
| cache_key | TEXT | Unique key (typically `{type}:{file_path}`) |
| file_path | TEXT | Path to the source file |
| file_hash | TEXT | SHA-256 hash for validation |
| file_size | INTEGER | File size in bytes |
| file_modified_at | TEXT | File modification timestamp |
| data | TEXT | JSON-encoded cached data |
| expires_at | TEXT | Cache expiration timestamp |

### Jobs Table

| Column | Type | Description |
|--------|------|-------------|
| job_type | TEXT | Type of job (bitrate_analysis, stream_removal, etc.) |
| status | TEXT | Job status (pending, running, completed, failed, cancelled) |
| file_path | TEXT | Target file path |
| file_hash | TEXT | File hash at job creation |
| params | TEXT | JSON-encoded job parameters |
| result | TEXT | JSON-encoded job result |
| progress | INTEGER | Progress percentage (0-100) |

## Roadmap

See [ROADMAP.md](./ROADMAP.md) for planned features and development milestones.

## License

MIT