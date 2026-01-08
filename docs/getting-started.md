# Getting Started

## Quick Start Workflow

1. **Open a Directory** — Select a folder containing media files
2. **Analyze Files** — Click on any media file to view details
3. **View Bitrate Charts** — Switch to Bitrate tab for visual analysis
4. **Manage Streams** — Remove unwanted audio/subtitle tracks
5. **Export Data** — Save charts or data for external use

## Opening Files

### Select Initial Directory

On first launch, Seer will prompt you to select a directory to browse. This directory will be remembered for future sessions.

### Change Directory

- Use the folder icon in the toolbar
- Navigate using the path breadcrumb
- Use keyboard shortcut `Cmd+O` (macOS) or `Ctrl+O` (Windows/Linux)

### File Browser

- Click on any file to view details
- Use arrow keys to navigate
- Press `Space` to preview
- Right-click for context menu

## Analyzing Media Files

### View File Metadata

1. Select a media file
2. File info panel shows:
   - File size and duration
   - Container format
   - All streams (video, audio, subtitles)
   - Codec information

### Codec Details

For each stream, view:
- **Video**: Resolution, frame rate, codec, bit depth, color space
- **Audio**: Channels, sample rate, codec, bit depth
- **Subtitles**: Format, language, title

## Bitrate Analysis

### Generate Bitrate Chart

1. Select a media file
2. Click "Analyze Bitrate" or switch to Bitrate tab
3. Wait for analysis to complete (cached for future use)
4. Interactive chart displays bitrate over time

### Understanding the Chart

- **Y-axis**: Bitrate in Mbps
- **X-axis**: Time in seconds
- **Multiple lines**: One per stream (video, audio, etc.)
- **Hover**: View exact values at any point
- **Zoom**: Scroll to zoom, drag to pan

### Export Bitrate Data

**Export as PNG**
- Preserves current theme
- High resolution
- Suitable for reports

**Export as JSON**
- Full data points
- Includes metadata
- Programmatic access

**Export as CSV**
- Spreadsheet compatible
- Easy data analysis
- One row per time point

## Stream Management

### View All Streams

1. Select a media file
2. Streams panel shows all tracks
3. Each stream displays:
   - Type (video/audio/subtitle)
   - Codec and language
   - Bitrate and duration

### Remove Streams

1. Select streams to remove (checkbox)
2. Click "Remove Selected Streams"
3. Job runs in background
4. Original file is backed up
5. Progress tracked in Jobs panel

**Note**: Stream removal is lossless (copy codec). No re-encoding occurs.

## File Operations

### Context Menu

Right-click any file for quick actions:
- **Rename**: Change filename
- **Move**: Move to another folder
- **Copy**: Duplicate file
- **Delete**: Move to trash
- **Reveal**: Open in Finder/Explorer

### Bulk Operations

1. Select multiple files:
   - Click + Shift for range
   - Click + Cmd/Ctrl for individual
2. Right-click selection
3. Choose operation
4. Confirm action

### Create Folders

- Click "New Folder" button
- Or right-click → New Folder
- Enter folder name
- Folder created in current directory

## Settings & Preferences

### Theme

- **System**: Follow OS theme
- **Light**: Always light mode
- **Dark**: Always dark mode

### Initial Directory

- Set default startup directory
- Use "Set as Default" in folder picker
- Change anytime in settings

### Cache Management

- View cache size
- Clear specific cache types
- Clear all cached data
- Cache location: App data directory

## Keyboard Shortcuts

### Navigation
- `↑/↓`: Navigate files
- `Enter`: Open/select file
- `Backspace`: Go up one directory
- `Cmd/Ctrl + O`: Open directory

### File Operations
- `Cmd/Ctrl + R`: Rename selected file
- `Cmd/Ctrl + D`: Delete selected file
- `Cmd/Ctrl + N`: New folder

### Panels
- `Cmd/Ctrl + 1`: File browser
- `Cmd/Ctrl + 2`: Metadata panel
- `Cmd/Ctrl + 3`: Bitrate panel
- `Cmd/Ctrl + J`: Jobs panel

### General
- `Cmd/Ctrl + ,`: Open settings
- `Cmd/Ctrl + Q`: Quit (macOS)
- `Alt + F4`: Quit (Windows/Linux)

## Troubleshooting

### FFmpeg Not Found

**Symptom**: "FFmpeg not found" warning on startup

**Solution**:
1. Install FFmpeg (see [Installation](/installation))
2. Ensure FFmpeg is in system PATH
3. Restart Seer

### Bitrate Analysis Fails

**Symptom**: Bitrate analysis errors or hangs

**Possible causes**:
- Corrupted media file
- Unsupported codec
- FFmpeg version too old

**Solution**:
1. Verify file plays in other media players
2. Update FFmpeg to latest version
3. Check Jobs panel for detailed error

### Cache Issues

**Symptom**: Stale data showing for changed files

**Solution**:
1. Cache should auto-invalidate when files change
2. Manually clear cache: Settings → Clear Cache
3. Report issue if problem persists

### Performance Issues

**Symptom**: Slow analysis or UI lag

**Possible causes**:
- Very large media files (>50GB)
- Many files in directory (>10,000)
- Low available RAM

**Solution**:
1. For large files: Seer uses smart sampling
2. For many files: Use subfolders to organize
3. Close other applications to free RAM
4. Clear cache to free disk space

## Getting Help

- **Documentation**: You're reading it!
- **GitHub Issues**: [Report bugs](https://github.com/imran-vz/seer/issues)
- **Discussions**: [Ask questions](https://github.com/imran-vz/seer/discussions)

## Next Steps

- Explore [Features](/features) for detailed capabilities
- Check [Roadmap](https://github.com/imran-vz/seer/blob/main/ROADMAP.md) for upcoming features
- Star the project on [GitHub](https://github.com/imran-vz/seer) if you find it useful!
