# Grain NLE — audio · video editor (pure Rust + egui 0.29)

A self-contained Rust non-linear editor that combines the Grain audio pitch &
time lab, a video editor, and a **native plug-in host** in one **unified NLE
workspace** styled as a blend of **Vegas Pro, Filmora, CapCut, and BananaV**
(palettes and layout sampled from the reference screenshots).

Audio plug-ins are supported in **four formats**:

- **VST 2.4** — real `AEffect` hosting through the `vst` crate.
- **VST 3** — real COM component/processor hosting through `vst3-host`.
- **Apple Audio Units** — direct AudioToolbox FFI on macOS (no Xcode/Swift
  bridge required).
- **OpenFX** — the existing bundle scanner/preset host, preserved.

The plug-in browser lives in the left dock's **Plug-ins** tab. Discovered
plug-ins are inserted into an 8-slot chain, expose their real parameters, and
are rendered in slot order inside the Audio FX DSP → WAV export path. The
Grain splash → loading screen and working Marlett viewport controls are
retained.

## Workspace layout (the blend)

```
┌──────────────────────────────────────────────────────────────┐
│  Grain NLE  ⏮ ⏪ ▶ ⏩ ⏭  00:00:00:00  30fps · ▦ Editing  ─ □ ✕ │  ← top bar (Vegas transport + workspace layout)
├────────────┬───────────────────────────────┬──────────────────┤
│ ProjectMedia│                              │ Properties       │
│ Generator  │        Preview (16:9)         │  Basic / Color   │
│ Transitions│   safe-frame + center mark    │  Effects / Distort│
│ Video FX   │   text overlays + PiP         │  Mask & text /    │
│ Color      │   scopes + transform handles  │  Tracks           │
│ Audio FX   │   ─────────────────────────── │  (curves, wheels, │
│ Titles/OFX │   00:00:00:00  frame 0  1920×1080│  HSL, keyframes)  │
│ Plug-ins   │                               │                  │
│ Export     │                               │                  │
├────────────┴───────────────────────────────┴──────────────────┤
│ V1│▓▓ thumbnail strips ▓▓ intro.mp4 ▓▓ scene_a ▓▓     M S      │
│ V2│  ▓ clip moved here ▓                             M S      │  ← many video tracks
│ V3│                                                  M S      │    (+ / − Video Track)
│ Overlay│      picture-in-picture                     M S      │
│ A1│∿∿∿ audio waveform ∿∿∿∿∿∿∿∿∿∿∿∿              M S      │     (white playhead +
│ A2│      ♪ music ∿∿∿∿∿∿                         M S      │      amber keyframes)
│ Text│ T title  T caption                          M S      │
│  ⬌ Split ✕ Delete ◀▶ Group Ungroup +Clip +VideoTrack Zoom │
└──────────────────────────────────────────────────────────────┘
```

- **Top bar** — Grain wordmark + Vegas-style transport (⏮ ⏪ ▶/❚❚ ⏩ ⏭) + a big
  `HH:MM:SS:FF` timecode + fps/duration, a **workspace layout switcher**, and the
  Marlett minimize/maximize/close caption buttons.
- **Left dock** (tabbed, Filmora-style) — **Project Media** (clip library),
  **Media Generator** (procedural sources / insert generator clips),
  **Transitions** (edge fades, wipes, slides, zooms), **Video FX** (effect browser
  grid), **Color Grading** (focused live grade), **Audio FX** (the 11-FX rack),
  **Titles**, **OFX**, **Plug-ins** (VST2 / VST3 / AU browser + 8-slot chain), and
  **Export**. Switching tabs never resizes the panels or restyles the UI.
- **Center** — black preview canvas with safe-frame + center crosshair, text
  overlays (with shadow), a V2 picture-in-picture box that shows the real
  imported poster frame, optional transform handles, selectable histogram /
  luma waveform / RGB parade / vectorscope, click-to-seek, and a slim
  Vegas-style readout strip (timecode · frame · 1920×1080 · 30p · speed).
- **Right dock — Properties** — the six collapsible groups (Basic · Color ·
  Effects · Distort · Mask & text · Tracks) with every slider/select, draggable
  color curves, draggable color wheels, HSL-by-color, mask, and keyframes.
- **Bottom — timeline** — a Premiere/FilmCraft-style NLE timeline driven by a
  dedicated engine (`src/timeline.rs`). It has V1 plus unlimited V2+ video
  tracks, Overlay, A1 master waveform, A2+ audio tracks and Text, each with
  M/S/Lock/Visibility/record-target controls. Tools: **V** Select, **C** Razor,
  **U** Slide, **Y** Slip, **Z** Zoom, **H** Hand. Clips are selected by click,
  Ctrl/Shift multi-select or marquee; drags snap to clip edges, the playhead,
  zero and whole seconds. Trim handles, Ripple Delete / Lift, Duplicate, Group /
  Ungroup (Cmd/Ctrl+G, +Shift to ungroup), markers (**M**), snap toggle (**S**)
  and Alt+arrow frame nudging are all available. Media-bin drops land on the
  hovered lane at the hovered time. The ruler, preview and Audio FX waveform
  scrub the same playhead and mirror seeks into the audio engine.
- **Workspace layouts** — the top-bar **▦** button recalls Editing / Color /
  Audio / Effects / Delivery / Preview presets (panel visibility + sizes + dock
  tab). Resize any panel and "Save Current Layout" persists it; layouts use
  persistent egui panel IDs so a recall applies immediately.

The palette is the blend sampled from the references: dark gray
`#181818/#232323/#2b2b2b`, Vegas 2027 cyan-blue accent `#3bc4e8`, Vegas 17
selection blue `#2f80ed`, amber playhead `#ffb000`, and Vegas-17 clip blocks
(blue video / green audio / amber text / purple overlay / teal music).

## What was ported from `index.html`

**Audio FX rack** (left dock → Audio FX tab — the audio editor, combined in)
- Source panel with click-to-load, file metadata, and a **waveform canvas** with
  a draggable playhead and ▶/❚❚ transport + `MM:SS.SS` timecode.
- All **11 FX tabs** and every control, with the exact live-value formatting
  from the JS:
  - **Pitch shift** — technique (FFT phase vocoder / WSOLA / Rubberband-style),
    semitones (−24..+24), FFT size, formant/timbre, preserve-formants checkbox.
  - **Time stretch** — technique + duration factor (0.40×–2.50×).
  - **Vibrato** — mode a/b (b hides technique), rate, depth.
  - **Speed + pitch** — native rate.
  - **Bitcrush** — bit depth, sample-rate divisor.
  - **Vocoder** — carrier waveform, carrier pitch, bands.
  - **Tremolo** — waveform (sine…smooth noise), rate, depth.
  - **Invert** — polarity-flip note.
  - **Multi-pitch** — technique + the −12…+12 interval checkboxes.
  - **More FX** — Reverse / Echo (delay+feedback) / Distortion (drive), with the
    conditional echo/distortion fields shown/hidden like the HTML.
  - **Studio FX** — all 19 effects, with the rate/depth, decay, fade, mix,
    frequency, and gain field groups shown only for the effects that use them
    (the same visibility rules as the JS `sf-type` change handler).
- The FX note line under the tabs and the **Apply Effect & Render WAV** button
  with a spinner/progress state, status line, and a real
  `download/grain_audio_export.wav` result.

**Video editor** (center preview + right Properties dock + bottom timeline)
- **Preview canvas** that re-grades the **active timeline source** (generated
  demo clip, imported image/video, or test pattern) with real per-pixel DSP:
  brightness, contrast, saturation, hue,
  look (grayscale/sepia/invert), **color curves** (per-channel bezier LUT,
  the same `lutOf` as the JS), **color wheels** (shadows/midtones/highlights,
  luminance-weighted), **HSL by color** (applied only to pixels near the
  selected hue), vignette, film grain, scanlines, RGB split, pixelate,
  posterize, blur, mirror, and mask (ellipse/rect/linear + feather + invert).
  Frames are cached while paused and recomputed only when the playhead, grade,
  or source changes. Text overlays and the V2 picture-in-picture box are
  composited on top at the playhead.
- **Transport** — ▶/❚❚ with `MM:SS.SS / MM:SS.SS` timecode and speed readout,
  plus a **trim bar** with two draggable handles and a playhead.
- **Inspector** — the seven collapsible groups from the HTML
  (Basic · Color · Effects · Distort · Mask & text · Tracks · Output) holding
  every slider and select: speed, transition type/duration, look, mirror,
  brightness/contrast/saturation/hue, color curves (draggable control points +
  reset), color wheels (draggable, double-click resets), HSL by color, blur,
  vignette, grain, scanlines, pixelate, glitch, posterize, RGB split,
  kaleidoscope, camera shake, motion trail, wave h/v, 3D tilt/yaw, cube
  rotation, mask type/center/size/feather/invert, text overlay (text/size/X/Y/
  start/end/color + add + list with remove), overlay (V2) and music (A2)
  add/params, and encoder mode/codec/bitrate.
- **Keyframes** — ◆ Add keyframe / Clear, markers on the timeline ruler, and
  linear interpolation of all 20 keyframable params at the playhead; once any
  keyframe exists, dragging a slider keys the current spot (auto-key, like the
  HTML).
- **Multi-lane timeline** — timecode ruler with markers/keyframes, V1/V2+,
  Overlay, A1/A2+ and Text lanes. Uniform clip blocks show thumbnails/waveforms,
  trim handles, compound badges and selection outlines; tool and snap state live
  in `TimelineUi`, while edits resolve by stable UIDs back into `VideoState`.

**Brand chrome (kept from the earlier shell)**
- **Splash screen** → **loading screen** → workspace (`AppPhase` state machine
  on `stable_dt`, ≈2.6 s splash + ≈1.8 s loading). The Grain logo and splash
  background are generated PNGs embedded via `include_bytes!`.
- **Marlett font** for the window caption buttons — loaded at runtime from
  `assets/marlett.ttf`, `C:\Windows\Fonts\marlett.ttf`, or the Linux font paths,
  with painter-drawn fallbacks when it is absent (see `assets/ABOUT_MARLETT.txt`).

## Engines & what is real

This build ships the **real media / DSP / OFX engines** the browser version only
simulated, plus opt-in "better than ffmpeg" engines:

| Module | Default | What it does |
| --- | --- | --- |
| `src/audio_dsp.rs` | **on** | Real `rustfft` DSP — FFT phase vocoder, WSOLA, formant shift, vocoder, tremolo, multi-pitch, studio FX, Rubberband-style pitch/time. The Audio FX rack's **Render** button runs `process_stereo` on the loaded source and writes a real float32 WAV to `download/grain_audio_export.wav`. |
| `src/audio_plugins.rs` | **on** | Unified VST2 / VST3 / AudioUnit host. Scans standard OS folders and user paths, instantiates plug-ins, reads parameters, and renders them into the DSP export chain. AudioUnits are macOS-only; other formats work wherever their host libraries load. |
| `src/media_engine.rs` | **on** | Built-in pure-Rust **RIFF/WAVE decoder** (PCM u8/i16/i24/i32 + IEEE f32/f64) + `hound` WAV export. `decode_audio_stereo` / `process_and_export_audio`. |
| `src/realtime_av.rs` | **on** | `ContinuousVideoStream` — a single warm `ffmpeg` subprocess streams RGBA frames into the preview as you scrub (reuses the pipe for sequential playback, restarts only on seek/clip-change). The cpal `RealtimeAudioEngine` is on by default and its hardware clock is the A/V master. `--no-default-features` keeps a silent transport stub. |
| `src/ofx_host.rs` | **on** | OpenFX bundle scanner / metadata reader via `libloading` — scans `OFX_PLUGIN_PATH` + `/Library/OFX/Plugins` etc., loads `.ofx.bundle` binaries, enumerates `OfxGetPlugin`/`OfxGetNumberOfPlugins`, and lists named Sapphire / Continuum / Twixtor / Universe presets with params. **Apply** stages a deterministic per-pixel approximation (glow / blur / warp / vignette / sharpen) driven by the plug-in's params; **Bypass** removes it. Full OFX suite/action/clip hosting is not implemented. |
| `src/elastique_cli.rs` | **on** | Headless `elastique` CLI — multi-voice pitch stacking, `--engine elastique\|a17`, `--formants`, `--audio-only`, MKV mux. Run via `cargo run --release -- elastique …`. |

**Opt-in engines** (`cargo build --release --features "…"`):

| Feature | Crate | Notes |
| --- | --- | --- |
| `symphonia` | symphonia 0.6 | pure-Rust mp3 / flac / ogg / vorbis / mp4 decode (replaces built-in WAV for those formats) |
| `lewton` | lewton + ogg | alternative Vorbis-in-Ogg decoder |
| `cpal` | cpal 0.15 | real-time playback + the `RealtimeAudioEngine` master clock (**default**; Linux needs ALSA dev headers) |
| `gstreamer` | gstreamer-rs 0.23 | GStreamer decode + mux (needs libgstreamer-1.0 / `-dev`) |
| `video-rs` | video-rs 0.6 | FFmpeg-backed frame decode (needs libavcodec etc.) |

`media_engine::engine_label()` reports the active engine in the preview toolbar.

### What is now real (this build)

- **Demo project.** A playable 12-second project loads at startup: three BananaV
  generator clips on V1, a title, and a generated **real stereo PCM score** on
  A2. Press <kbd>Space</kbd> to hear it; **Project Media → 🎬 Load Demo Project** restores
  it at any time. The first real import replaces the demo cleanly.
- **Media import + drag-and-drop.** Drop a file anywhere on the window, browse in
  the **Project Media** tab, or paste a path and click **Import**. Audio (`.wav`/`.mp3`/…)
  is decoded into the Audio FX rack and A2; video (`.mp4`/`.mov`/`.mkv`/…) is
  probed with real `ffprobe` metadata, gets a V1 clip, and streams real frames
  through `ContinuousVideoStream` (ffmpeg); images (`.png`/`.jpg`/`.webp`/…)
  become V1 source frames. Media-bin items can be dragged onto preview/timeline.
- **Audible playback + sync.** `cpal` is on by default. VST2/VST3/AU plug-ins,
  gain, pan, mute, and A2 clip mixing run through the shared stereo engine; the
  hardware audio clock drives the video playhead. Scrubbing preview, ruler, or
  Home/End/arrow keys seeks both clocks even while paused. With
  `--no-default-features` the same transport runs as a silent stub.
- **Premiere/FilmCraft timeline.** Add/delete video and audio tracks; drag clips
  in time or between compatible lanes. V1 stays contiguous and reorders by UID;
  V2+/Overlay/A2+/Text carry explicit starts. Select/Razor/Slide/Slip/Zoom/Hand
  tools, snapping, marquee selection, trims, ripple/lift delete, duplicate,
  group/ungroup and markers all edit the same backing clips. Deleting a video
  track moves its clips to V1; deleting an extra audio track moves its clips to
  A2. Hidden video/FX/text lanes are excluded from preview/export; audio lanes
  stay audible. A1 draws real waveform peaks, and preview resolves the topmost
  active video track (including nested compounds).
- **Editing workspaces.** The ▦ top-bar switcher recalls six layouts; panel
  widths/heights are captured from the live UI and persist per layout. Hiding
  panels (Preview layout) gives a clean full-screen monitor.
- **Dock tabs.** Project Media, Media Generator, Transitions, Video FX, Color
  Grading, Audio FX, Titles, OFX, Plug-ins and Export. The Audio FX rack uses the
  same dark theme and fixed panel width as every other tab — no surprise
  "audio editor" resizing or restyling.
- **Export.** **Export frame → PNG** runs the full grade + OFX pass and writes a
  real `download/grain_export_frame.png`; **Render/Bounce Audio → WAV** runs the
  real `rustfft` DSP followed by the plug-in chain and writes
  `download/grain_audio_export.wav` / `grain_audio_bounce.wav`. Video export uses
  the installed `ffmpeg` (H.264 MP4 by default), with a graded PNG fallback.
- **OFX.** Apply stages a per-pixel effect classified from the plug-in identifier
  (Sapphire-glow → glow, defocus → blur, distort → warp, vignette, sharpen) and
  driven by the OFX tab's mix/intensity/threshold/radius sliders; the preview
  recomputes live and caches the frame while paused.

Plug-in rendering is offline/export-time; realtime playback renders the shared
PCM mix plus gain/pan/mute. The per-pixel **color grade, OFX preview effects,
audio DSP, native plug-in processing, WAV/PNG export, video decode and timeline**
are real.

## Build & run

Run these from the project root (the folder containing `Cargo.toml`):

```bash
cargo run --release                               # GUI
cargo run --release -- elastique -6 8 clip.wav    # headless pitch render
cargo build --release --features "symphonia lewton gstreamer video-rs"      # all optional decoders/engines (cpal already default)
cargo build --release --no-default-features                                 # silent transport stub, no audio device
```

Default dependencies:

```toml
eframe = { version = "0.29", default-features = true }
egui   = "0.29"
image  = { version = "0.25", default-features = false, features = ["png", "jpeg", "bmp", "webp", "gif"] }
rustfft = "5"                                      # FFT phase vocoder / WSOLA DSP
libloading = "0.8"                                 # OpenFX + AudioUnit FFI
hound = "3"                                        # WAV read/write
vst = "0.4"                                        # VST2 host
vst3-host = { version = "0.11", default-features = false }  # VST3 host
```

> **Audio Unit note:** AU hosting compiles on every platform but only works on
> macOS, where AudioToolbox is present. VST2/VST3 scanning uses the standard OS
> folders (`VST_PATH` / `VST3_PATH` are also honored) and the **＋ Path** button
> adds extra folders. Scanning loads third-party code, so it is manual rather
> than automatic at startup.

Verified in this workspace:

```text
$ cargo check            # 0 errors, 0 warnings
$ cargo build --release  # 0 errors, 0 warnings  ·  Finished `release` [optimized]
```

> The sandbox that produced this file is headless (no GPU/OpenGL), so the GUI
> was **compile-verified but not rendered here**. Run `cargo run` locally to see
> the splash, the OFX panel, the Audio FX rack (with real DSP export), and the
> video grade in motion.

## Marlett

Marlett is a Windows system font and is **not freely redistributable**, so it is
not bundled. Drop a real `marlett.ttf` (from `C:\Windows\Fonts\marlett.ttf`)
into `assets/` to get the authentic caption glyphs; otherwise the painter draws
the minimize/maximize/close shapes. See `assets/ABOUT_MARLETT.txt`.

## Files

```
vegas-ui-reference/
├── Cargo.toml      # eframe/egui 0.29 + rustfft/hound/vst/vst3-host + opt-in engines
├── Cargo.lock      # reproducible build
├── README.md
├── assets/
│   ├── grain_logo.png   # Grain logo (embedded via include_bytes!)
│   ├── splash_bg.png    # splash/loading background (embedded)
│   └── ABOUT_MARLETT.txt
└── src/
    ├── main.rs          # unified NLE GUI: workspace, import, editing, export, plug-in UI
    ├── timeline.rs      # NLE interaction engine: tools, snap, selection, drag, normalization
    ├── audio_dsp.rs     # real rustfft DSP engine (AudioLabState + process_stereo)
    ├── audio_plugins.rs # VST2 / VST3 / AudioUnit host + format-agnostic FX chain
    ├── media_engine.rs  # built-in WAV decode + hound export; feature-gated decoders
    ├── realtime_av.rs   # ContinuousVideoStream (ffmpeg) + cpal engine (default; optional stub)
    ├── ofx_host.rs      # OpenFX bundle scanner / metadata reader
    ├── bananav_suite.rs # BananaV color suite, generators and interchange exports
    └── elastique_cli.rs # headless multi-voice pitch CLI
```

Programs Used:

Claude AI

Z.ai

Grok AI

Gemini AI

OpenCode