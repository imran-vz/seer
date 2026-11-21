# Getting Started

## Quick Start Workflow

1. **Open a folder** — Pick a location in the sidebar, or Go → Go to Folder… (⇧⌘G)
2. **Select a media file** — The inspector shows its details
3. **Analyse bit rate** — Open the Bit Rate tab, or press ⌘B
4. **Remove streams** — Mark unwanted tracks in the Streams tab and press Remove
5. **Clean up names** — Accept a suggested file name, or clean up stream names across a folder (⇧⌘K)

## Browsing

- Use the arrow keys to move, Shift+arrow to extend the selection
- ⌘O or ⌘↓ opens the selection; ⌘↑ goes to the enclosing folder
- ⌘[ and ⌘] go back and forward
- Sort from the View menu; ⇧⌘. shows hidden files
- The list reloads by itself when the folder changes on disk

## Analysing Media Files

Select a file and the inspector shows three tabs:

- **Info**: container, size, duration
- **Streams**: each video, audio, subtitle and attachment stream, with codec, language, title and flags
- **Bit Rate**: a per-stream bit rate chart with statistics

Bit rate results are cached. A file is analysed again only when it changes.

## Removing Streams

1. Select a media file and open the Streams tab
2. Click the streams you want to remove, or start from Seer's suggestions
3. Press Remove

The file is rewritten without re-encoding and the original moves to the Trash.

## Cleaning Up Stream Names

File → Clean Up Stream Names… (⇧⌘K) reviews every video under the current
folder for stream titles that carry a web address or don't describe their
stream, and offers a standard name for each. Apply them in one go.

## Name Suggestions

For a video file, the inspector suggests a clean name such as
`Title (Year).mkv` or `Show S01E02.mkv`, worked out offline from the file name.
You can also ask an AI model (Anthropic, OpenAI or OpenRouter) using your own
API key, which is stored in the Keychain. Only text is sent, never the media.

## Keyboard Shortcuts

### Navigation
| Shortcut | Action |
|---|---|
| ↑ / ↓ | Move selection |
| ⇧↑ / ⇧↓ | Extend selection |
| ⌥↑ / ⌥↓, Home / End | First / last item |
| ⌘O, ⌘↓ | Open |
| ⌘↑ | Enclosing folder |
| ⌘[ / ⌘] | Back / forward |
| ⇧⌘G | Go to folder |
| ⇧⌘H, ⇧⌘D, ⇧⌘O, ⌥⌘L | Home, Desktop, Documents, Downloads |

### Files
| Shortcut | Action |
|---|---|
| Return | Rename |
| ⇧⌘N | New folder |
| ⌘R | Reveal in Finder |
| ⌘⌫ | Move to Trash |
| ⌘C / ⌘V | Copy / paste items |
| ⌥⌘V | Move copied items here |
| ⌘A / Esc | Select all / deselect all |

### Media
| Shortcut | Action |
|---|---|
| ⌘B | Analyse bit rate |
| ⇧⌘K | Clean up stream names |

### View
| Shortcut | Action |
|---|---|
| ⌥⌘S | Toggle sidebar |
| ⌥⌘I | Toggle inspector |
| ⇧⌘. | Toggle hidden files |

## Troubleshooting

### Seer quits on launch

FFmpeg is missing or was upgraded to a new major version. Run
`brew install ffmpeg`, or rebuild Seer from source. See [Installation](/installation).

### Bit rate analysis fails

- Check that the file plays in another player; it may be damaged
- Update FFmpeg with `brew upgrade ffmpeg` and rebuild Seer

## Getting Help

- **GitHub Issues**: [Report bugs](https://github.com/imran-vz/seer/issues)
- **Discussions**: [Ask questions](https://github.com/imran-vz/seer/discussions)

## Next Steps

- Explore [Features](/features) for detailed capabilities
- Check the [Roadmap](https://github.com/imran-vz/seer/blob/main/ROADMAP.md) for upcoming features
