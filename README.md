# Grain Video & Audio Editor — 100% Pure Rust (Zero JavaScript)

A native desktop video and audio editor written **100% in Rust** (`src/main.rs`, `src/audio_dsp.rs`, `src/media_engine.rs`) using `eframe`/`egui` for native macOS Metal/OpenGL rendering, native macOS file dialogs (`rfd`), native Rust FFT/WSOLA audio DSP (`rustfft` + `hound`), and **macOS VideoToolbox** hardware-accelerated encoding/decoding.

**Zero JavaScript, zero HTML/CSS, zero Node.js/npm required.**

## How to Run & Build on macOS

### 1. Prerequisites
```bash
xcode-select --install
brew install ffmpeg
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Run in Development / Release Mode
```bash
cargo run --release
```

### 3. Build macOS `.app` Bundle & `.dmg` Installer
```bash
./build-macos.sh
```
This produces:
- `target/universal-apple-darwin/release/bundle/osx/Grain Video Editor.app` (or `target/release/bundle/osx/Grain Video Editor.app`)
- `target/Grain-Video-Editor-macOS.dmg`


Programs Used:
Claude AI
Grok AI
Gemini AI