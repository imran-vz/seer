# Features

## Available Now ✅

### File Management

**Advanced File Browser**
- Navigate directories with full keyboard and mouse support
- Bulk file selection (Shift/Cmd+Click)
- Context menu operations
- File operations: rename, move, copy, delete
- Create new folders
- Reveal files in system explorer (Finder/Explorer)
- Sort and filter files

**Smart File Filtering**
- Filter by file type (All, Files Only, Folders Only, Media Only)
- Size range filters with presets (>100MB, >1GB, <10MB, 100MB-1GB)
- Date range filtering (modified date)
- Extension-based filtering with dynamic available extensions
- Advanced media filters: resolution presets (4K+, 1080p+, 720p+)
- Video/audio codec filtering (H264, HEVC, AV1, VP9, AAC, AC3, DTS, FLAC, etc.)
- Duration range filtering for media files
- Active filter badges with quick removal
- Filtered results count display

**Bulk Renaming**
- Find & Replace with case sensitivity
- Sequential numbering with padding - supports `{n}`, `{name}`, `{ext}` placeholders
- Case transformation (lowercase, uppercase, titlecase)
- Template system with 9 variables:
  - `{name}` - Original filename (no extension)
  - `{ext}` - File extension
  - `{date}` - Current date (YYYY-MM-DD)
  - `{index}` - File index (0-based)
  - `{counter}` - File counter (1-based)
  - `{parent}` - Parent folder name
  - `{type}` - File type (video/audio/file)
  - `{video_codec}` - Video codec (media files only)
  - `{audio_codec}` - Audio codec (media files only)
- Live preview with conflict detection
- Auto-rename conflicts option (file.mp4 → file (1).mp4)
- Validation errors display

**Folder Creation from Selection**
- Per-file mode: Create individual folder for each file
- Grouped mode with 5 criteria:
  - By Extension: Groups files by file extension
  - By Date Modified: Groups by date (Day/Month/Year granularity)
  - By Media Type: Groups by video/audio/other
  - By Resolution: Groups by resolution (4K, 1080p, 720p, etc.)
  - By Codec: Groups by video codec
- Single folder mode: Move all files into one custom-named folder
- Automatic folder creation and file organization

### Media Analysis

**Codec Detection**
- Detailed codec information for all streams
- Video: codec, resolution, frame rate, bit depth, color space
- Audio: codec, channels, sample rate, bit depth
- Subtitle: format, language metadata
- Attachment: embedded fonts and files

**Stream Inspection**
- Per-stream metadata display
- Language and title information
- Stream flags (default, forced, hearing impaired)
- Duration and bitrate per stream

**FFprobe Integration**
- Complete media file analysis
- Format and container information
- Metadata extraction
- Chapter and attachment detection

### Bitrate Analysis

**Interactive Bitrate Charts**
- Real-time bitrate visualization
- Per-stream bitrate graphs
- Zoom and pan controls
- Peak detection and statistics
- Packet-level or frame-level analysis modes
- Smart sampling for large files (>1GB)

**Export Options**
- Export charts as PNG (theme-aware)
- Export data as JSON
- Export data as CSV
- Include metadata in exports

**Statistics**
- Average, min, max bitrate
- Peak bitrate timestamps
- Bitrate distribution
- File size breakdown by stream

### Stream Management

**Stream Operations**
- View all streams in a file
- Inspect stream metadata
- Remove unwanted streams
- Lossless stream removal (copy codec)
- Background job processing with progress tracking

**Bulk Stream Cleanup**
- Custom stream selection across multiple files
- Expandable file list with per-stream checkboxes
- Preset filters: "All Subtitles", "Non-English Audio", "Commentary", "Cover Art"
- Stream info display with codec, language, and flags
- Job queue integration with progress tracking
- Stream type icons and detailed labels

**Use Cases**
- Remove commentary audio tracks
- Strip subtitle streams
- Remove embedded fonts
- Clean up unnecessary streams
- Batch cleanup of multiple files

### Caching System

**SQLite-Based Caching**
- Persistent database for all cache data
- Background job tracking
- Analysis result caching
- File metadata caching

**Hash-Based Validation**
- SHA-256 hash computation
- Based on: file size + mtime + first/last 8KB
- Automatic cache invalidation on file changes
- No stale data issues

**Cache Types**
- File metadata cache
- Bitrate analysis cache
- FFprobe data cache
- Job status cache

### User Experience

**Theme Support**
- System theme detection
- Light mode
- Dark mode
- Theme persistence

**Background Jobs**
- Concurrent job processing
- Job queue management
- Progress tracking
- Cancel individual or all jobs
- Job history

**Cross-Platform**
- Native experience on macOS, Windows, Linux
- Platform-specific optimizations
- System file manager integration
- Native file dialogs

## Coming Soon 🚧

### Metadata Editing

**Tag Editing**
- EXIF metadata (images)
- ID3 tags (audio)
- Matroska tags (video)
- MP4 metadata
- Batch metadata editing

**Cover Art**
- Embed cover art
- Extract cover art
- Replace cover art
- Auto-fetch from online sources

### Re-encoding

**FFmpeg Transcoding**
- Video codec conversion
- Audio codec conversion
- Preset management
- Custom FFmpeg arguments

**Hardware Acceleration**
- NVIDIA NVENC
- Intel Quick Sync
- AMD VCE
- Apple VideoToolbox

**Quality Control**
- CRF/QP settings
- Bitrate targeting
- Two-pass encoding
- Quality comparison tools

### Advanced Batch Processing

**Job Queue Enhancements**
- Priority management
- Estimated time remaining

**Additional Batch Operations**
- Bulk re-encoding
- Bulk metadata editing

### Advanced File Organization

**Smart Organization**
- Duplicate detection and removal
- Watch folders for automatic processing
- Advanced file sorting rules

**Advanced Templates**
- Regex-based renaming patterns
- Custom metadata-based organization

## Roadmap

See the full [roadmap on GitHub](https://github.com/imran-vz/seer/blob/main/ROADMAP.md) for detailed development plans and timelines.
