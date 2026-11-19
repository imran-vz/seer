# Installation

Seer is a native Mac app. It needs macOS 13 or later.

## Prerequisites

Seer links the FFmpeg libraries, so FFmpeg must be installed:

```bash
brew install ffmpeg
```

## Download

Pre-built builds for Apple Silicon are on

👉 **[GitHub Releases](https://github.com/imran-vz/seer/releases)**

**Installation steps:**
1. Download the `.zip` file and open it
2. Drag Seer to Applications
3. The app is not code-signed, so clear the quarantine flag in Terminal:
   ```bash
   xattr -cr /Applications/Seer.app
   ```
4. First launch: Right-click → Open (to bypass Gatekeeper)

## Build from Source

### Prerequisites for Building

- [Rust](https://rustup.rs/)
- FFmpeg libraries and pkg-config: `brew install ffmpeg pkgconf`

### Build Steps

```bash
# Clone repository
git clone https://github.com/imran-vz/seer.git
cd seer

# Build target/release/Seer.app
make app
```

### Development Mode

```bash
make run                         # or: cargo run -p seer -- <folder>
```

## Troubleshooting

### Seer quits on launch

Seer loads the FFmpeg libraries it was built against. If FFmpeg is missing
or was upgraded to a new major version, reinstall it with
`brew install ffmpeg`, or rebuild Seer from source.

### macOS Gatekeeper Warning

If you see "Seer can't be opened because it's from an unidentified developer":

**Option 1: Clear extended attributes (recommended)**
```bash
xattr -cr /Applications/Seer.app
```
Then launch normally.

**Option 2: System Settings**
1. Go to System Settings → Privacy & Security
2. Click "Open Anyway" for Seer

**Option 3: Right-click**
Right-click Seer.app → Open

## Next Steps

Once installed, check out the [Getting Started](/getting-started) guide to learn how to use Seer.
