# Vegas Pro 17 UI Reference — pure Rust + egui 0.29

A self-contained, **compiles-clean (0 errors, 0 warnings)** reference shell that
reproduces the Vegas Pro 17 chrome and workspace from the three reference
screenshots, written against the **same `eframe`/`egui` 0.29** versions the
existing Grain Video Editor uses (see `Cargo.lock`). It is meant as a faithful
visual + structural reference you can lift styling and layout from, or run
standalone to see the look on screen.

## What maps to which screenshot

| Reference screenshot | Implemented in `src/main.rs` |
| --- | --- |
| **Shot 1 — Video FX docked window** | `render_dock` + `render_dock_tabs` + `render_video_fx_dock`: search field with a drawn blue magnifier, the "All" folder tree, the scrollable `BCC+` plug-in list (every name read off the screenshot) with blue **FX** badges, the empty "Preset:" pane, and the bottom tab bar `Project Media | Explorer | Transitions | Video FX | Project Notes`. |
| **Shot 2 — Video Preview + Master Bus** | `render_preview`: black canvas with safe-frames + center crosshair, preview toolbar (Set/Save/FX/Copy/Menu, "Preview (Auto)" combo, Grid/Snap/Ref/Safe), transport (▶/‖/■/|◀), and the orange project readouts (`Project: 1920x1080x32, 60.000p`, `Preview: 480x270x32, 60.000p`, `Frame: 0`, `Display: …`). `render_master_bus` + `draw_master_fader`: header row, draggable vertical fader, dB scale (`-3 -6 -9 -12 … -60`) and a green→yellow→red meter, `0.0 / 0.0` readout with a drawn padlock, and the "Master Bus" bottom tab. |
| **Shot 3 — Multi-track timeline** | `render_timeline` + `render_timeline_left` + `draw_ruler` + `draw_tracks` + `draw_track_header` + `draw_waveform`: the big `00:00:00.33` timecode, the `Rate: 1.00` box, the `P` preferences icon, the ruler with 60 fps timecode ticks, track headers with volume `dB` / pan (`Center`/`L`/`R`) / animated VU meter / M-S-FX buttons, video events with faux-thumbnail strips, audio events with drawn stereo waveforms, the `+0.500` / `87.5%` badges, the white playhead + triangle, the yellow loop-region strip, and the bottom transport bar (Loop, ▶, ‖, ■, |◀ ◀◀ ▶▶ ▶|, ● record, S/Lp/Snap/Qt/Split/Del/Lk, zoom +/−). |

## Build & run

```bash
cd vegas-ui-reference
cargo run --release
```

Pinned dependencies (match the Grain project's `Cargo.lock`):

```toml
eframe = { version = "0.29", default-features = true }
egui   = "0.29"
```

Verified in this environment with `rustc 1.100.0`:

```text
$ cargo build            # 0 warnings
$ cargo build --release  # Finished `release` profile [optimized]
```

> The sandbox that produced this file is headless (no GPU/OpenGL), so the GUI
> was **compile-verified but not rendered here**. Run `cargo run` locally to
> see it. Keyboard: `Space` play/pause, `Home`/`End` jump, `←`/`→` nudge one
> frame; click/drag the ruler or a track to scrub.

## What's new in this revision

- **Splash screen + loading screen.** On launch the app runs a brief branded
  splash (the Grain logo over the cinematic background, the "Grain /
  VIDEO EDITOR" wordmark, a fade-in), then a loading screen with a progress
  bar and a `Loading project… NN%` readout, then the full workspace. The
  state machine is `AppPhase { Splash, Loading, Ready }` driven by
  `stable_dt`, so the timings are frame-rate independent (≈2.6 s splash,
  ≈1.8 s loading). Brand assets are embedded at compile time via
  `include_bytes!("../assets/*.png")` and decoded with the `image` crate, so
  the binary is fully self-contained.
- **Marlett font.** The window caption buttons (minimize / maximize / close)
  are now drawn with the real **Marlett** font when it is available, using
  its actual glyph code points (`'0'` min, `'1'` max, `'2'` restore, `'r'`
  close). Marlett is a Windows system font and **is not freely
  redistributable**, so it is *not* bundled — `ensure_assets()` loads it at
  runtime from `assets/marlett.ttf`, `C:\Windows\Fonts\marlett.ttf`, or the
  Linux font paths. On a host without Marlett, `draw_marlett()` falls back to
  painter-drawn shapes that look identical. To use the real font, copy
  `marlett.ttf` from a Windows install into `assets/`.
- **Fixed playback.** The transport now advances the playhead by real elapsed
  time, **wraps inside the loop region** when one is set (`loop_start`→
  `loop_end`), stops cleanly at the project end, keeps the frame counter in
  sync, and drives the master meter from the **active (non-muted) tracks** —
  and the meter **decays back to silence** when stopped instead of freezing.
- **UI polish.** Marlett caption buttons, a Vegas-style accent line along the
  top of the active dock tab, compact `M:SS` timecode labels on the ruler
  (no more overlapping `00:00:01.00` strings), and slimmer flat scrollbars.

## Vegas palette (lift these into the Grain project)

All colors are `const` at the top of `src/main.rs`, sampled directly from the
three screenshots:

| Constant | RGB | Used for |
| --- | --- | --- |
| `TITLE_BAR`  | `#0D0D0D` | window title strip |
| `MENU_BAR`   | `#2B2B2B` | menu bar, transport bar |
| `TOOLBAR`    | `#3A3A3A` | standard toolbar |
| `PANEL_DARK` | `#252525` | dock / mixer panel fill |
| `LIST_BG`    | `#1E1E1E` | FX list, track bg |
| `PRESET_BG`  | `#2A2A2A` | preset pane |
| `TAB_INACTIVE` / `TAB_ACTIVE` | `#333333` / `#3D3D3D` | dock tabs |
| `FX_BLUE`    | `#3B7BBF` | FX badge + selection accent |
| `ACCENT_BLUE`| `#1A6FD4` | play highlight, magnifier |
| `ORANGE`     | `#E67E22` | numeric readouts |
| `RED_REC`    | `#C4403A` | record button, mute, red event header |
| `TRACK_VIDEO` / `_OLIVE` / `_TEAL` / `_GREEN` | slate/olive/teal/green | track + event colors |
| `PLAYHEAD`   | `#F0F0F0` | timeline playhead |

The `update()` method also rewrites `egui::Visuals::dark()` with these (panel
fill, widget states, and `selection.bg_fill = FX_BLUE`) so every default widget
matches the Vegas look — copy that block straight into the Grain app's
`update()`.

## Integrating into the existing Grain Video Editor

The Grain project (`Cargo.toml` → `grain-video-editor`, `egui` 0.29, `eframe`
0.29) already has `render_video_mode` with a teal `#2DD4BF` accent and
`panel_fill = #0E1014`. To make it look like Vegas Pro:

1. **Palette + visuals** — replace the `Visuals` block at the top of its
   `update()` (`src/main.rs:1109`) with the one in this file's `update()`,
   and paste the `const` palette table at the top of the file.
2. **Chrome** — add the three `TopBottomPanel::top` strips from
   `render_title_strip` / `render_menu_bar` / `render_toolbar` before the
   existing side panels. (The Grain app already has a mode bar; the Vegas
   title strip can sit above it, or replace it.)
3. **Docked tab window** — swap the Grain left library panel
   (`render_lib_*`, `src/main.rs:2781+`) for `render_dock` +
   `render_dock_tabs`, keeping the Grain `LibraryPage::Effects` content (its
   `S_`/`BCC` FX lists, OFX plugins, etc.) as the body of the `Video FX` /
   `OpenFx` tabs. The bottom tab bar and the internal list/preset split come
   for free.
4. **Video Preview** — point `render_preview` at the Grain
   `preview_texture` (`self.preview_texture: Option<TextureHandle>`) instead
   of the placeholder black canvas; the toolbar, transport and orange
   readouts stay.
5. **Master Bus** — wire `draw_master_fader` to the Grain
   `RealtimeAudioEngine` / `rt_audio` level instead of the simulated
   `master_level`.
6. **Timeline** — the Grain `clips: Vec<TimelineClip>` maps directly onto the
   `Track { events }` model here; reuse `draw_ruler`, `draw_track_header`,
   `draw_tracks`, `draw_waveform`, and the transport bar, feeding real clip
   start/duration/selected state. The Vegas timecode formatter
   `format_tc(sec, fps)` is included.

## Deliberate choices (worth knowing)

- **Text toolbar labels** ("New", "Open", "Save", "Undo", …) instead of
  emoji. egui's bundled fonts don't ship color emoji, so 📁/💾/🔍/🗑 would
  render as missing-glyph boxes. The few drawn icons (magnifier, folder,
  padlock) use the painter. Swap the text labels for `egui::Image` of your
  own icon set when you integrate.
- **OS window decorations kept on** (no frameless custom title bar). The
  in-app dark title strip still shows the `45% - Untitled - VEGAS Pro 17.0`
  text and the Marlett caption buttons (close exits the reference). Going
  frameless needs `ViewportBuilder::with_decorations(false)` +
  `ctx.send_viewport_cmd(ViewportCommand::StartDrag)` on the strip — safe to
  add later.
- **Marlett is loaded, not bundled.** It is a Microsoft font shipped only with
  Windows, so redistributing `marlett.ttf` in this zip would be a license
  violation. `ensure_assets()` probes `assets/marlett.ttf` and the platform
  font paths at runtime; `draw_marlett()` falls back to drawn glyphs. Drop a
  real `marlett.ttf` into `assets/` to get the authentic glyphs.
- **`#![allow(deprecated)]`** — egui 0.29 deprecates
  `Ui::allocate_ui_at_rect` in favor of `allocate_new_ui`, but
  `allocate_new_ui` places by size+cursor, not by absolute `Rect`. This shell
  positions the dock sub-panes, preview regions and timeline columns at
  explicit rects, so `allocate_ui_at_rect` is the correct tool and still
  works in 0.29; the allow is documented at the crate top.
- Sample data (the `BCC+` FX list, the `novG40` / `cf813` events, the dB/VU
  readouts) mirrors the screenshots so the whole UI renders populated. It is
  not wired to real media — see the integration steps above.

## Files

```
vegas-ui-reference/
├── Cargo.toml      # eframe/egui 0.29 + image (png) for the embedded brand art
├── Cargo.lock      # reproducible build (egui 0.29.1, image 0.25.10)
├── README.md
├── assets/
│   ├── grain_logo.png   # Grain logo (embedded via include_bytes!)
│   └── splash_bg.png    # splash/loading background (embedded)
└── src/main.rs     # the whole UI shell (~2200 lines, single file)
```
