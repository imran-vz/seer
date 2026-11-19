# Seer Roadmap

This document outlines the planned features and development milestones for Seer.

## Current Status

🟢 = Complete | 🟡 = In Progress | ⚪ = Planned

### v0.1.0 - Foundation ✅

- 🟢 Native Mac app in Rust and GPUI, with FFmpeg linked in-process
- 🟢 File browser with directory navigation
- 🟢 Basic metadata display
- 🟢 Theme switching (system/light/dark)
- 🟢 Restored window state

### v0.2.0 - Advanced File Management ✅

- 🟢 Finder-style keyboard navigation and selection
- 🟢 File operations (rename, Move to Trash)
- 🟢 New folder creation
- 🟢 Reveal in Finder
- 🟢 Live reload when a folder changes
- ⚪ Bulk rename
- ⚪ Filters and search
- ⚪ Folders from selection

### v0.3.0 - Media Analysis ✅

- 🟢 Detailed codec information display
- 🟢 Stream analysis (video, audio, subtitles, attachments)
- 🟢 Stream metadata viewing (language, title, flags)
- 🟢 Bitrate graphs and statistics
- 🟢 SQLite cache for analysis results
- ⚪ Export bitrate charts and data
- 🟢 Analysis caching keyed by file fingerprint
- 🟢 Automatic cache invalidation on file changes
- ⚪ Media file comparison tool
- ⚪ Duplicate detection

### v0.4.0 - Pruning & Stream Management

- 🟢 Lossless stream removal (unwanted audio/subtitle tracks)
- 🟢 Suggestions for duplicate streams
- 🟢 Folder-wide stream name clean-up
- 🟢 Clean file name suggestions (offline, or with an AI model)
- ⚪ Lossless container conversion (remuxing)
- ⚪ File size optimization suggestions
- ⚪ Bulk stream operations with filters

### v0.5.0 - Metadata Editing

- ⚪ EXIF metadata viewing (images)
- ⚪ EXIF metadata editing
- ⚪ ID3 tag support for audio files
- ⚪ Matroska/MKV tag editing
- ⚪ Batch metadata operations
- ⚪ Metadata templates/presets

### v0.6.0 - Re-encoding

- ⚪ FFmpeg-based transcoding
- ⚪ Preset encoding profiles
- ⚪ Custom encoding parameters
- ⚪ Progress tracking with ETA
- ⚪ Queue management for batch encoding
- ⚪ Hardware acceleration support (NVENC, VideoToolbox, VAAPI)

### v0.7.0 - Organization

- ⚪ Custom file naming templates
- ⚪ Automatic file organization rules
- ⚪ Watch folders
- ⚪ Integration with media servers (Plex, Jellyfin)

## Technical Implementation Details

### Caching Architecture

Analysis results are cached in SQLite (`crates/seer-core/src/cache.rs`):

1. **Fingerprint** (`crates/seer-core/src/fingerprint.rs`)
   - xxh3 hash of the file's size, modification time and first and last 8 KB
   - Fast even for large files

2. **Validation**
   - Rows are keyed by path and fingerprint; a file that changes gets a new fingerprint, so stale results are never read
   - A schema version is stored with each row; bumping it when a cached type changes shape makes old rows ignored

## Future Considerations

- Plugin system for extensibility
- Cloud storage integration
- AI-powered content tagging
- Subtitle extraction and conversion
- Thumbnail generation
- Media library management

## Dependencies

| Dependency | Purpose | Installation |
|------|---------|--------------|
| FFmpeg libraries | Media analysis and remuxing, linked in-process | `brew install ffmpeg pkgconf` |

### Platform Support

| Platform | Status |
|----------|--------|
| macOS 13+ | 🟢 Supported |

## Contributing

Contributions are welcome! Please check the issues page for current tasks or suggest new features.