---
layout: home

hero:
  name: "Seer"
  text: "சீர்"
  tagline: Media file manager with bulk operations, codec detection, and stream management
  image:
    src: /seer.svg
    alt: Seer
  actions:
    - theme: brand
      text: Get Started
      link: /getting-started
    - theme: alt
      text: View on GitHub
      link: https://github.com/imran-vz/seer
    - theme: alt
      text: Download
      link: https://github.com/imran-vz/seer/releases

features:
  - icon: 🎬
    title: Media Analysis
    details: View detailed codec information and stream analysis for video, audio, subtitles, and attachments. Powered by FFmpeg/FFprobe.

  - icon: 📊
    title: Bitrate Analysis
    details: Interactive bitrate graphs with per-stream visualization, statistics, peak detection, and smart sampling for large files. Export as PNG, JSON, or CSV.

  - icon: ⚡
    title: Smart Caching
    details: SQLite-based persistent caching with hash-based validation. Automatic cache invalidation when files change.

  - icon: 🔍
    title: Smart Filtering
    details: Filter files by size, date, extension, file type, and media properties (resolution, codec, duration). Apply multiple filters simultaneously.

  - icon: 🎯
    title: Stream Management
    details: Inspect and remove streams with bulk cleanup across multiple files. Preset filters for subtitles, non-English audio, commentary, and cover art.

  - icon: 📁
    title: Bulk File Operations
    details: Advanced file browser with bulk renaming (4 patterns), folder creation (3 modes), smart filtering, and complete file management suite.

  - icon: 🌍
    title: Cross-Platform
    details: Native experience on macOS, Windows, and Linux. Built with Tauri for performance and small bundle size.
---

## What is Seer?

**Seer** (Tamil: சீர்) — to prune, order, uniformity, neatness.

A desktop application for media file management with powerful bulk operations: rename files with templates, create folder structures, clean up media streams, and filter files intelligently. Built for codec analysis and stream management.

## Screenshots

<div style="display: flex; gap: 1rem; margin: 2rem 0;">
  <img src="/screenshot-1.webp" alt="Seer Screenshot 1" style="width: 48%; border-radius: 8px;" />
  <img src="/screenshot-2.webp" alt="Seer Screenshot 2" style="width: 48%; border-radius: 8px;" />
</div>

## Tech Stack

Built with modern technologies for performance and reliability:

- **Frontend**: React + Vite + TypeScript
- **Backend**: Tauri (Rust)
- **Database**: SQLite (via tauri-plugin-sql)
- **State Management**: Zustand
- **Media Processing**: FFmpeg / FFprobe
- **Code Quality**: Biome (formatting & linting)

## Coming Soon

- **Duplicate Finder** — Find and remove duplicate files by hash
- **Metadata Editing** — Edit EXIF, ID3, and Matroska tags in bulk
- **Re-encoding** — FFmpeg-based transcoding with presets and hardware acceleration
- **Archive Operations** — Create and extract ZIP, TAR, TAR.GZ archives
