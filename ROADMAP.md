# Seer Roadmap

This document outlines the planned features and development milestones for Seer.

## Current Status

🟢 = Complete | 🟡 = In Progress | ⚪ = Planned

### v0.1.0 - Foundation ✅

- 🟢 Project setup (Tauri + Vite + TypeScript)
- 🟢 File browser with directory navigation
- 🟢 Basic metadata display using ffprobe
- 🟢 Theme switching (system/light/dark)
- 🟢 Custom title bar with window controls
- 🟢 Dependency checking and onboarding flow
- 🟢 Cross-platform path detection for tools

### v0.2.0 - Advanced File Management ✅

- 🟢 Advanced file browser with context menus
- 🟢 Bulk file selection and actions
- 🟢 File operations (rename, move, copy, delete)
- 🟢 New folder creation
- 🟢 Reveal in Finder/Explorer
- 🟢 Clipboard integration
- 🟢 State management with Zustand
- 🟢 Biome config for formatting/linting

### v0.3.0 - Media Analysis ✅

- 🟢 Detailed codec information display
- 🟢 Stream analysis (video, audio, subtitles, attachments)
- 🟢 Stream metadata viewing (language, title, flags)
- 🟢 Bitrate graphs and statistics
- 🟢 SQLite database for caching and job tracking
- 🟢 Export bitrate charts as PNG (theme-aware)
- 🟢 Export bitrate data as JSON/CSV
- 🟢 File metadata caching with hash-based validation
- 🟢 Automatic cache invalidation on file changes
- ⚪ Media file comparison tool
- ⚪ Duplicate detection

### v0.4.0 - Pruning & Stream Management

- 🟢 Stream removal (unwanted audio/subtitle tracks)
- ⚪ Lossless container conversion (remuxing)
- ⚪ File size optimization suggestions
- ⚪ Bulk stream operations with filters

### v0.5.0 - Metadata Editing

- ⚪ EXIF metadata viewing (images)
- ⚪ EXIF metadata editing via ExifTool
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

The caching system uses SQLite with automatic invalidation:

1. **File Hash Computation** (Rust backend)
   - SHA-256 hash of: file size + mtime + first 8KB + last 8KB
   - Fast computation even for large files

2. **Cache Validation** (TypeScript frontend)
   - Cache key format: `{cache_type}:{file_path}`
   - On cache lookup, current file hash is compared with stored hash
   - Mismatched hashes trigger automatic cache invalidation

3. **Cache Types**
   - `file_metadata` - File info + ffprobe data
   - `bitrate_analysis` - Computed bitrate statistics
   - `media_streams` - Parsed stream information
   - `ffprobe_data` - Raw ffprobe output

## Future Considerations

- Plugin system for extensibility
- Cloud storage integration
- AI-powered content tagging
- Subtitle extraction and conversion
- Thumbnail generation
- Media library management

## Dependencies

### Required External Tools

| Tool | Purpose | Installation |
|------|---------|--------------|
| FFmpeg/FFprobe | Media analysis & encoding | [ffmpeg.org](https://ffmpeg.org) |
| ExifTool | Image metadata | [exiftool.org](https://exiftool.org) |

### Platform Support

| Platform | Status |
|----------|--------|
| macOS | 🟢 Primary development platform |
| Windows | 🟡 Supported, testing in progress |
| Linux | 🟡 Supported, testing in progress |

## Contributing

Contributions are welcome! Please check the issues page for current tasks or suggest new features.