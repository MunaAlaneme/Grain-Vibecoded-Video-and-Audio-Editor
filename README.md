# Grain Video Editor

Cross-platform (browser + desktop) audio & video editor with advanced DSP, film-grain aesthetics, and professional multi-format export.

## Architecture

- **Frontend**: Existing single-file HTML/JS UI (`frontend/index.html`) — runs in the browser or inside Tauri’s webview.
- **Rust core** (`src-tauri/`): Shared engine for audio processing, frame handling, and export. Exposed to the frontend via Tauri commands.
- **Desktop targets**: macOS, Windows, Linux via Tauri.

## Quick start (desktop)

```bash
cd grain-video-editor
npm install          # already done if you follow this repo
npm run tauri dev    # or: node node_modules/@tauri-apps/cli/tauri.js dev
```

For a pure browser preview of the current UI:

```bash
# any static server, e.g.
npx serve frontend
# or python -m http.server -d frontend
```

## Current status

| Area                        | Status                                      |
|-----------------------------|---------------------------------------------|
| Tauri scaffold              | Done                                        |
| UI extraction               | Done (still monolithic HTML for now)        |
| Rust ↔ frontend API stubs   | Done (`process_audio`, `export_video`, …)   |
| FFmpeg export pipeline      | Planned (requires system FFmpeg + bindings) |
| Audio DSP port (phase vocoder, WSOLA, …) | Planned                          |
| Hardware encode/decode selection | Stub + planned                         |
| WebCodecs fallbacks         | Existing in HTML; needs real-browser tests  |

## Next implementation steps

1. **FFmpeg integration**  
   Add `ffmpeg-next` (or `rusty-ffmpeg`) to `Cargo.toml` and implement `export_video`. Support:
   - Containers: mp4, mov, mkv, avi, webm
   - Video: H.264, HEVC, AV1, VP9, MJPEG
   - Audio: AAC, Opus, FLAC, PCM (48/96/192 kHz)
   - Bitrate control + selectable hardware acceleration (VideoToolbox, NVENC, VAAPI, AMF, QSV, …)

2. **Audio DSP port**  
   Move phase-vocoder, WSOLA, vibrato, bitcrush, etc. from the JS in `frontend/index.html` into Rust (or wrap an existing crate).

3. **Hardware decode selection**  
   Prefer FFmpeg or platform decoders over the plain `<video>` element when running on desktop.

4. **Frontend bridge**  
   Add a small JS module that calls `invoke('process_audio', …)` / `invoke('export_video', …)` and wires the existing UI buttons to the Rust engine (with WebCodecs fallback when not in Tauri).

5. **Split the monolithic HTML**  
   Gradually extract CSS, JS modules, and components for maintainability.

## Requirements for full desktop builds

- Rust toolchain
- Node.js
- System FFmpeg (for the export pipeline)
- Platform-specific WebView (Tauri handles this)

See `src-tauri/src/commands.rs` for the current API surface.
