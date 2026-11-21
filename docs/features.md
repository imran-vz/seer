# Features

## Available Now ✅

### File Management

**File Browser**
- Finder's keyboard and selection rules (arrow keys, Shift to extend, ⌘A, Esc)
- Back and forward history, enclosing folder, and Go menu locations
- Sidebar with common locations
- Sort by name, size or date modified
- Show or hide hidden files
- Live reload when a folder changes on disk
- Codec and duration columns for media files, probed in the background

**File Operations**
- Rename
- New folder
- Reveal in Finder
- Move to Trash
- Copy (⌘C), Paste (⌘V) and Move Item Here (⌥⌘V) between folders, with progress and cancel in the status bar
- Drag rows onto a folder in the list or a place in the sidebar: a move within one disk or device, a copy across (as in Finder)
- Drop files from Finder onto the list or a sidebar place to copy them there
- Click the transfer in the status bar to see the file in hand, its own progress and the file count
- Right-click a file, the empty space in a folder, or a sidebar place for the commands that apply there

**Android Devices**
- A phone or tablet connected by USB in File Transfer mode appears in the sidebar under Devices
- Browse its storage like any folder; create folders, rename and delete on the device
- Copy or move files and folders in both directions, and between folders on the device
- Nothing is overwritten: an item whose name is taken arrives as "name 2"
- Talks MTP directly over USB in pure Rust; no Android File Transfer, drivers or helper apps

### Media Analysis

**Inspector**
- **Info**: container, size, duration and file details
- **Streams**: every video, audio, subtitle and attachment stream with its codec, language, title and flags
- **Bit Rate**: per-stream bit rate chart and statistics

All media reading goes through the FFmpeg libraries linked into the app.
Nothing shells out to `ffmpeg` or `ffprobe`.

### Stream Management

**Lossless Stream Removal**
- Click the streams you want to remove, then press Remove
- The file is rewritten without re-encoding
- The original moves to the Trash, so nothing is lost
- Runs in the background

**Suggestions**
- Seer proposes streams that duplicate a better one: every video but the best, and for each audio language every track but the best
- Each suggestion says why it was made; it only marks streams, and you still review and press Remove
- Cover art, subtitles and attachments are never suggested

**Stream Name Clean-up**
- Reviews every video under a folder, at any depth
- Flags stream titles that carry a web address or don't describe their stream
- Offers a standard name ("1080p H.264", "AC-3 5.1 384 kbps", "English") and fixes them in one go

### Naming

**Name Suggestions**
- Works out a clean name from a release file name, offline: `Title (Year).ext` or `Show S01E02.ext`, the way Jellyfin and Plex expect
- Optional suggestions from an AI model (Anthropic, OpenAI or OpenRouter) with your own API key
- The key is kept in the macOS Keychain; only the file name and a few facts read from the file are sent, never the media

### Caching

- Analysis results are stored in SQLite
- Each result is keyed by a fingerprint of the file's size, modification time and first and last 8 KB
- A file that changes is analysed again; stale results are never shown

### User Experience

- Light and dark appearance, following the system
- Window size, panes and layout restored on launch
- Every command is in the menu bar

## Not Ported Yet

These were in the earlier version of Seer and are coming back:

- Bulk rename with templates
- Filters and search
- Folders from selection
- Tag editing
- Bit rate export (PNG, JSON, CSV)

## Coming Soon 🚧

### Metadata Editing
- EXIF metadata (images)
- ID3 tags (audio)
- Matroska and MP4 tags
- Cover art

### Re-encoding
- Video and audio codec conversion
- Presets and custom parameters
- Apple VideoToolbox hardware acceleration

### File Organization
- Duplicate detection and removal
- Watch folders
- Naming templates and organisation rules

## Roadmap

See the full [roadmap on GitHub](https://github.com/imran-vz/seer/blob/main/ROADMAP.md) for detailed development plans.
