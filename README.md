# Grain Video Editor

Full Rust core + WebAssembly FFmpeg path.

## Architecture

- **Desktop (Tauri + Rust)**: Native FFmpeg via `ffmpeg-next` for export and decode. Hardware acceleration (VideoToolbox, NVENC, VAAPI, AMF, QSV) selectable.
- **Browser**: WebCodecs (existing) or **ffmpeg.wasm** for the same codec/container matrix.
- **Shared DSP**: Audio effects (phase vocoder, WSOLA, vibrato, bitcrush, …) live in Rust (`src-tauri/src/audio_dsp.rs`) and are called from the frontend when running in Tauri; browser keeps the original JS implementations as fallback.
- **Frontend**: Monolithic HTML is being split (`styles.css` extracted; more modules next). `grain-bridge.js` routes Process/Export buttons to Rust or WebCodecs/ffmpeg.wasm.

## Supported export matrix (both native & WASM)

- Containers: mp4, mov, mkv, avi, webm
- Video: H.264, HEVC, AV1, VP9, MJPEG
- Audio: AAC, Opus, FLAC, PCM (48 / 96 / 192 kHz)
- Bitrate control + hardware encode/decode selection

## Next to make it production-ready

1. Uncomment `ffmpeg-next` in `Cargo.toml` and implement the real encode/decode path in `export_video`.
2. Add `@ffmpeg/ffmpeg` to the frontend for the WASM path and call it from `grain-bridge.js` when not in Tauri.
3. Finish the phase-vocoder / WSOLA ports using `rustfft` (or keep calling the proven JS versions via a thin wrapper).
4. Continue splitting `index.html` into modules.

Run desktop: `npm install && npm run dev`  
Browser: serve the `frontend/` folder.
