# Seer Roadmap

This document outlines the planned features and development milestones for Seer.

## Current Status

🟢 = Complete | 🟡 = In Progress | ⚪ = Planned

### v0.1.0 - Foundation (Current)

- 🟢 Project setup (Tauri + Vite + TypeScript)
- 🟢 File browser with directory navigation
- 🟢 Basic metadata display using ffprobe
- 🟢 Theme switching (system/light/dark)
- 🟢 Custom title bar with window controls
- 🟢 Dependency checking and onboarding flow
- 🟢 Cross-platform path detection for tools

## Upcoming Releases

### v0.2.0 - Metadata Editing

- ⚪ EXIF metadata viewing (images)
- ⚪ EXIF metadata editing via ExifTool
- ⚪ ID3 tag support for audio files
- ⚪ Matroska/MKV tag editing
- ⚪ Batch metadata operations
- ⚪ Metadata templates/presets

### v0.3.0 - Media Analysis

- ⚪ Detailed codec information display
- ⚪ Stream analysis (video, audio, subtitles)
- ⚪ Bitrate graphs and statistics
- ⚪ Media file comparison tool
- ⚪ Duplicate detection

### v0.4.0 - Re-encoding

- ⚪ FFmpeg-based transcoding
- ⚪ Preset encoding profiles
- ⚪ Custom encoding parameters
- ⚪ Progress tracking with ETA
- ⚪ Queue management for batch encoding
- ⚪ Hardware acceleration support (NVENC, VideoToolbox, VAAPI)

### v0.5.0 - Pruning & Optimization

- ⚪ Stream removal (unwanted audio/subtitle tracks)
- ⚪ Lossless container conversion
- ⚪ File size optimization suggestions
- ⚪ Bulk operations with filters

### v0.6.0 - Organization

- ⚪ Custom file naming templates
- ⚪ Automatic file organization rules
- ⚪ Watch folders
- ⚪ Integration with media servers (Plex, Jellyfin)

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
