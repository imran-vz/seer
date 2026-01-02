# Seer

> **⚠️ Work In Progress** - This project is under active development. Features may be incomplete or subject to change.

A desktop application for media file management, metadata editing, codec detection, and re-encoding.

**Seer** (Tamil: சீர்) - to prune, order, uniformity, neatness.

## Features

- **Metadata Editor** - View and edit file metadata (EXIF, ID3, etc.)
- **Media Re-encoder** - Transcode files to different formats and codecs
- **Audio Codec Detection** - Identify audio codecs in media containers
- **Pruning Tools** - Remove unwanted streams, optimize file sizes

## Tech Stack

- **Frontend**: Vite + TypeScript
- **Backend**: Tauri (Rust)
- **Media Processing**: FFmpeg

## Getting Started

### Prerequisites

- Node.js
- Rust
- FFmpeg - (https://crates.io/crates/ffmpeg-next)
- ExifTool (https://docs.rs/exiftool/latest/exiftool/)

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

## License

MIT
