// Vegas Pro 17 UI reference shell — pure Rust + egui 0.29
//
// Reproduces the chrome and layout shown in the three reference screenshots:
//   shot 1 -> the "Video FX" docked window (search box, "All" folder tree,
//             BCC+ plug-in list with blue FX badges, "Preset:" pane, bottom
//             tab bar: Project Media | Explorer | Transitions | Video FX | Project Notes)
//   shot 2 -> the Video Preview window (black canvas, transport, project info)
//             and the Master Bus mixer (vertical fader + dB meter scale)
//   shot 3 -> the multi-track timeline (big timecode readout, ruler with
//             timecode ticks, track headers with volume/pan/VU meters, event
//             clips + waveforms, playhead, bottom transport toolbar)
//
// This is a self-contained UI shell: it uses representative sample data so the
// whole interface renders and is interactive (scrub the timeline, play/pause,
// switch dock tabs, search FX, toggle tracks). It is meant as a faithful
// visual + structural reference to drop the styling/layout into the existing
// Grain Video Editor (eframe 0.29 / egui 0.29).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// egui 0.29 deprecates `Ui::allocate_ui_at_rect` in favor of `allocate_new_ui`,
// but `allocate_new_ui` places by size+cursor, not by absolute Rect. This shell
// positions dock panes, the preview sub-panels and the timeline columns at
// explicit rects, so `allocate_ui_at_rect` is the correct tool here. The call
// still works in 0.29; we silence the deprecation rather than distort the layout.
#![allow(deprecated)]

use eframe::egui;
use egui::{pos2, vec2, Color32, FontId, Rect, Rounding, Sense, Stroke, Ui, Vec2};

// ===========================================================================
// Vegas Pro 17 palette (sampled from the three reference screenshots)
// ===========================================================================

const TITLE_BAR: Color32 = Color32::from_rgb(13, 13, 13);       // #0D0D0D
const MENU_BAR: Color32 = Color32::from_rgb(43, 43, 43);        // #2B2B2B
const TOOLBAR: Color32 = Color32::from_rgb(58, 58, 58);         // #3A3A3A
const PANEL_DARK: Color32 = Color32::from_rgb(37, 37, 37);      // #252525
const LIST_BG: Color32 = Color32::from_rgb(30, 30, 30);         // #1E1E1E
const PRESET_BG: Color32 = Color32::from_rgb(42, 42, 42);       // #2A2A2A
const TAB_INACTIVE: Color32 = Color32::from_rgb(51, 51, 51);    // #333333
const TAB_ACTIVE: Color32 = Color32::from_rgb(61, 61, 61);      // #3D3D3D
const CANVAS_BLACK: Color32 = Color32::from_rgb(0, 0, 0);       // #000000
const FX_BLUE: Color32 = Color32::from_rgb(59, 123, 191);       // #3B7BBF (FX badge)
const ACCENT_BLUE: Color32 = Color32::from_rgb(26, 111, 212);   // #1A6FD4 (title bar / play highlights)
const ORANGE: Color32 = Color32::from_rgb(230, 126, 34);        // numeric readouts
const RED_REC: Color32 = Color32::from_rgb(196, 64, 58);        // record button / red event header
const TEXT_LIGHT: Color32 = Color32::from_rgb(208, 208, 208);   // #D0D0D0
const TEXT_DIM: Color32 = Color32::from_rgb(139, 146, 161);     // secondary text
const TEXT_WHITE: Color32 = Color32::from_rgb(238, 238, 238);
const TRACK_VIDEO: Color32 = Color32::from_rgb(74, 85, 104);    // slate-blue video track bg
const TRACK_AUDIO_OLIVE: Color32 = Color32::from_rgb(110, 102, 56);
const TRACK_AUDIO_TEAL: Color32 = Color32::from_rgb(40, 104, 116);
const TRACK_AUDIO_GREEN: Color32 = Color32::from_rgb(58, 116, 70);
const EVENT_HEADER_RED: Color32 = Color32::from_rgb(150, 44, 44);
const RULER_BG: Color32 = Color32::from_rgb(45, 45, 45);
const GRID_LINE: Color32 = Color32::from_rgb(58, 58, 58);
const PLAYHEAD: Color32 = Color32::from_rgb(240, 240, 240);
fn loop_region_color() -> Color32 {
    Color32::from_rgba_unmultiplied(220, 190, 70, 60)
}

// ===========================================================================
// Data model (representative sample data matching screenshot 3)
// ===========================================================================

#[derive(Clone, Copy, PartialEq)]
enum TrackKind {
    Video,
    Audio,
}

#[derive(Clone)]
struct TlEvent {
    name: String,
    start_sec: f64,
    dur_sec: f64,
    color: Color32,        // event body color
    header: Color32,       // event header strip color
    waveform: bool,        // audio -> draw waveform; video -> draw thumbnails
    gain: Option<f32>,     // floating gain box like "+0.500"
}

#[derive(Clone)]
struct Track {
    name: String,
    kind: TrackKind,
    bg: Color32,
    volume_db: f32,
    pan: f32,       // -1..1, 0 == Center
    muted: bool,
    solo: bool,
    height: f32,
    events: Vec<TlEvent>,
}

#[derive(Clone, Copy, PartialEq)]
enum DockTab {
    ProjectMedia,
    Explorer,
    Transitions,
    VideoFx,
    ProjectNotes,
}
impl DockTab {
    const ALL: &'static [DockTab] = &[
        DockTab::ProjectMedia,
        DockTab::Explorer,
        DockTab::Transitions,
        DockTab::VideoFx,
        DockTab::ProjectNotes,
    ];
    fn label(self) -> &'static str {
        match self {
            DockTab::ProjectMedia => "Project Media",
            DockTab::Explorer => "Explorer",
            DockTab::Transitions => "Transitions",
            DockTab::VideoFx => "Video FX",
            DockTab::ProjectNotes => "Project Notes",
        }
    }
}

// App start-up phases: a brief branded splash, then a loading screen, then the
// full workspace. Timers are driven by `stable_dt` so they are frame-rate
// independent.
#[derive(Clone, Copy, PartialEq)]
enum AppPhase {
    Splash,
    Loading,
    Ready,
}

// Marlett glyph code points. Marlett is a Windows system font whose glyphs live
// at these ASCII code points (not Unicode symbol blocks). We try to load the
// real font at runtime; if it is unavailable we draw equivalent shapes.
#[derive(Clone, Copy, PartialEq)]
enum MarlettGlyph {
    Minimize, // '0'
    Maximize, // '1'
    #[allow(dead_code)] // Marlett code point '2'; not wired to a button yet
    Restore,  // '2'
    Close,    // 'r'
}

/// The BorisFX Continuum plug-in list exactly as read in screenshot 1.
const BCC_FX_LIST: &[&str] = &[
    "BCC+Ambient Light",
    "BCC+Atmospheric Glow",
    "BCC+Beauty Studio",
    "BCC+Black and White",
    "BCC+Bleach Bypass",
    "BCC+Blur",
    "BCC+Borders",
    "BCC+Camera Shake",
    "BCC+Center Spot",
    "BCC+Channel Blur",
    "BCC+Channel Blur YUV",
    "BCC+Chroma Bands",
    "BCC+Chromatic Aberration",
    "BCC+Color Gradient",
    "BCC+Color Infrared",
    "BCC+Color Paste",
    "BCC+Color Shadow",
    "BCC+Color Spot",
    "BCC+Colorize Gradient",
    "BCC+Colorama",
    "BCC+Compound Blur",
    "BCC+Contrast",
    "BCC+Convert to HDR",
    "BCC+Cross Process",
    "BCC+Day for Night",
    "BCC+Defocus",
    "BCC+Depth of Field",
    "BCC+Deband",
    "BCC+Dielectric",
    "BCC+Diffusion",
    "BCC+Digital Garbage Matte",
    "BCC+Dissolve",
    "BCC+Drop Shadow",
    "BCC+Edge Lighting",
    "BCC+Emboss",
    "BCC+Exposure Blend",
    "BCC+Faux Film",
    "BCC+Film Glow",
    "BCC+Film Grain",
    "BCC+Film Process",
    "BCC+Glow",
    "BCC+Glow Edge",
    "BCC+Gradient",
    "BCC+Halo",
    "BCC+Halation",
    "BCC+Highlights",
    "BCC+Hot Spot",
    "BCC+HSL Curves",
    "BCC+Invert",
    "BCC+Luma Key",
    "BCC+Magic Sharp",
    "BCC+Matte Cleanup",
    "BCC+Mosaic",
    "BCC+Motion Blur",
    "BCC+Neon Edges",
    "BCC+Noise",
    "BCC+Old Film",
    "BCC+Optical Stabilizer",
    "BCC+Pin Art",
    "BCC+Pixelator",
    "BCC+Posterize",
    "BCC+Priority Blur",
    "BCC+Prism",
    "BCC+Pseudo Color",
    "BCC+Radial Blur",
    "BCC+Rays",
    "BCC+Reptile",
    "BCC+Selective Bloom",
    "BCC+Selective Gaussian",
    "BCC+Selective Tint",
    "BCC+Shadows",
    "BCC+Sharpen",
    "BCC+Show Hide Alpha",
    "BCC+Skin Tone",
    "BCC+Sky Designer",
    "BCC+Slide",
    "BCC+Smear",
    "BCC+Snow",
    "BCC+Solarize",
    "BCC+Sparkle",
    "BCC+Stabilize",
    "BCC+Star",
    "BCC+Star Filter",
    "BCC+Tic Tac Toe",
    "BCC+Tile",
    "BCC+Title Studio",
    "BCC+Tone",
    "BCC+Tritone",
    "BCC+Twirl",
    "BCC+Undulate",
    "BCC+Vignette",
    "BCC+Warp",
    "BCC+Wave",
    "BCC+Wedge",
    "BCC+Zonal",
];

// ===========================================================================
// App
// ===========================================================================

struct VegasApp {
    // dock / library state
    dock_tab: DockTab,
    fx_search: String,
    selected_fx: Option<String>,
    show_all_folder: bool,
    notes: String,

    // layout visibility (View menu)
    show_dock: bool,
    show_mixer: bool,
    show_preview: bool,

    // timeline state
    tracks: Vec<Track>,
    tl_zoom: f32,        // pixels per second
    playhead_sec: f64,
    is_playing: bool,
    selected_track: Option<usize>,
    loop_start: f64,
    loop_end: f64,
    snap: bool,

    // mixer / master bus
    master_db: f32,
    master_level: f32,   // 0..1 simulated meter level

    // project info (screenshot 2 readouts)
    project_w: u32,
    project_h: u32,
    fps: f32,
    preview_scale: f32,  // 0.25 -> "Preview: 480x270"
    frame: u64,

    status: String,

    // --- start-up phases + brand assets ---------------------------------
    phase: AppPhase,
    phase_t: f32,        // seconds elapsed in the current phase
    logo_tex: Option<egui::TextureHandle>,
    splash_bg_tex: Option<egui::TextureHandle>,
    marlett_loaded: bool, // true if a real Marlett TTF was registered
}

impl Default for VegasApp {
    fn default() -> Self {
        let tracks = vec![
            Track {
                name: "Video Track 1".into(),
                kind: TrackKind::Video,
                bg: TRACK_VIDEO,
                volume_db: 0.0,
                pan: 0.0,
                muted: false,
                solo: false,
                height: 74.0,
                events: vec![TlEvent {
                    name: "novG40".into(),
                    start_sec: 0.0,
                    dur_sec: 1.7167, // 00:00:01.43 @ 60fps
                    color: Color32::from_rgb(90, 110, 150),
                    header: EVENT_HEADER_RED,
                    waveform: false,
                    gain: None,
                }],
            },
            Track {
                name: "Audio Track 1".into(),
                kind: TrackKind::Audio,
                bg: TRACK_AUDIO_OLIVE,
                volume_db: 0.0,
                pan: 0.0,
                muted: false,
                solo: false,
                height: 60.0,
                events: vec![TlEvent {
                    name: "novG40".into(),
                    start_sec: 0.0,
                    dur_sec: 1.7167,
                    color: Color32::from_rgb(140, 130, 70),
                    header: Color32::from_rgb(120, 110, 55),
                    waveform: true,
                    gain: None,
                }],
            },
            Track {
                name: "Video Track 2".into(),
                kind: TrackKind::Video,
                bg: TRACK_AUDIO_TEAL,
                volume_db: -2.0,
                pan: 0.0,
                muted: false,
                solo: false,
                height: 74.0,
                events: vec![TlEvent {
                    name: "cf813".into(),
                    start_sec: 0.35,
                    dur_sec: 1.4,
                    color: Color32::from_rgb(60, 130, 140),
                    header: Color32::from_rgb(45, 100, 110),
                    waveform: false,
                    gain: None,
                }],
            },
            Track {
                name: "Audio Track 2".into(),
                kind: TrackKind::Audio,
                bg: TRACK_AUDIO_GREEN,
                volume_db: -3.0,
                pan: 0.0,
                muted: false,
                solo: false,
                height: 60.0,
                events: vec![TlEvent {
                    name: "cf813".into(),
                    start_sec: 0.35,
                    dur_sec: 1.4,
                    color: Color32::from_rgb(80, 140, 90),
                    header: Color32::from_rgb(60, 116, 70),
                    waveform: true,
                    gain: Some(0.5),
                }],
            },
            Track {
                name: "Audio Track 3".into(),
                kind: TrackKind::Audio,
                bg: Color32::from_rgb(50, 55, 65),
                volume_db: -6.0,
                pan: 0.2,
                muted: true,
                solo: false,
                height: 52.0,
                events: vec![],
            },
        ];

        Self {
            dock_tab: DockTab::VideoFx,
            fx_search: String::new(),
            selected_fx: Some("BCC+Ambient Light".into()),
            show_all_folder: true,
            notes: String::from("Project Notes\n\n- Rough cut v1\n- Color: BCC+Color Gradient\n- Export 1080p60"),

            show_dock: true,
            show_mixer: true,
            show_preview: true,

            tracks,
            tl_zoom: 520.0,
            playhead_sec: 0.55,    // ~00:00:00.33
            is_playing: false,
            selected_track: Some(0),
            loop_start: 0.0,
            loop_end: 2.0,
            snap: true,

            master_db: 0.0,
            master_level: 0.62,

            project_w: 1920,
            project_h: 1080,
            fps: 60.0,
            preview_scale: 0.25,
            frame: 0,

            status: "Ready — Vegas Pro 17 UI reference (pure Rust + egui)".into(),

            phase: AppPhase::Splash,
            phase_t: 0.0,
            logo_tex: None,
            splash_bg_tex: None,
            marlett_loaded: false,
        }
    }
}

impl VegasApp {
    fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }

    fn total_dur(&self) -> f64 {
        let mut max = 0.0_f64;
        for t in &self.tracks {
            for e in &t.events {
                max = max.max(e.start_sec + e.dur_sec);
            }
        }
        max.max(2.5)
    }

    // One-time-per-session asset init: registers the Marlett font (if it can be
    // found on the host) and uploads the embedded Grain logo + splash background
    // to the GPU as textures. Runs every frame but returns immediately once both
    // textures exist, so the filesystem probes happen only on the first frame.
    fn ensure_assets(&mut self, ctx: &egui::Context) {
        if self.logo_tex.is_some() && self.splash_bg_tex.is_some() {
            return;
        }
        // --- Marlett (Windows system font; not bundled — see README) ---------
        self.marlett_loaded = false;
        let mut fonts = egui::FontDefinitions::default();
        for path in [
            "assets/marlett.ttf",
            "C:\\Windows\\Fonts\\marlett.ttf",
            "/usr/share/fonts/marlett.ttf",
            "/usr/share/fonts/truetype/marlett.ttf",
        ] {
            if let Ok(bytes) = std::fs::read(path) {
                fonts
                    .font_data
                    .insert("Marlett".to_owned(), egui::FontData::from_owned(bytes));
                fonts
                    .families
                    .entry(egui::FontFamily::Name("Marlett".into()))
                    .or_default()
                    .push("Marlett".to_owned());
                self.marlett_loaded = true;
                break;
            }
        }
        ctx.set_fonts(fonts);

        // --- Grain brand textures (embedded at compile time) ---------------
        if self.logo_tex.is_none() {
            self.logo_tex = load_embedded_png(ctx, "grain_logo", include_bytes!("../assets/grain_logo.png"));
        }
        if self.splash_bg_tex.is_none() {
            self.splash_bg_tex = load_embedded_png(ctx, "splash_bg", include_bytes!("../assets/splash_bg.png"));
        }
    }

    // Full-screen branded splash (logo + "Grain" wordmark + tagline) and the
    // subsequent loading screen (progress bar). Shown while phase != Ready.
    fn render_splash_or_loading(&self, ctx: &egui::Context) {
        let screen = ctx.screen_rect();
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE)
                    .inner_margin(egui::Margin::ZERO)
                    .outer_margin(egui::Margin::ZERO),
            )
            .show(ctx, |ui| {
                let p = ui.painter();
                // background image (or flat fallback), then a dark scrim
                if let Some(bg) = &self.splash_bg_tex {
                    p.image(bg.id(), screen, uv, Color32::WHITE);
                } else {
                    p.rect_filled(screen, 0.0, Color32::from_rgb(10, 12, 18));
                }
                p.rect_filled(
                    screen,
                    0.0,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 110),
                );

                let cx = screen.center().x;
                let cy = screen.center().y;

                match self.phase {
                    AppPhase::Splash => {
                        // fade the whole composition in over the first 0.6s
                        let fade = (self.phase_t / 0.6).min(1.0);
                        let alpha = (255.0 * fade) as u8;
                        if let Some(logo) = &self.logo_tex {
                            let lr = Rect::from_center_size(pos2(cx, cy - 30.0), vec2(150.0, 150.0));
                            p.image(
                                logo.id(),
                                lr,
                                uv,
                                Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                            );
                        }
                        p.text(
                            pos2(cx, cy + 72.0),
                            egui::Align2::CENTER_CENTER,
                            "Grain",
                            FontId::proportional(48.0),
                            Color32::from_rgba_unmultiplied(236, 236, 236, alpha),
                        );
                        p.text(
                            pos2(cx, cy + 112.0),
                            egui::Align2::CENTER_CENTER,
                            "VIDEO EDITOR",
                            FontId::proportional(12.0),
                            Color32::from_rgba_unmultiplied(150, 165, 185, alpha),
                        );
                        // subtle "Loading…" hint at the bottom
                        p.text(
                            pos2(cx, screen.bottom() - 28.0),
                            egui::Align2::CENTER_CENTER,
                            "Loading…",
                            FontId::proportional(11.0),
                            Color32::from_rgba_unmultiplied(120, 120, 120, alpha),
                        );
                    }
                    AppPhase::Loading => {
                        p.text(
                            pos2(cx, cy - 34.0),
                            egui::Align2::CENTER_CENTER,
                            "Grain",
                            FontId::proportional(30.0),
                            TEXT_WHITE,
                        );
                        let prog = (self.phase_t / 1.8).min(1.0);
                        let bar_w = 280.0;
                        let bar = Rect::from_min_size(pos2(cx - bar_w / 2.0, cy + 6.0), vec2(bar_w, 6.0));
                        p.rect_filled(bar, 3.0, Color32::from_rgb(38, 38, 38));
                        p.rect_filled(
                            Rect::from_min_size(bar.min, vec2(bar_w * prog, 6.0)),
                            3.0,
                            ACCENT_BLUE,
                        );
                        p.text(
                            pos2(cx, cy + 28.0),
                            egui::Align2::CENTER_CENTER,
                            format!("Loading project… {:.0}%", prog * 100.0),
                            FontId::proportional(11.0),
                            TEXT_DIM,
                        );
                    }
                    AppPhase::Ready => {}
                }
            });
    }

    // Draw a Marlett UI glyph (window min/max/restore/close) into `rect`. Uses
    // the real Marlett font when it was loaded, otherwise draws the equivalent
    // shape with the painter so the chrome looks identical on any host.
    fn draw_marlett(&self, p: &egui::Painter, rect: Rect, glyph: MarlettGlyph, color: Color32) {
        if self.marlett_loaded {
            let ch = match glyph {
                MarlettGlyph::Minimize => '0',
                MarlettGlyph::Maximize => '1',
                MarlettGlyph::Restore => '2',
                MarlettGlyph::Close => 'r',
            };
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                ch.to_string(),
                FontId::new(13.0, egui::FontFamily::Name("Marlett".into())),
                color,
            );
        } else {
            let s = Stroke::new(1.4_f32, color);
            match glyph {
                MarlettGlyph::Minimize => {
                    p.line_segment(
                        [
                            pos2(rect.left() + 5.0, rect.center().y + 3.0),
                            pos2(rect.right() - 5.0, rect.center().y + 3.0),
                        ],
                        s,
                    );
                }
                MarlettGlyph::Maximize => {
                    p.rect_stroke(rect.shrink(5.0), 1.0, s);
                }
                MarlettGlyph::Restore => {
                    let a = Rect::from_min_size(rect.min + vec2(5.0, 3.0), vec2(9.0, 7.0));
                    let b = Rect::from_min_size(rect.min + vec2(8.0, 6.0), vec2(9.0, 7.0));
                    p.rect_stroke(b, 1.0, s);
                    p.line_segment([a.left_top(), a.right_top()], s);
                    p.line_segment([a.left_top(), a.left_bottom()], s);
                }
                MarlettGlyph::Close => {
                    let a = rect.shrink(5.0);
                    p.line_segment([a.left_top(), a.right_bottom()], s);
                    p.line_segment([a.right_top(), a.left_bottom()], s);
                }
            }
        }
    }
}

// Decode an embedded PNG (include_bytes!) into an egui GPU texture.
fn load_embedded_png(
    ctx: &egui::Context,
    name: &str,
    bytes: &[u8],
) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
    Some(ctx.load_texture(name, image, egui::TextureOptions::default()))
}

// ===========================================================================
// eframe::App
// ===========================================================================

impl eframe::App for VegasApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- one-time asset init: Marlett font + Grain brand textures -------
        self.ensure_assets(ctx);

        // --- Vegas dark visuals + blue selection accent --------------------
        let mut v = egui::Visuals::dark();
        v.panel_fill = PANEL_DARK;
        v.window_fill = Color32::from_rgb(34, 34, 34);
        v.faint_bg_color = Color32::from_rgb(40, 40, 40);
        v.extreme_bg_color = LIST_BG;
        v.widgets.noninteractive.bg_fill = Color32::from_rgb(33, 33, 33);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_LIGHT);
        v.widgets.inactive.bg_fill = Color32::from_rgb(48, 48, 48);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_LIGHT);
        v.widgets.hovered.bg_fill = Color32::from_rgb(64, 64, 64);
        v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT_WHITE);
        v.widgets.active.bg_fill = FX_BLUE;
        v.widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT_WHITE);
        v.selection.bg_fill = FX_BLUE;
        v.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(120, 170, 220));
        v.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(20, 20, 20));
        ctx.set_visuals(v);
        // slimmer, flatter scrollbars (Vegas style)
        ctx.style_mut(|s| {
            s.spacing.scroll.floating = false;
            s.spacing.scroll.bar_width = 10.0;
            s.spacing.scroll.bar_inner_margin = 2.0;
            s.spacing.scroll.bar_outer_margin = 0.0;
            s.spacing.scroll.handle_min_length = 18.0;
        });

        // --- start-up phase machine (Splash -> Loading -> Ready) ------------
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.phase_t += dt;
        match self.phase {
            AppPhase::Splash => {
                if self.phase_t >= 2.6 {
                    self.phase = AppPhase::Loading;
                    self.phase_t = 0.0;
                }
            }
            AppPhase::Loading => {
                if self.phase_t >= 1.8 {
                    self.phase = AppPhase::Ready;
                    self.phase_t = 0.0;
                }
            }
            AppPhase::Ready => {}
        }
        if self.phase != AppPhase::Ready {
            self.render_splash_or_loading(ctx);
            ctx.request_repaint(); // keep the phase timer advancing
            return;
        }

        // --- playback clock (fixed) ----------------------------------------
        // Advances the playhead by real elapsed time, wraps inside the loop
        // region when one is set, stops at the project end, and drives the
        // master meter. When stopped, the meter decays back to silence so the
        // mixer does not freeze on a stale reading.
        if self.is_playing {
            self.playhead_sec += dt as f64;
            let total = self.total_dur();
            let looping = self.loop_end > self.loop_start;
            if looping && self.playhead_sec >= self.loop_end {
                self.playhead_sec = self.loop_start;
            } else if self.playhead_sec >= total {
                self.playhead_sec = total;
                self.is_playing = false;
            }
            self.frame = (self.playhead_sec * self.fps as f64) as u64;
            // simulate an output level from the active (non-muted) tracks so
            // the master meter reacts to transport state, not a free sine.
            let mut lvl = 0.0_f32;
            for (i, t) in self.tracks.iter().enumerate() {
                if !t.muted {
                    lvl += 0.18 + 0.12 * (self.playhead_sec as f32 * 4.3 + i as f32).sin().abs();
                }
            }
            self.master_level = lvl.clamp(0.0, 1.0);
            ctx.request_repaint();
        } else {
            // meter decay
            self.master_level = (self.master_level - dt * 1.8).max(0.0);
        }

        // --- keyboard (only when not typing into a text field) -------------
        if !ctx.wants_keyboard_input() {
            let (space, home, end, left, right) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::Space),
                    i.key_pressed(egui::Key::Home),
                    i.key_pressed(egui::Key::End),
                    i.key_pressed(egui::Key::ArrowLeft),
                    i.key_pressed(egui::Key::ArrowRight),
                )
            });
            let total = self.total_dur();
            if space {
                self.is_playing = !self.is_playing;
            }
            if home {
                self.playhead_sec = 0.0;
            }
            if end {
                self.playhead_sec = total;
            }
            if left {
                self.playhead_sec = (self.playhead_sec - 1.0 / self.fps as f64).max(0.0);
            }
            if right {
                self.playhead_sec = (self.playhead_sec + 1.0 / self.fps as f64).min(total);
            }
        }

        // === top stack (order: title, menu, toolbar) =======================
        self.render_title_strip(ctx);
        self.render_menu_bar(ctx);
        self.render_toolbar(ctx);

        // === bottom stack (order matters: transport first -> it sits at the
        //     very bottom edge; timeline second -> it sits ABOVE transport) ==
        self.render_transport_bar(ctx);
        self.render_timeline(ctx);

        // === middle row (side panels + central preview) ====================
        if self.show_dock {
            egui::SidePanel::left("vegas_dock")
                .min_width(320.0)
                .default_width(420.0)
                .resizable(true)
                .show(ctx, |ui| {
                    self.render_dock(ui);
                });
        }
        if self.show_mixer {
            egui::SidePanel::right("vegas_mixer")
                .min_width(120.0)
                .default_width(150.0)
                .resizable(true)
                .show(ctx, |ui| {
                    self.render_master_bus(ui);
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_preview(ui);
        });

        // --- status toast --------------------------------------------------
        if !self.status.is_empty() {
            egui::Area::new(egui::Id::new("vegas_status_toast"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(12.0, 6.0))
                .show(ctx, |ui| {
                    ui.painter().rect_filled(
                        ui.max_rect(),
                        Rounding::same(3.0),
                        Color32::from_rgba_unmultiplied(0, 0, 0, 170),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(&self.status)
                            .color(TEXT_LIGHT)
                            .size(11.0),
                    );
                    ui.add_space(2.0);
                });
        }
    }
}

// ===========================================================================
// Title strip / menu bar / toolbar (screenshot 1 + 2 chrome)
// ===========================================================================

impl VegasApp {
    fn render_title_strip(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("vegas_title")
            .exact_height(24.0)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().rect_filled(rect, 0.0, TITLE_BAR);

                // app icon (blue square + magenta accent) like the VEGAS glyph
                let icon_rect = Rect::from_min_size(
                    pos2(rect.min.x + 8.0, rect.center().y - 7.0),
                    Vec2::new(14.0, 14.0),
                );
                ui.painter().rect_filled(icon_rect, 1.0, ACCENT_BLUE);
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        pos2(icon_rect.center().x - 1.0, icon_rect.min.y + 2.0),
                        Vec2::new(6.0, 6.0),
                    ),
                    1.0,
                    Color32::from_rgb(190, 70, 160),
                );

                ui.painter().text(
                    pos2(rect.min.x + 30.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "45% - Untitled - VEGAS Pro 17.0",
                    FontId::proportional(12.0),
                    TEXT_WHITE,
                );

                // Window caption buttons drawn with Marlett glyphs (the real
                // font when present on the host, otherwise drawn shapes that
                // look identical). Close exits the reference app.
                let p = ui.painter();
                let mut x = rect.max.x;
                for (glyph, col, id) in [
                    (MarlettGlyph::Close, Color32::from_rgb(232, 80, 80), "win_close"),
                    (MarlettGlyph::Maximize, TEXT_LIGHT, "win_max"),
                    (MarlettGlyph::Minimize, TEXT_LIGHT, "win_min"),
                ] {
                    let r = Rect::from_min_size(pos2(x - 16.0, rect.min.y), Vec2::new(16.0, 24.0));
                    let resp = ui.interact(r, egui::Id::new(id), Sense::click());
                    if resp.hovered() {
                        p.rect_filled(r, 0.0, Color32::from_rgb(40, 40, 40));
                    }
                    self.draw_marlett(p, r, glyph, col);
                    if id == "win_close" && resp.clicked() {
                        std::process::exit(0);
                    }
                    x -= 16.0;
                }
            });
    }

    fn render_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("vegas_menu")
            .exact_height(22.0)
            .show(ctx, |ui| {
                ui.painter().rect_filled(ui.max_rect(), 0.0, MENU_BAR);
                ui.spacing_mut().item_spacing = Vec2::new(14.0, 0.0);
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    for &m in &["File", "Edit", "View", "Insert", "Tools", "Options", "Help"] {
                        ui.menu_button(m, |ui| {
                            self.menu_items(ui, m);
                        });
                    }
                });
            });
    }

    fn menu_items(&mut self, ui: &mut Ui, menu: &str) {
        ui.style_mut().spacing.item_spacing = Vec2::new(8.0, 3.0);
        match menu {
            "File" => {
                if ui.button("New Project").clicked() {
                    self.set_status("New project (demo)");
                    ui.close_menu();
                }
                if ui.button("Open...").clicked() {
                    self.set_status("Open (demo)");
                    ui.close_menu();
                }
                if ui.button("Save").clicked() {
                    self.set_status("Saved (demo)");
                    ui.close_menu();
                }
                if ui.button("Save As...").clicked() {
                    self.set_status("Save As (demo)");
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    std::process::exit(0);
                }
            }
            "Edit" => {
                if ui.button("Undo").clicked() {
                    self.set_status("Undo (demo)");
                    ui.close_menu();
                }
                if ui.button("Redo").clicked() {
                    self.set_status("Redo (demo)");
                    ui.close_menu();
                }
                ui.separator();
                ui.add_enabled(false, egui::Button::new("Cut"));
                ui.add_enabled(false, egui::Button::new("Copy"));
                ui.add_enabled(false, egui::Button::new("Paste"));
                if ui.button("Delete").clicked() {
                    self.set_status("Delete selected (demo)");
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Split (S)").clicked() {
                    self.set_status("Split at playhead (demo)");
                    ui.close_menu();
                }
            }
            "View" => {
                ui.checkbox(&mut self.show_dock, "Docked Window");
                ui.checkbox(&mut self.show_preview, "Video Preview");
                ui.checkbox(&mut self.show_mixer, "Master Bus");
                ui.separator();
                if ui.button("Zoom In").clicked() {
                    self.tl_zoom *= 1.25;
                    ui.close_menu();
                }
                if ui.button("Zoom Out").clicked() {
                    self.tl_zoom /= 1.25;
                    ui.close_menu();
                }
            }
            "Insert" => {
                if ui.button("Import Media...").clicked() {
                    self.set_status("Import media (demo)");
                    ui.close_menu();
                }
                if ui.button("Video Track").clicked() {
                    self.set_status("Insert video track (demo)");
                    ui.close_menu();
                }
                if ui.button("Audio Track").clicked() {
                    self.set_status("Insert audio track (demo)");
                    ui.close_menu();
                }
                if ui.button("Text Media").clicked() {
                    self.set_status("Insert text (demo)");
                    ui.close_menu();
                }
            }
            "Tools" => {
                ui.add_enabled(false, egui::Button::new("Video Scopes"));
                ui.add_enabled(false, egui::Button::new("Beep Mapper"));
                ui.add_enabled(false, egui::Button::new("Media Manager"));
            }
            "Options" => {
                if ui.button("Preferences...").clicked() {
                    self.set_status("Preferences (demo)");
                    ui.close_menu();
                }
            }
            "Help" => {
                if ui.button("About Vegas Pro Reference").clicked() {
                    self.set_status("Pure Rust + egui 0.29 reference shell");
                    ui.close_menu();
                }
            }
            _ => {}
        }
    }

    fn render_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("vegas_toolbar")
            .exact_height(32.0)
            .show(ctx, |ui| {
                ui.painter().rect_filled(ui.max_rect(), 0.0, TOOLBAR);
                ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if toolbar_btn(ui, "New", "New project") {
                        self.set_status("New project (demo)");
                    }
                    if toolbar_btn(ui, "Open", "Open project") {
                        self.set_status("Open project (demo)");
                    }
                    if toolbar_btn(ui, "Save", "Save project") {
                        self.set_status("Save project (demo)");
                    }
                    ui.separator();
                    if toolbar_btn(ui, "Undo", "Undo") {
                        self.set_status("Undo (demo)");
                    }
                    if toolbar_btn(ui, "Redo", "Redo") {
                        self.set_status("Redo (demo)");
                    }
                    ui.separator();
                    if toolbar_btn(ui, "Split", "Split at playhead") {
                        self.set_status("Split at playhead (demo)");
                    }
                    if toolbar_btn(ui, "Del", "Delete selected") {
                        self.set_status("Delete selected (demo)");
                    }
                    ui.separator();
                    if toolbar_btn(ui, "Z+", "Zoom in timeline") {
                        self.tl_zoom *= 1.25;
                    }
                    if toolbar_btn(ui, "Z-", "Zoom out timeline") {
                        self.tl_zoom /= 1.25;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(&self.status)
                                .size(11.0)
                                .color(TEXT_DIM),
                        )
                        .on_hover_text("Status");
                    });
                });
            });
    }
}

// Free-function toolbar button (no &mut self) so the caller can run an action
// on self after the call returns, without a self-borrow conflict.
fn toolbar_btn(ui: &mut Ui, label: &str, tip: &str) -> bool {
    let btn = egui::Button::new(egui::RichText::new(label).size(12.0))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .min_size(Vec2::new(30.0, 24.0));
    ui.add(btn).on_hover_text(tip).clicked()
}

// Small drawn icons (avoid relying on emoji/color-glyph fonts).
fn draw_magnifier_icon(ui: &mut Ui, color: Color32) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
    ui.painter().circle_stroke(pos2(r.center().x - 1.0, r.center().y - 1.0), 4.0, Stroke::new(1.4_f32, color));
    ui.painter().line_segment(
        [
            pos2(r.center().x + 2.5, r.center().y + 2.5),
            pos2(r.right() - 1.0, r.bottom() - 1.0),
        ],
        Stroke::new(1.6_f32, color),
    );
}

fn draw_folder_icon(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 13.0), Sense::hover());
    ui.painter().rect_filled(
        Rect::from_min_size(pos2(r.min.x, r.min.y + 3.0), Vec2::new(r.width(), r.height() - 3.0)),
        1.0,
        Color32::from_rgb(220, 180, 60),
    );
    ui.painter().rect_filled(
        Rect::from_min_size(pos2(r.min.x, r.min.y), Vec2::new(r.width() * 0.45, 4.0)),
        1.0,
        Color32::from_rgb(200, 160, 40),
    );
}

fn draw_lock_icon(ui: &mut Ui, color: Color32) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(12.0, 14.0), Sense::hover());
    ui.painter().circle_stroke(pos2(r.center().x, r.min.y + 4.0), 3.0, Stroke::new(1.2_f32, color));
    ui.painter().rect_filled(
        Rect::from_min_size(pos2(r.min.x + 1.0, r.min.y + 4.0), Vec2::new(r.width() - 2.0, 9.0)),
        1.0,
        color,
    );
}

// ===========================================================================
// Left dock: tabbed windows (screenshot 1)
// ===========================================================================

impl VegasApp {
    fn render_dock(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, PANEL_DARK);

        // content fills the area above the bottom tab bar
        let tab_h = 24.0;
        let content_h = (rect.height() - tab_h).max(0.0);
        let content_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), content_h));
        ui.allocate_ui_at_rect(content_rect, |ui| {
            match self.dock_tab {
                DockTab::VideoFx => self.render_video_fx_dock(ui),
                DockTab::ProjectMedia => self.render_project_media(ui),
                DockTab::Explorer => self.render_explorer(ui),
                DockTab::Transitions => self.render_transitions(ui),
                DockTab::ProjectNotes => self.render_project_notes(ui),
            }
        });

        // bottom tab bar: Project Media | Explorer | Transitions | Video FX | Project Notes
        let tab_rect = Rect::from_min_size(
            pos2(rect.min.x, rect.min.y + content_h),
            Vec2::new(rect.width(), tab_h),
        );
        ui.allocate_ui_at_rect(tab_rect, |ui| {
            self.render_dock_tabs(ui);
        });
    }

    fn render_dock_tabs(&mut self, ui: &mut Ui) {
        ui.painter().rect_filled(ui.max_rect(), 0.0, TAB_INACTIVE);
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
        ui.horizontal(|ui| {
            for &t in DockTab::ALL {
                let active = self.dock_tab == t;
                let label = t.label();
                let btn = egui::Button::new(
                    egui::RichText::new(label)
                        .size(11.0)
                        .color(if active { TEXT_WHITE } else { TEXT_DIM }),
                )
                .fill(if active { TAB_ACTIVE } else { TAB_INACTIVE })
                .stroke(Stroke::NONE)
                .min_size(Vec2::new(84.0, 24.0));
                let resp = ui.add(btn);
                // Vegas-style accent line along the top of the active tab
                if active {
                    let r = resp.rect;
                    ui.painter().line_segment(
                        [pos2(r.left(), r.top() + 1.0), pos2(r.right(), r.top() + 1.0)],
                        Stroke::new(2.0_f32, ACCENT_BLUE),
                    );
                }
                if resp.clicked() {
                    self.dock_tab = t;
                }
            }
            // trailing close/minimize glyphs on the active tab (like the ▢ ✕ in shot 1)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                let _ = ui.small_button("✕");
                let _ = ui.small_button("▢");
            });
        });
    }

    // ---- Video FX dock (screenshot 1) -------------------------------------
    fn render_video_fx_dock(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        let list_w = (rect.width() * 0.42).clamp(140.0, 220.0);
        let list_rect = Rect::from_min_size(rect.min, Vec2::new(list_w, rect.height()));
        let preset_rect = Rect::from_min_size(
            pos2(rect.min.x + list_w, rect.min.y),
            Vec2::new(rect.width() - list_w, rect.height()),
        );

        // left: search box + "All" folder + scrollable FX list
        ui.allocate_ui_at_rect(list_rect, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            ui.add_space(4.0);
            // search field with drawn magnifier (blue accent, like shot 1)
            ui.horizontal(|ui| {
                draw_magnifier_icon(ui, ACCENT_BLUE);
                ui.add_sized(
                    Vec2::new(ui.available_width(), 20.0),
                    egui::TextEdit::singleline(&mut self.fx_search)
                        .hint_text("Search plug-ins")
                        .font(FontId::proportional(12.0)),
                );
            });
            // "All" folder tree row
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.show_all_folder, "");
                draw_folder_icon(ui);
                ui.label(egui::RichText::new("All").color(TEXT_LIGHT).size(12.0));
            });
            ui.separator();

            let q = self.fx_search.to_lowercase();
            let filtered: Vec<&str> = BCC_FX_LIST
                .iter()
                .copied()
                .filter(|n| q.is_empty() || n.to_lowercase().contains(&q))
                .collect();

            egui::ScrollArea::vertical()
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                    for fx in &filtered {
                        let (r_rect, resp) =
                            ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::click());
                        let sel = self.selected_fx.as_deref() == Some(*fx);
                        if sel {
                            ui.painter().rect_filled(r_rect, 0.0, TAB_ACTIVE);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r_rect, 0.0, Color32::from_rgb(45, 45, 45));
                        }
                        // blue FX badge
                        let badge = Rect::from_min_size(
                            pos2(r_rect.min.x + 5.0, r_rect.center().y - 7.0),
                            Vec2::new(26.0, 14.0),
                        );
                        ui.painter().rect_filled(badge, 1.0, FX_BLUE);
                        ui.painter().text(
                            badge.center(),
                            egui::Align2::CENTER_CENTER,
                            "FX",
                            FontId::proportional(9.0),
                            TEXT_WHITE,
                        );
                        ui.painter().text(
                            pos2(r_rect.min.x + 36.0, r_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            *fx,
                            FontId::proportional(12.0),
                            TEXT_LIGHT,
                        );
                        if resp.clicked() {
                            self.selected_fx = Some((*fx).to_string());
                        }
                    }
                    if filtered.is_empty() {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("No plug-ins match").color(TEXT_DIM).size(11.0));
                    }
                });
        });

        // right: "Preset:" pane (empty in screenshot 1)
        ui.allocate_ui_at_rect(preset_rect, |ui| {
            ui.painter().rect_filled(ui.max_rect(), 0.0, PRESET_BG);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Preset:")
                        .size(12.0)
                        .color(TEXT_LIGHT)
                        .strong(),
                );
            });
            ui.add_space(6.0);
            if let Some(name) = &self.selected_fx {
                ui.label(egui::RichText::new(format!("Plug-in: {}", name)).color(TEXT_DIM).size(11.0));
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("(no preset selected)")
                        .color(TEXT_DIM)
                        .size(11.0),
                );
                ui.add_space(8.0);
                if ui.add(egui::Button::new("Add to selected event").small()).clicked() {
                    self.set_status(format!("Applied {} (demo)", name));
                }
            } else {
                ui.label(
                    egui::RichText::new("Select a plug-in from the list")
                        .color(TEXT_DIM)
                        .size(11.0),
                );
            }
        });

        // vertical splitter between list and preset
        ui.painter().line_segment(
            [
                pos2(list_rect.right(), rect.min.y + 4.0),
                pos2(list_rect.right(), rect.max.y - 4.0),
            ],
            Stroke::new(1.0_f32, Color32::from_rgb(20, 20, 20)),
        );
    }

    fn render_project_media(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Project Media").strong().color(TEXT_LIGHT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Import").clicked() {
                    self.set_status("Import media (demo)");
                }
            });
        });
        ui.separator();
        ui.add_space(6.0);
        ui.vertical_centered(|ui| {
            draw_folder_icon(ui);
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Media bin is empty").color(TEXT_DIM).size(11.0));
            ui.label(
                egui::RichText::new("Drop video / audio files here or click Import")
                    .color(TEXT_DIM)
                    .size(10.0),
            );
        });
    }

    fn render_explorer(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut String::new()).on_hover_text("Path");
            let _ = ui.small_button("⌃");
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for d in &["~/Movies", "~/Music", "~/Pictures", "/Library/OFX/Plugins"] {
                ui.horizontal(|ui| {
                    draw_folder_icon(ui);
                    ui.label(egui::RichText::new(*d).size(11.0).color(TEXT_LIGHT));
                });
            }
        });
    }

    fn render_transitions(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Transitions").strong().color(TEXT_LIGHT));
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for t in &[
                "Dissolve",
                "Crossfade",
                "Wipe",
                "Slide",
                "Push",
                "Dip to Black",
                "S_DissolveGlow",
                "S_DissolveLensFlare",
                "S_FilmRoll",
                "S_SwishPan",
            ] {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, Color32::from_rgb(45, 45, 45));
                }
                ui.painter().text(
                    pos2(r.min.x + 6.0, r.center().y),
                    egui::Align2::LEFT_CENTER,
                    *t,
                    FontId::proportional(12.0),
                    TEXT_LIGHT,
                );
                if resp.clicked() {
                    self.set_status(format!("Transition: {} (demo)", t));
                }
            }
        });
    }

    fn render_project_notes(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Project Notes").strong().color(TEXT_LIGHT));
        ui.separator();
        ui.add_space(2.0);
        egui::ScrollArea::vertical()
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.notes)
                        .desired_width(f32::MAX)
                        .desired_rows(20)
                        .font(FontId::proportional(12.0)),
                );
            });
    }
}

// ===========================================================================
// Video Preview window (screenshot 2)
// ===========================================================================

impl VegasApp {
    fn render_preview(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(26, 26, 26));

        // --- preview toolbar (gear, save, fx, copy, dropdown, "Preview (Auto)", grid, ...)
        let tb_h = 26.0;
        let tb_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), tb_h));
        ui.painter().rect_filled(tb_rect, 0.0, MENU_BAR);
        ui.allocate_ui_at_rect(tb_rect, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                for (g, tip) in &[
                    ("Set", "Settings"),
                    ("Save", "Save frame"),
                    ("FX", "Plug-ins"),
                    ("Copy", "Copy frame"),
                    ("Menu", "Menu"),
                ] {
                    let btn = egui::Button::new(egui::RichText::new(*g).size(10.0))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE)
                        .min_size(Vec2::new(26.0, 22.0));
                    ui.add(btn).on_hover_text(*tip);
                }
                ui.separator();
                // "Preview (Auto)" dropdown
                egui::ComboBox::from_id_salt("preview_combo")
                    .selected_text("Preview (Auto)")
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.preview_scale, 1.0, "Preview (Full)");
                        ui.selectable_value(&mut self.preview_scale, 0.5, "Preview (Half)");
                        ui.selectable_value(&mut self.preview_scale, 0.25, "Preview (Quarter)");
                    });
                ui.separator();
                for (g, tip) in &[
                    ("Grid", "Grid"),
                    ("Snap", "Snapshot"),
                    ("Ref", "Refresh"),
                    ("Safe", "Safe frames"),
                ] {
                    let btn = egui::Button::new(egui::RichText::new(*g).size(10.0))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE)
                        .min_size(Vec2::new(28.0, 22.0));
                    ui.add(btn).on_hover_text(*tip);
                }
            });
        });

        // --- black canvas (centered) --------------------------------------
        let info_h = 56.0;
        let canvas_rect = Rect::from_min_max(
            pos2(rect.min.x + 8.0, rect.min.y + tb_h + 6.0),
            pos2(rect.max.x - 8.0, rect.max.y - info_h - 6.0),
        );
        ui.painter().rect_filled(canvas_rect, 0.0, CANVAS_BLACK);
        ui.painter().rect_stroke(canvas_rect, 0.0, Stroke::new(1.0_f32, Color32::from_rgb(40, 40, 40)));

        // safe-frames + center crosshair so the black canvas reads as a real preview
        let cx = canvas_rect.center();
        ui.painter().line_segment(
            [pos2(cx.x - 8.0, cx.y), pos2(cx.x + 8.0, cx.y)],
            Stroke::new(1.0_f32, Color32::from_rgb(60, 60, 60)),
        );
        ui.painter().line_segment(
            [pos2(cx.x, cx.y - 8.0), pos2(cx.x, cx.y + 8.0)],
            Stroke::new(1.0_f32, Color32::from_rgb(60, 60, 60)),
        );
        let safe = canvas_rect.shrink2(Vec2::new(canvas_rect.width() * 0.1, canvas_rect.height() * 0.1));
        ui.painter().rect_stroke(safe, 0.0, Stroke::new(0.5_f32, Color32::from_rgb(50, 50, 50)));
        ui.painter().text(
            canvas_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Preview",
            FontId::proportional(22.0),
            Color32::from_rgb(45, 45, 45),
        );

        // --- transport row beneath the canvas ------------------------------
        let trans_h = 28.0;
        let trans_rect = Rect::from_min_max(
            pos2(canvas_rect.min.x, canvas_rect.max.y + 4.0),
            pos2(canvas_rect.max.x, canvas_rect.max.y + 4.0 + trans_h),
        );
        ui.allocate_ui_at_rect(trans_rect, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                let play_lbl = if self.is_playing { "‖" } else { "▶" };
                let play_col = if self.is_playing { ACCENT_BLUE } else { TEXT_LIGHT };
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(play_lbl).size(14.0).color(play_col))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                    )
                    .on_hover_text("Play / Pause (Space)")
                    .clicked()
                {
                    self.is_playing = !self.is_playing;
                }
                if ui
                    .add(egui::Button::new(egui::RichText::new("■").size(12.0)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE))
                    .on_hover_text("Stop")
                    .clicked()
                {
                    self.is_playing = false;
                    self.playhead_sec = 0.0;
                }
                if ui
                    .add(egui::Button::new(egui::RichText::new("|◀").size(11.0)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE))
                    .on_hover_text("Go to start")
                    .clicked()
                {
                    self.playhead_sec = 0.0;
                }
                ui.separator();
                ui.label(
                    egui::RichText::new(format_tc(self.playhead_sec, self.fps))
                        .monospace()
                        .size(12.0)
                        .color(TEXT_LIGHT),
                );
            });
        });

        // --- project info text (orange numeric readouts like shot 2) -------
        let info_rect = Rect::from_min_max(
            pos2(rect.min.x + 8.0, rect.max.y - info_h),
            pos2(rect.max.x - 8.0, rect.max.y),
        );
        ui.allocate_ui_at_rect(info_rect, |ui| {
            ui.add_space(2.0);
            ui.vertical(|ui| {
                info_line(
                    ui,
                    "Project:",
                    &format!("{}x{}x32, {:.3}p", self.project_w, self.project_h, self.fps),
                );
                info_line(
                    ui,
                    "Preview:",
                    &format!(
                        "{}x{}x32, {:.3}p",
                        (self.project_w as f32 * self.preview_scale) as u32,
                        (self.project_h as f32 * self.preview_scale) as u32,
                        self.fps
                    ),
                );
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Frame:").size(10.0).color(TEXT_DIM));
                    ui.label(egui::RichText::new(format!("{}", self.frame)).size(10.0).color(ORANGE));
                });
                let disp_w = (canvas_rect.width().round() as u32).max(1);
                let disp_h = (canvas_rect.height().round() as u32).max(1);
                info_line(ui, "Display:", &format!("{}x{}x32", disp_w, disp_h));
            });
        });
    }
}

fn info_line(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(10.0).color(TEXT_DIM));
        ui.label(egui::RichText::new(value).size(10.0).color(ORANGE));
    });
}

// ===========================================================================
// Master Bus mixer (screenshot 2 right panel)
// ===========================================================================

impl VegasApp {
    fn render_master_bus(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, PANEL_DARK);

        // header row: gear / rewind / speaker / sliders icons
        let head_h = 64.0;
        let head_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), head_h));
        ui.allocate_ui_at_rect(head_rect, |ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                for (g, tip) in &[("Set", "Bus settings"), ("Rst", "Reset"), ("Mon", "Monitor"), ("Mix", "Mixer")] {
                    ui.add(
                        egui::Button::new(egui::RichText::new(*g).size(10.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                    )
                    .on_hover_text(*tip);
                }
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let mut master_on = true;
                ui.checkbox(&mut master_on, "").on_hover_text("Master enable");
                ui.label(egui::RichText::new("Master").size(11.0).color(TEXT_LIGHT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::Button::new("S").small().fill(RED_REC)).on_hover_text("Solo");
                });
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("FX").size(11.0).color(TEXT_LIGHT));
                ui.add_space(6.0);
                ui.add(egui::Button::new("●").small().fill(RED_REC)).on_hover_text("FX enable");
                ui.add(egui::Button::new("$").small().fill(ORANGE)).on_hover_text("Bus FX");
            });
        });

        // fader + dB meter scale area
        let meter_rect = Rect::from_min_max(
            pos2(rect.min.x + 6.0, rect.min.y + head_h + 4.0),
            pos2(rect.max.x - 6.0, rect.max.y - 34.0),
        );
        self.draw_master_fader(ui, meter_rect);

        // bottom readout: padlock + 0.0 / 0.0
        let foot_rect = Rect::from_min_max(
            pos2(rect.min.x + 4.0, rect.max.y - 30.0),
            pos2(rect.max.x - 4.0, rect.max.y - 4.0),
        );
        ui.allocate_ui_at_rect(foot_rect, |ui| {
            ui.horizontal(|ui| {
                draw_lock_icon(ui, TEXT_DIM);
                ui.label(
                    egui::RichText::new(format!("{:>5.1} {:>5.1}", self.master_db, self.master_db))
                        .monospace()
                        .size(10.0)
                        .color(ORANGE),
                );
            });
        });

        // "Master Bus" bottom tab (shot 2)
        let tab_rect = Rect::from_min_max(
            pos2(rect.min.x, rect.max.y - 22.0),
            rect.max,
        );
        ui.painter().rect_filled(tab_rect, 0.0, TAB_INACTIVE);
        let active = Rect::from_min_size(tab_rect.min, Vec2::new(tab_rect.width() * 0.55, 22.0));
        ui.painter().rect_filled(active, 0.0, TAB_ACTIVE);
        ui.painter().text(
            active.center(),
            egui::Align2::CENTER_CENTER,
            "Master Bus",
            FontId::proportional(10.0),
            TEXT_LIGHT,
        );
    }

    fn draw_master_fader(&mut self, ui: &mut Ui, rect: Rect) {
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(24, 24, 24));

        // fader on the left, dB scale + meter on the right
        let fader_w = 22.0;
        let fader_rect = Rect::from_min_size(
            pos2(rect.min.x + 4.0, rect.min.y + 6.0),
            Vec2::new(fader_w, rect.height() - 12.0),
        );
        let meter_rect = Rect::from_min_size(
            pos2(rect.min.x + fader_w + 6.0, rect.min.y + 6.0),
            Vec2::new(rect.width() - fader_w - 18.0, rect.height() - 12.0),
        );

        // fader track + handle (draggable)
        ui.painter().rect_filled(fader_rect, 2.0, Color32::from_rgb(40, 40, 40));
        // map master_db (-60..+12) to 0..1
        let norm = ((self.master_db + 60.0) / 72.0).clamp(0.0, 1.0);
        let handle_y = fader_rect.bottom() - norm * fader_rect.height();
        let handle = Rect::from_center_size(pos2(fader_rect.center().x, handle_y), Vec2::new(20.0, 14.0));
        ui.painter().rect_filled(handle, 2.0, Color32::from_rgb(80, 80, 80));
        ui.painter().rect_stroke(handle, 2.0, Stroke::new(1.0_f32, Color32::from_rgb(120, 120, 120)));
        // little twin-square glyph on the handle
        ui.painter().rect_stroke(
            Rect::from_center_size(handle.center(), Vec2::new(7.0, 7.0)),
            1.0,
            Stroke::new(1.0_f32, TEXT_LIGHT),
        );

        let resp = ui.interact(handle, egui::Id::new("master_fader"), Sense::drag());
        if resp.dragged() {
            let dy = resp.drag_delta().y;
            let dn = -dy / fader_rect.height();
            self.master_db = (self.master_db + dn * 72.0).clamp(-60.0, 12.0);
        }

        // dB scale + colored meter
        let scale_labels = [3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 36, 42, 48, 54, 60];
        let n = scale_labels.len();
        for (i, db) in scale_labels.iter().enumerate() {
            let frac = i as f32 / (n - 1) as f32;
            let y = meter_rect.top() + frac * meter_rect.height();
            ui.painter().line_segment(
                [pos2(meter_rect.left(), y), pos2(meter_rect.left() + 3.0, y)],
                Stroke::new(1.0_f32, TEXT_DIM),
            );
            ui.painter().text(
                pos2(meter_rect.right() + 2.0, y),
                egui::Align2::LEFT_CENTER,
                format!("-{}", db),
                FontId::proportional(8.0),
                TEXT_DIM,
            );
        }

        // meter body with green->yellow->red gradient + current level fill
        let lvl = self.master_level.clamp(0.0, 1.0);
        let mh = meter_rect.height();
        let cols = [
            Color32::from_rgb(40, 200, 80),
            Color32::from_rgb(120, 200, 40),
            Color32::from_rgb(220, 190, 30),
            Color32::from_rgb(230, 120, 30),
            Color32::from_rgb(220, 50, 40),
        ];
        let seg = mh / cols.len() as f32;
        for (i, c) in cols.iter().enumerate() {
            let seg_top = meter_rect.top() + i as f32 * seg;
            let r = Rect::from_min_size(pos2(meter_rect.left(), seg_top), Vec2::new(meter_rect.width(), seg));
            // darken unlit portion
            ui.painter().rect_filled(r, 0.0, Color32::from_rgb(28, 28, 28));
            // lit portion (from bottom up) up to lvl
            let lit_h = ((lvl * mh) - i as f32 * seg).clamp(0.0, seg);
            if lit_h > 0.0 {
                ui.painter().rect_filled(
                    Rect::from_min_size(pos2(r.left(), r.bottom() - lit_h), Vec2::new(r.width(), lit_h)),
                    0.0,
                    *c,
                );
            }
        }
        ui.painter().rect_stroke(meter_rect, 1.0, Stroke::new(1.0_f32, Color32::from_rgb(20, 20, 20)));
    }
}

// ===========================================================================
// Timeline (screenshot 3)
// ===========================================================================

impl VegasApp {
    fn render_timeline(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("vegas_timeline")
            .min_height(330.0)
            .default_height(380.0)
            .resizable(true)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(28, 28, 28));

                let header_w = 200.0;
                let ruler_h = 26.0;
                let left_rect = Rect::from_min_size(rect.min, Vec2::new(header_w, rect.height()));
                let right_rect = Rect::from_min_size(
                    pos2(rect.min.x + header_w, rect.min.y),
                    Vec2::new(rect.width() - header_w, rect.height()),
                );

                // ---- left column: big timecode + "Rate: 1.00" + track headers
                ui.allocate_ui_at_rect(left_rect, |ui| {
                    self.render_timeline_left(ui, ruler_h);
                });

                // ---- right column: ruler + tracks + playhead overlay
                ui.allocate_ui_at_rect(right_rect, |ui| {
                    let panel = ui.max_rect();
                    let ruler_rect = Rect::from_min_size(panel.min, Vec2::new(panel.width(), ruler_h));
                    let tracks_rect = Rect::from_min_max(
                        pos2(panel.min.x, panel.min.y + ruler_h),
                        panel.max,
                    );

                    // ruler
                    ui.allocate_ui_at_rect(ruler_rect, |ui| {
                        self.draw_ruler(ui, ruler_rect);
                    });

                    // tracks
                    ui.allocate_ui_at_rect(tracks_rect, |ui| {
                        self.draw_tracks(ui, tracks_rect);
                    });

                    // loop region strip (thin yellow band at top of tracks)
                    let lx = tracks_rect.left() + (self.loop_start as f32) * self.tl_zoom;
                    let rx = tracks_rect.left() + (self.loop_end as f32) * self.tl_zoom;
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            pos2(lx, ruler_rect.top()),
                            pos2(rx, ruler_rect.top() + 4.0),
                        ),
                        0.0,
                        loop_region_color(),
                    );

                    // playhead overlay (spans ruler + tracks)
                    let px = tracks_rect.left() + (self.playhead_sec as f32) * self.tl_zoom;
                    if px >= tracks_rect.left() && px <= tracks_rect.right() {
                        ui.painter().line_segment(
                            [pos2(px, ruler_rect.top() + 2.0), pos2(px, tracks_rect.bottom())],
                            Stroke::new(1.5_f32, PLAYHEAD),
                        );
                        // playhead triangle at the ruler top
                        let tri = [
                            pos2(px - 4.0, ruler_rect.top() + 1.0),
                            pos2(px + 4.0, ruler_rect.top() + 1.0),
                            pos2(px, ruler_rect.top() + 6.0),
                        ];
                        ui.painter().add(egui::Shape::convex_polygon(
                            tri.to_vec(),
                            PLAYHEAD,
                            Stroke::NONE,
                        ));
                    }

                    // "P" preferences icon, top-right of the timeline
                    let p_icon = Rect::from_min_size(
                        pos2(panel.right() - 18.0, panel.top() + 4.0),
                        Vec2::new(14.0, 14.0),
                    );
                    let p_resp = ui.interact(p_icon, egui::Id::new("tl_prefs"), Sense::click());
                    ui.painter().text(
                        p_icon.center(),
                        egui::Align2::CENTER_CENTER,
                        "P",
                        FontId::proportional(11.0),
                        if p_resp.hovered() { TEXT_WHITE } else { TEXT_DIM },
                    );
                    if p_resp.clicked() {
                        self.set_status("Timeline preferences (demo)");
                    }
                });

                // splitter between headers and tracks
                ui.painter().line_segment(
                    [
                        pos2(rect.min.x + header_w, rect.min.y),
                        pos2(rect.min.x + header_w, rect.max.y),
                    ],
                    Stroke::new(1.0_f32, Color32::from_rgb(18, 18, 18)),
                );
            });
    }

    fn render_timeline_left(&mut self, ui: &mut Ui, ruler_h: f32) {
        let rect = ui.max_rect();

        // big timecode display box (top-left, spans the ruler row)
        let tc_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), ruler_h));
        ui.painter().rect_filled(tc_rect, 0.0, Color32::from_rgb(20, 20, 20));
        ui.painter().text(
            pos2(tc_rect.min.x + 8.0, tc_rect.center().y),
            egui::Align2::LEFT_CENTER,
            format_tc(self.playhead_sec, self.fps),
            FontId::monospace(18.0),
            TEXT_WHITE,
        );
        // "Rate: 1.00" box just under the timecode (bottom-left of left col)
        let rate_rect = Rect::from_min_size(
            pos2(rect.min.x + 4.0, rect.max.y - 16.0),
            Vec2::new(70.0, 12.0),
        );
        ui.painter().rect_filled(rate_rect, 1.0, Color32::from_rgb(40, 40, 40));
        ui.painter().text(
            rate_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Rate: 1.00",
            FontId::proportional(9.0),
            TEXT_DIM,
        );

        // track headers (aligned with the tracks which start below the ruler)
        let headers_rect = Rect::from_min_max(
            pos2(rect.min.x, rect.min.y + ruler_h),
            pos2(rect.max.x, rect.max.y),
        );
        // Snapshot the tracks so we can call &mut self methods inside the loop
        // (self.draw_track_header) without holding an immutable borrow of self.tracks.
        let tracks: Vec<Track> = self.tracks.clone();
        ui.allocate_ui_at_rect(headers_rect, |ui| {
            let mut y = headers_rect.min.y;
            for (idx, track) in tracks.iter().enumerate() {
                let h_rect = Rect::from_min_size(
                    pos2(headers_rect.min.x, y),
                    Vec2::new(headers_rect.width(), track.height),
                );
                self.draw_track_header(ui, h_rect, idx, track.clone());
                y += track.height;
            }
        });
    }

    fn draw_track_header(&mut self, ui: &mut Ui, rect: Rect, idx: usize, track: Track) {
        let sel = self.selected_track == Some(idx);
        ui.painter().rect_filled(rect, 0.0, if sel { Color32::from_rgb(45, 45, 45) } else { Color32::from_rgb(33, 33, 33) });
        // top accent strip in the track's own color
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(rect.width(), 3.0)),
            0.0,
            track.bg,
        );

        // row 1: track number + menu + mute/solo/fx
        let row1 = Rect::from_min_size(pos2(rect.min.x + 4.0, rect.min.y + 4.0), Vec2::new(rect.width() - 8.0, 16.0));
        ui.allocate_ui_at_rect(row1, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                ui.label(egui::RichText::new(format!("{}", idx + 1)).size(11.0).color(TEXT_DIM));
                // track kind tag (uses Track.kind): "V" video / "A" audio
                ui.label(
                    egui::RichText::new(if track.kind == TrackKind::Video { "V" } else { "A" })
                        .size(9.0)
                        .color(if track.kind == TrackKind::Video { ACCENT_BLUE } else { ORANGE }),
                );
                let m_btn = egui::Button::new(egui::RichText::new("M").size(10.0))
                    .fill(if track.muted { RED_REC } else { Color32::TRANSPARENT })
                    .stroke(Stroke::new(0.5_f32, TEXT_DIM))
                    .min_size(Vec2::new(16.0, 14.0));
                if ui.add(m_btn).clicked() {
                    if let Some(t) = self.tracks.get_mut(idx) {
                        t.muted = !t.muted;
                    }
                }
                let s_btn = egui::Button::new(egui::RichText::new("S").size(10.0))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(0.5_f32, TEXT_DIM))
                    .min_size(Vec2::new(16.0, 14.0));
                if ui.add(s_btn).clicked() {
                    if let Some(t) = self.tracks.get_mut(idx) {
                        t.solo = !t.solo;
                    }
                }
                let f_btn = egui::Button::new(egui::RichText::new("FX").size(9.0))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(0.5_f32, TEXT_DIM))
                    .min_size(Vec2::new(18.0, 14.0));
                if ui.add(f_btn).clicked() {
                    self.set_status(format!("Track FX: {} (demo)", track.name));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let _ = ui.small_button("≡");
                });
            });
        });

        // row 2: volume dB + pan + VU meter
        let row2 = Rect::from_min_max(
            pos2(rect.min.x + 4.0, row1.bottom() + 2.0),
            pos2(rect.max.x - 4.0, rect.max.y - 4.0),
        );
        ui.allocate_ui_at_rect(row2, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{:>5.1} dB",
                        track.volume_db
                    ))
                    .monospace()
                    .size(10.0)
                    .color(TEXT_LIGHT),
                );
                // pan readout: "Center" at 0, otherwise L/R with offset (uses track.pan)
                let pan_lbl = if track.pan.abs() < 0.01 {
                    "Center".to_string()
                } else if track.pan < 0.0 {
                    format!("L{:.0}", track.pan.abs() * 100.0)
                } else {
                    format!("R{:.0}", track.pan * 100.0)
                };
                ui.label(egui::RichText::new(pan_lbl).size(9.0).color(TEXT_DIM));
            });
            ui.add_space(1.0);
            // mini VU meter
            let vu = Rect::from_min_size(
                pos2(row2.min.x, row2.min.y + 18.0),
                Vec2::new(row2.width(), 8.0),
            );
            ui.painter().rect_filled(vu, 0.0, Color32::from_rgb(22, 22, 22));
            let lvl: f32 = if self.is_playing && !track.muted {
                0.4 + 0.5 * (self.playhead_sec as f32 * 8.0 + idx as f32).sin().abs()
            } else {
                0.0
            };
            let lw = lvl * vu.width();
            let col = if lvl > 0.85 {
                RED_REC
            } else if lvl > 0.6 {
                ORANGE
            } else {
                Color32::from_rgb(70, 200, 90)
            };
            ui.painter().rect_filled(
                Rect::from_min_size(vu.min, Vec2::new(lw, vu.height())),
                0.0,
                col,
            );
            ui.painter().rect_stroke(vu, 0.0, Stroke::new(0.5_f32, Color32::from_rgb(20, 20, 20)));
        });

        // click the header to select the track
        let resp = ui.interact(rect, egui::Id::new(("track_hdr", idx)), Sense::click());
        if resp.clicked() {
            self.selected_track = Some(idx);
        }
    }

    fn draw_ruler(&mut self, ui: &mut Ui, rect: Rect) {
        ui.painter().rect_filled(rect, 0.0, RULER_BG);
        // major tick every 0.2s (= 12 frames at 60fps), label every 1.0s
        let step = 0.2_f64;
        let mut t = 0.0;
        let max_t = (rect.width() / self.tl_zoom) as f64 + step;
        while t <= max_t {
            let x = rect.left() + (t as f32) * self.tl_zoom;
            if x > rect.right() {
                break;
            }
            let major = (t * 10.0).round() as i64 % 5 == 0;
            ui.painter().line_segment(
                [pos2(x, rect.top() + 4.0), pos2(x, rect.bottom())],
                Stroke::new(if major { 1.0 } else { 0.5_f32 }, Color32::from_rgb(90, 90, 90)),
            );
            if major {
                // compact M:SS label at the top of each integer-second tick
                let secs = t as i64;
                let lbl = format!("{}:{:02}", secs / 60, secs % 60);
                ui.painter().text(
                    pos2(x + 3.0, rect.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    lbl,
                    FontId::monospace(9.0),
                    TEXT_DIM,
                );
            }
            t += step;
        }
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0_f32, Color32::from_rgb(20, 20, 20)),
        );

        // click/drag on the ruler scrubs the playhead
        let resp = ui.interact(rect, egui::Id::new("tl_ruler"), Sense::click_and_drag());
        if resp.dragged() || resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let t = ((pos.x - rect.left()) / self.tl_zoom).max(0.0) as f64;
                self.playhead_sec = t.min(self.total_dur());
                self.is_playing = false;
            }
        }
    }

    fn draw_tracks(&mut self, ui: &mut Ui, rect: Rect) {
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(22, 22, 22));

        // Snapshot so self can be mutated during interaction without holding
        // an immutable borrow of self.tracks across the loop body.
        let tracks: Vec<Track> = self.tracks.clone();
        let mut y = rect.min.y;
        for (idx, track) in tracks.iter().enumerate() {
            let t_rect = Rect::from_min_size(pos2(rect.min.x, y), Vec2::new(rect.width(), track.height));
            // track background
            ui.painter().rect_filled(t_rect, 0.0, track.bg);
            // subtle grid every 0.2s
            let mut gx = rect.left();
            while gx <= rect.right() {
                ui.painter().line_segment(
                    [pos2(gx, t_rect.top()), pos2(gx, t_rect.bottom())],
                    Stroke::new(0.5_f32, GRID_LINE),
                );
                gx += 0.2 * self.tl_zoom;
            }

            // events on this track
            for ev in &track.events {
                let ex = t_rect.left() + (ev.start_sec as f32) * self.tl_zoom;
                let ew = (ev.dur_sec as f32) * self.tl_zoom;
                if ex + ew < t_rect.left() || ex > t_rect.right() {
                    continue;
                }
                let ev_rect = Rect::from_min_size(
                    pos2(ex, t_rect.top() + 4.0),
                    Vec2::new(ew.max(8.0), t_rect.height() - 8.0),
                );
                // body
                ui.painter().rect_filled(ev_rect, 1.0, ev.color);
                // header strip
                let hdr = Rect::from_min_size(ev_rect.min, Vec2::new(ev_rect.width(), 12.0));
                ui.painter().rect_filled(hdr, 1.0, ev.header);
                ui.painter().text(
                    pos2(hdr.min.x + 4.0, hdr.center().y),
                    egui::Align2::LEFT_CENTER,
                    &ev.name,
                    FontId::proportional(10.0),
                    TEXT_WHITE,
                );
                // duration on the right of the header
                ui.painter().text(
                    pos2(hdr.max.x - 4.0, hdr.center().y),
                    egui::Align2::RIGHT_CENTER,
                    format_tc(ev.dur_sec, self.fps),
                    FontId::monospace(8.0),
                    Color32::from_rgb(230, 230, 230),
                );

                if ev.waveform {
                    // audio waveform inside the body
                    self.draw_waveform(ui, ev_rect, ev.color);
                    if let Some(g) = ev.gain {
                        let gbox = Rect::from_min_size(
                            pos2(ev_rect.center().x - 22.0, ev_rect.bottom() - 14.0),
                            Vec2::new(44.0, 12.0),
                        );
                        ui.painter().rect_filled(gbox, 1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 160));
                        ui.painter().text(
                            gbox.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("+{:.3}", g),
                            FontId::monospace(9.0),
                            ORANGE,
                        );
                    }
                } else {
                    // video event: faux thumbnail strip
                    let mut tx = ev_rect.left() + 2.0;
                    let th = ev_rect.height() - 16.0;
                    let mut i = 0;
                    while tx + 18.0 < ev_rect.right() - 2.0 {
                        let c = if i % 2 == 0 {
                            Color32::from_rgb(70, 80, 100)
                        } else {
                            Color32::from_rgb(90, 100, 120)
                        };
                        ui.painter().rect_filled(
                            Rect::from_min_size(pos2(tx, ev_rect.top() + 14.0), Vec2::new(16.0, th)),
                            1.0,
                            c,
                        );
                        tx += 18.0;
                        i += 1;
                    }
                }

                // "87.5%" bottom-right badge like shot 3
                ui.painter().text(
                    pos2(ev_rect.right() - 3.0, ev_rect.bottom() - 1.0),
                    egui::Align2::RIGHT_BOTTOM,
                    "87.5%",
                    FontId::monospace(7.0),
                    Color32::from_rgb(220, 220, 220),
                );

                // selection outline on the currently selected track's events
                if self.selected_track == Some(idx) {
                    ui.painter().rect_stroke(ev_rect, 1.0, Stroke::new(1.0_f32, PLAYHEAD));
                }
            }

            // separator between tracks
            ui.painter().line_segment(
                [t_rect.left_bottom(), t_rect.right_bottom()],
                Stroke::new(1.0_f32, Color32::from_rgb(18, 18, 18)),
            );

            // click on empty track area to move playhead
            let resp = ui.interact(
                Rect::from_min_size(pos2(rect.min.x, y), Vec2::new(rect.width(), track.height)),
                egui::Id::new(("tl_track", idx)),
                Sense::click_and_drag(),
            );
            if resp.dragged() || resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let t = ((pos.x - rect.left()) / self.tl_zoom).max(0.0) as f64;
                    self.playhead_sec = t.min(self.total_dur());
                    self.is_playing = false;
                    self.selected_track = Some(idx);
                }
            }

            y += track.height;
        }
    }

    fn draw_waveform(&self, ui: &mut Ui, rect: Rect, base: Color32) {
        let mid = rect.center().y;
        let h = (rect.height() * 0.42).max(2.0);
        let stroke = Stroke::new(1.0_f32, Color32::from_rgb(240, 235, 170));
        let mut pts_top = Vec::new();
        let mut pts_bot = Vec::new();
        let step = 2.0_f32;
        let mut x = rect.left() + 1.0;
        let mut i = 0_u32;
        while x < rect.right() - 1.0 {
            // deterministic pseudo-waveform
            let s = (i as f32).sin() * 0.5
                + ((i as f32) * 2.3 + 1.7).sin() * 0.3
                + ((i as f32) * 5.1).sin() * 0.18;
            let amp = (s.abs() * h).min(h);
            pts_top.push(pos2(x, mid - amp));
            pts_bot.push(pos2(x, mid + amp));
            x += step;
            i += 3;
        }
        if pts_top.len() > 1 {
            ui.painter().add(egui::Shape::line(pts_top, stroke));
            ui.painter().add(egui::Shape::line(pts_bot, stroke));
        }
        // tint the base color a touch under the waveform
        let _ = base;
    }
}

// ===========================================================================
// Transport bar (screenshot 3 bottom strip)
// ===========================================================================

impl VegasApp {
    fn render_transport_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("vegas_transport")
            .exact_height(34.0)
            .show(ctx, |ui| {
                ui.painter().rect_filled(ui.max_rect(), 0.0, MENU_BAR);
                ui.spacing_mut().item_spacing = Vec2::new(1.0, 0.0);
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    // loop toggle
                    ui.add(
                        egui::Button::new(egui::RichText::new("Loop").size(10.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                    )
                    .on_hover_text("Loop");

                    // play (blue when playing)
                    let play_col = if self.is_playing { ACCENT_BLUE } else { TEXT_LIGHT };
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("▶").size(14.0).color(play_col))
                                .fill(if self.is_playing { Color32::from_rgba_unmultiplied(26, 111, 212, 60) } else { Color32::TRANSPARENT })
                                .stroke(Stroke::NONE),
                        )
                        .on_hover_text("Play / Pause (Space)")
                        .clicked()
                    {
                        self.is_playing = !self.is_playing;
                    }
                    ui.add(
                        egui::Button::new(egui::RichText::new("‖").size(12.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                    )
                    .on_hover_text("Pause");
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("■").size(11.0))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        )
                        .on_hover_text("Stop")
                        .clicked()
                    {
                        self.is_playing = false;
                        self.playhead_sec = 0.0;
                    }
                    ui.separator();
                    for (g, tip) in &[
                        ("|◀", "Go to start"),
                        ("◀◀", "Rewind"),
                        ("▶▶", "Fast forward"),
                        ("▶|", "Go to end"),
                    ] {
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new(*g).size(11.0))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::NONE),
                            )
                            .on_hover_text(*tip)
                            .clicked()
                        {
                            match *tip {
                                "Go to start" => self.playhead_sec = 0.0,
                                "Go to end" => self.playhead_sec = self.total_dur(),
                                _ => {}
                            }
                        }
                    }
                    ui.separator();
                    // record (red, blue ring when armed)
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("●").size(13.0).color(RED_REC))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::new(1.0_f32, ACCENT_BLUE)),
                        )
                        .on_hover_text("Record")
                        .clicked()
                    {
                        self.set_status("Record (demo)");
                    }
                    ui.separator();
                    for (g, tip) in &[
                        ("S", "Solo"),
                        ("Lp", "Loop region"),
                        ("Snap", "Snap"),
                        ("Qt", "Quantize"),
                        ("Split", "Split"),
                        ("Del", "Delete"),
                        ("Lk", "Lock"),
                    ] {
                        // Snap is a real toggle (reads/writes self.snap); the rest are demo.
                        let active = *g == "Snap" && self.snap;
                        let clicked = ui
                            .add(
                                egui::Button::new(egui::RichText::new(*g).size(10.0))
                                    .fill(if active { FX_BLUE } else { Color32::TRANSPARENT })
                                    .stroke(Stroke::NONE),
                            )
                            .on_hover_text(*tip)
                            .clicked();
                        if clicked && *g == "Snap" {
                            self.snap = !self.snap;
                        }
                    }
                    ui.separator();
                    // zoom +/-
                    if ui.small_button("+").clicked() {
                        self.tl_zoom *= 1.25;
                    }
                    if ui.small_button("–").clicked() {
                        self.tl_zoom /= 1.25;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format_tc(self.total_dur(), self.fps))
                                .monospace()
                                .size(11.0)
                                .color(TEXT_DIM),
                        );
                        ui.label(egui::RichText::new("/").size(11.0).color(TEXT_DIM));
                        ui.label(
                            egui::RichText::new(format_tc(self.playhead_sec, self.fps))
                                .monospace()
                                .size(11.0)
                                .color(ORANGE),
                        );
                    });
                });
            });
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Vegas-style timecode "HH:MM:SS.ff" where ff is the frame within the second.
fn format_tc(sec: f64, fps: f32) -> String {
    let fps = fps.max(1.0) as u64;
    let total_frames = (sec * fps as f64).round() as u64;
    let frames = total_frames % fps;
    let total_secs = total_frames / fps;
    let s = total_secs % 60;
    let m = (total_secs / 60) % 60;
    let h = (total_secs / 3600) % 100;
    format!("{:02}:{:02}:{:02}.{:02}", h, m, s, frames)
}

// ===========================================================================
// main
// ===========================================================================

fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::vec2(1400.0, 900.0))
            .with_min_inner_size(egui::vec2(900.0, 600.0))
            .with_title("VEGAS Pro 17.0 — Rust UI Reference"),
        ..Default::default()
    };
    eframe::run_native(
        "Vegas Pro 17 UI Reference",
        opts,
        Box::new(|_cc| Ok(Box::new(VegasApp::default()))),
    )
}
