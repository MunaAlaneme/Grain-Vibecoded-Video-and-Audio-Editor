# Grain Video Editor (Rust + macOS Tauri App)

Desktop video & audio editor powered by a **Rust / Tauri v2** core with **macOS VideoToolbox** hardware acceleration, native **Rust DSP**, and a frame-accurate WebCodecs/muxer fallback.

## What Was Fixed & Added

1. **Export Tab Fixed (`frontend/index.html`)**:
   - Resolved the CSS/JS conflict where the legacy Cut/Edit/Export tab switcher kept `details.vg` (`Output`) hidden (`display: none`) after the CapCut-style shell moved it into the **Export** tab.
   - Moved the primary **Export** button (`#vrun`), progress status (`#vstatus`), and output preview (`#vout`) directly into the Export panel and synced button labels with the selected container/resolution/FPS.
2. **Rust Core & macOS Support (`src-tauri/`)**:
   - Fixed duplicate `mod audio_dsp;` compiler error in `src-tauri/src/commands.rs`.
   - Enabled `withGlobalTauri: true` and macOS bundle metadata (`minimumSystemVersion: "11.0"`, `icon.icns`) in `src-tauri/tauri.conf.json`.
   - Implemented `export_video` with automatic **macOS VideoToolbox** (`h264_videotoolbox` / `hevc_videotoolbox`) hardware encoding & decoding, `process_audio` with `hound`/`rustfft`, and native macOS Save dialogs (`tauri-plugin-dialog`).
   - Updated `frontend/grain-bridge.js` to use `window.__TAURI__.core.invoke` directly without requiring a separate JS bundler.

## How to Build the macOS `.app` and `.dmg`

### Prerequisites (on macOS)
```bash
xcode-select --install
brew install ffmpeg pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Quick Build
Run the included build script from the project root:
```bash
./build-macos.sh
```

Or run the commands manually:
```bash
npm install
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run build -- --target universal-apple-darwin
```

### Output Locations
- **macOS App**: `src-tauri/target/universal-apple-darwin/release/bundle/macos/Grain Video Editor.app`
- **macOS Installer DMG**: `src-tauri/target/universal-apple-darwin/release/bundle/dmg/Grain Video Editor_0.1.0_universal.dmg`

### Development Mode
```bash
npm run dev
```
