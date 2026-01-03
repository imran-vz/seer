# Seer

> **⚠️ Work In Progress** - This project is under active development. Features may be incomplete or subject to change.

A desktop application for media file management, metadata editing, codec detection, and re-encoding.

**Seer** (Tamil: சீர்) - to prune, order, uniformity, neatness.

## Features

### ✅ Available Now

- **Advanced File Browser** - Navigate directories with context menus, bulk selection, and file operations (rename, move, copy, delete, new folder, reveal in Finder/Explorer)
- **Media Analysis** - View detailed codec information and stream analysis for video, audio, subtitles, and attachments
- **Stream Management** - Inspect stream metadata (language, title, flags) and remove unwanted streams
- **Metadata Display** - View file metadata using ffprobe
- **Cross-Platform** - Native experience on macOS, Windows, and Linux with Tauri
- **Theme Support** - System, light, and dark themes
- **Dependency Checking** - Onboarding flow to verify required tools are installed

### 🚧 Coming Soon

- **Metadata Editing** - Edit EXIF, ID3, and Matroska tags
- **Re-encoding** - FFmpeg-based transcoding with presets and hardware acceleration
- **Batch Processing** - Queue operations for multiple files
- **File Organization** - Custom naming templates and automatic organization rules

## Tech Stack

- **Frontend**: React + Vite + TypeScript
- **Backend**: Tauri (Rust)
- **State Management**: Zustand
- **Media Processing**: FFmpeg / FFprobe
- **Code Quality**: Biome (formatting & linting)

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
│   └── index.css        # Global styles
├── src-tauri/           # Tauri backend (Rust)
│   └── src/lib.rs       # Rust commands and logic
├── docs/                # Documentation website
├── public/              # Static assets
└── index.html           # Entry point
```

## Roadmap

See [ROADMAP.md](./ROADMAP.md) for planned features and development milestones.

## License

MIT