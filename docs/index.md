---
layout: home

hero:
  name: "Seer"
  text: "சீர்"
  tagline: Media file manager for the Mac, with codec detection and stream management
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
    details: Codec, duration and stream details for video, audio, subtitles and attachments. FFmpeg is linked into the app, so nothing shells out.

  - icon: 📊
    title: Bit Rate Analysis
    details: Per-stream bit rate chart and statistics, cached so a file is only analysed again when it changes.

  - icon: 🎯
    title: Lossless Stream Removal
    details: Drop unwanted audio, subtitle and other streams without re-encoding. Seer suggests duplicates and says why; the original goes to the Trash.

  - icon: 🧹
    title: Stream Name Clean-up
    details: Review every video under a folder for stream titles that carry a web address or say nothing about their stream, and fix them in one go.

  - icon: 🏷️
    title: Name Suggestions
    details: Clean "Title (Year)" and "Show S01E02" names worked out offline, with optional suggestions from an AI model using your own API key.

  - icon: 🍎
    title: Native Mac App
    details: Written in Rust with GPUI. Finder's keyboard and selection rules, a menu bar command for everything, light and dark appearance.
---

## What is Seer?

**Seer** (Tamil: சீர்) — to prune, order, uniformity, neatness.

A native Mac app for media file management: inspect codecs and streams, analyse bit rate, strip unwanted streams without re-encoding, and give files clean names.

## Screenshots

<div style="display: flex; gap: 1rem; margin: 2rem 0;">
  <img src="/screenshot-1.webp" alt="Seer Screenshot 1" style="width: 48%; border-radius: 8px;" />
  <img src="/screenshot-2.webp" alt="Seer Screenshot 2" style="width: 48%; border-radius: 8px;" />
</div>

## Tech Stack

- **App**: Rust + [GPUI](https://www.gpui.rs), with AppKit through `objc2`
- **Media processing**: FFmpeg libraries, linked in-process
- **Cache**: SQLite

## Coming Soon

- **Bulk Rename, Filters and Folders from Selection**
- **Duplicate Finder** — Find and remove duplicate files by hash
- **Metadata Editing** — Edit EXIF, ID3, and Matroska tags in bulk
- **Re-encoding** — FFmpeg-based transcoding with presets and hardware acceleration
- **Archive Operations** — Create and extract ZIP, TAR, TAR.GZ archives
