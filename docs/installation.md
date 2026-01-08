# Installation

## Prerequisites

Before installing Seer, ensure you have the following dependencies installed:

### Required

- **FFmpeg** — Required for media analysis and processing
  - macOS: `brew install ffmpeg`
  - Windows: Download from [ffmpeg.org](https://ffmpeg.org/download.html)
  - Linux: `apt install ffmpeg` or `yum install ffmpeg`

### Optional

- **ExifTool** — For enhanced image metadata support
  - macOS: `brew install exiftool`
  - Windows: Download from [exiftool.org](https://exiftool.org/)
  - Linux: `apt install libimage-exiftool-perl`

## Download Binaries

### macOS

Pre-built binaries are available for macOS:

👉 **[GitHub Releases](https://github.com/imran-vz/seer/releases)**

**Installation steps:**
1. Download the `.dmg` file
2. Open the `.dmg` and drag Seer to Applications
3. For unsigned apps, run in Terminal:
   ```bash
   xattr -cr /Applications/Seer.app
   ```
4. First launch: Right-click → Open (to bypass Gatekeeper)

### Windows & Linux

Pre-built binaries for Windows and Linux are not currently available. Please [build from source](#build-from-source).

## Build from Source

**Required for Windows and Linux users.** macOS users can also build from source if preferred.

### Prerequisites for Building

- [Bun](https://bun.sh/) or Node.js 18+
- [Rust](https://rustup.rs/) 1.70+
- FFmpeg (see [Prerequisites](#prerequisites) above)

### Build Steps

```bash
# Clone repository
git clone https://github.com/imran-vz/seer.git
cd seer

# Install frontend dependencies
bun install

# Build the application
bun run tauri build
```

The built application will be in `src-tauri/target/release/`.

### Development Mode

To run in development mode with hot reload:

```bash
bun run tauri dev
```

## Verify Installation

After installation, verify that FFmpeg is accessible:

1. Open Seer
2. The app will check for FFmpeg on startup
3. If FFmpeg is missing, you'll see a warning with installation instructions

## Troubleshooting

### FFmpeg Not Found

If Seer can't find FFmpeg:

- Ensure FFmpeg is in your system PATH
- Restart Seer after installing FFmpeg
- On macOS: Restart terminal after `brew install ffmpeg`

### macOS Gatekeeper Warning

If you see "Seer can't be opened because it's from an unidentified developer":

**Option 1: Clear extended attributes (recommended)**
```bash
xattr -cr /Applications/Seer.app
```
Then launch normally.

**Option 2: System Preferences**
1. Go to System Preferences → Security & Privacy
2. Click "Open Anyway" for Seer

**Option 3: Right-click**
Right-click Seer.app → Open

## Next Steps

Once installed, check out the [Getting Started](/getting-started) guide to learn how to use Seer.
