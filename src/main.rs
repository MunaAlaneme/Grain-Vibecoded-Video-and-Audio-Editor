#![allow(deprecated)]
//! Grain — audio pitch & time lab + video editor.
//!
//! Faithful Rust/egui port of `index.html`: an Audio / Video mode toggle, the
//! full audio FX rack (pitch shift, time stretch, vibrato, speed+pitch,
//! bitcrush, vocoder, tremolo, invert, multi-pitch, more FX, studio FX) and the
//! video editor (per-pixel color grade, color curves, color wheels, HSL, mask,
//! keyframes, text overlays, sequence / overlay / music tracks, a multi-lane
//! timeline with split/delete/group/nudge/zoom, and transport).
//!
//! Brand chrome (Grain splash + loading screen, Marlett caption buttons) is
//! retained from the earlier shell. See README for what is simulated.

use eframe::egui;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use egui::{
    pos2, vec2, Align, CollapsingHeader, Color32, ColorImage, ComboBox, FontId, Id, Layout, Rect,
    RichText, Rounding, Sense, Slider, Stroke, TextureHandle, Ui,
};

// Real media / DSP / OFX engines. These are reusable library modules; not every
// variant/field is exercised by the GUI yet, so dead_code is allowed per-module.
#[allow(dead_code)]
mod audio_dsp;
#[allow(dead_code)]
mod audio_plugins;
#[allow(dead_code)]
mod bananav_suite;
mod elastique_cli;
#[allow(dead_code)]
mod media_engine;
#[allow(dead_code)]
mod ofx_host;
#[allow(dead_code)]
mod realtime_av;
#[allow(dead_code)]
mod timeline;
use audio_dsp::{AudioLabState, AudioTab as LabTab, CarrierWave, MoreFxType as LabMore, StretchTech as LabTech, StudioFxType as LabStudio, TremoloWave as LabTrem};
use audio_plugins::{PluginChain, PluginFormat};
use timeline::{DragMode as TlDragMode, ItemDrag, TimelineItem, TimelineTool, TrackHeader, TrackKind, TimelineUi};

// ===========================================================================
// Palette — blend of Vegas Pro 2027 / Vegas Pro 17 / Filmora / BananaV
// (sampled from the reference screenshots: dark gray #303030/#101010/#505050,
//  Vegas 2027 cyan accent #50b0d0, Vegas 17 selection blue #3070f0, multi-track
//  clip blocks blue/green/purple, amber playhead).
// ===========================================================================
const BG: Color32 = Color32::from_rgb(0x18, 0x18, 0x18); // deepest (window)
const PANEL: Color32 = Color32::from_rgb(0x23, 0x23, 0x23); // dock / panel fill
const PANEL2: Color32 = Color32::from_rgb(0x2b, 0x2b, 0x2b); // raised / track header
const TLBG: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x1c); // timeline lane bg
const INK: Color32 = Color32::from_rgb(0xe8, 0xe8, 0xe8);
const SUB: Color32 = Color32::from_rgb(0x90, 0x90, 0x90);
const LINE: Color32 = Color32::from_rgb(0x39, 0x39, 0x39);
const ACCENT: Color32 = Color32::from_rgb(0x3b, 0xc4, 0xe8); // cyan-blue (Vegas 2027)
const ACCENT2: Color32 = Color32::from_rgb(0x2f, 0x80, 0xed); // selection blue (Vegas 17)
const AMBER: Color32 = Color32::from_rgb(0xff, 0xb0, 0x00); // playhead / marker
const TBBG: Color32 = Color32::from_rgb(0x33, 0x33, 0x33); // button bg
// clip block colors (from the reference timeline: blue video, green audio,
// amber text, purple overlay, teal music, purple compound)
const CLIP_TXT: Color32 = Color32::from_rgb(0xf0, 0xf0, 0xf0);
const CLIP_V: Color32 = Color32::from_rgb(0x3a, 0x6e, 0xa5);
const CLIP_CMP: Color32 = Color32::from_rgb(0x7a, 0x6a, 0xb0);
const CLIP_A: Color32 = Color32::from_rgb(0x3f, 0x7a, 0x4f);
const CLIP_O: Color32 = Color32::from_rgb(0x6a, 0x5a, 0xcd);
const CLIP_M: Color32 = Color32::from_rgb(0x4a, 0x8a, 0x6a);

// ===========================================================================
// Enums
// ===========================================================================
#[derive(Clone, Copy, PartialEq, Debug)]
enum AppPhase {
    Splash,
    Loading,
    Ready,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum MarlettGlyph {
    Minimize,
    Maximize,
    #[allow(dead_code)]
    Restore,
    Close,
}

#[derive(Clone, Copy, PartialEq, Debug, Hash)]
enum DockTab {
    ProjectMedia,
    MediaGenerator,
    Transitions,
    VideoFx,
    ColorGrading,
    AudioFx,
    Titles,
    Ofx,
    Plugins,
    Export,
}
impl DockTab {
    const ALL: &'static [DockTab] = &[
        DockTab::ProjectMedia, DockTab::MediaGenerator, DockTab::Transitions,
        DockTab::VideoFx, DockTab::ColorGrading, DockTab::AudioFx,
        DockTab::Titles, DockTab::Ofx, DockTab::Plugins, DockTab::Export,
    ];
    fn label(self) -> &'static str {
        match self {
            DockTab::ProjectMedia => "Project Media",
            DockTab::MediaGenerator => "Media Generator",
            DockTab::Transitions => "Transitions",
            DockTab::VideoFx => "Video FX",
            DockTab::ColorGrading => "Color Grading",
            DockTab::AudioFx => "Audio FX",
            DockTab::Titles => "Titles",
            DockTab::Ofx => "OFX",
            DockTab::Plugins => "Plug-ins",
            DockTab::Export => "Export",
        }
    }
}

/// A saved editing workspace. Panel sizes are captured from the real egui
/// panels every frame, so "Save Layout" persists exactly what the user sees.
#[derive(Clone)]
struct WorkspaceLayout {
    name: String,
    dock: DockTab,
    left_width: f32,
    right_width: f32,
    timeline_height: f32,
    left_open: bool,
    right_open: bool,
    timeline_open: bool,
}

impl WorkspaceLayout {
    fn preset(name: &str, dock: DockTab, lw: f32, rw: f32, th: f32, lo: bool, ro: bool, to: bool) -> Self {
        Self { name: name.into(), dock, left_width: lw, right_width: rw, timeline_height: th, left_open: lo, right_open: ro, timeline_open: to }
    }
    fn defaults() -> Vec<Self> {
        vec![
            Self::preset("Editing", DockTab::ProjectMedia, 290.0, 324.0, 236.0, true, true, true),
            Self::preset("Color", DockTab::ColorGrading, 220.0, 420.0, 210.0, true, true, true),
            Self::preset("Audio", DockTab::AudioFx, 320.0, 280.0, 300.0, true, false, true),
            Self::preset("Effects", DockTab::VideoFx, 280.0, 320.0, 230.0, true, true, true),
            Self::preset("Delivery", DockTab::Export, 260.0, 300.0, 200.0, true, true, true),
            Self::preset("Preview", DockTab::ProjectMedia, 0.0, 0.0, 0.0, false, false, false),
        ]
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum FxTab {
    Pitch,
    Time,
    Vibrato,
    SpeedPitch,
    Bitcrush,
    Vocoder,
    Tremolo,
    Invert,
    MultiPitch,
    More,
    Studio,
}
impl FxTab {
    const ALL: &'static [FxTab] = &[
        FxTab::Pitch,
        FxTab::Time,
        FxTab::Vibrato,
        FxTab::SpeedPitch,
        FxTab::Bitcrush,
        FxTab::Vocoder,
        FxTab::Tremolo,
        FxTab::Invert,
        FxTab::MultiPitch,
        FxTab::More,
        FxTab::Studio,
    ];
    fn label(self) -> &'static str {
        match self {
            FxTab::Pitch => "Pitch shift",
            FxTab::Time => "Time stretch",
            FxTab::Vibrato => "Vibrato",
            FxTab::SpeedPitch => "Speed + pitch",
            FxTab::Bitcrush => "Bitcrush",
            FxTab::Vocoder => "Vocoder",
            FxTab::Tremolo => "Tremolo",
            FxTab::Invert => "Invert",
            FxTab::MultiPitch => "Multi-pitch",
            FxTab::More => "More FX",
            FxTab::Studio => "Studio FX",
        }
    }
    fn note(self) -> &'static str {
        match self {
            FxTab::Pitch => "Changes pitch only; duration is preserved by re-stretching after the shift.",
            FxTab::Time => "Changes duration only; pitch is preserved.",
            FxTab::Vibrato => "Oscillates pitch (mode a) or pitch+speed together (mode b) at an LFO rate.",
            FxTab::SpeedPitch => "Pitch rises/falls with speed — the free, zero-DSP option.",
            FxTab::Bitcrush => "Quantizes bit depth and holds samples to emulate a lower sample rate.",
            FxTab::Vocoder => "Imposes the input's spectral envelope onto a generated carrier tone, band by band.",
            FxTab::Tremolo => "Modulates amplitude with the chosen waveform — sine through to noise.",
            FxTab::Invert => "Flips polarity; inaudible alone but useful for cancellation and layering.",
            FxTab::MultiPitch => "Shifts the source by several intervals at once and mixes them into a chord.",
            FxTab::More => "Reverse, echo, or soft-clip distortion.",
            FxTab::Studio => "Wah-wah, flanger, phaser, chorus, reverb, fades and normalize.",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum PitchTech {
    ElastiquePro,
    ElastiqueA17,
    Fft,
    Wsola,
    Rb,
    Bungee,
}
impl PitchTech {
    fn label(self) -> &'static str {
        match self {
            PitchTech::ElastiquePro => "Élastique Pro V3 (phase-coherent transient + formant)",
            PitchTech::ElastiqueA17 => "Élastique A17 Drums (transient lock)",
            PitchTech::Fft => "FFT phase vocoder",
            PitchTech::Wsola => "WSOLA (similarity)",
            PitchTech::Rb => "Rubberband-style (refined WSOLA)",
            PitchTech::Bungee => "Bungee granular-phase hybrid",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum VibMode {
    A,
    B,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum MoreFx {
    Reverse,
    Echo,
    Distortion,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum StudioFx {
    Wah,
    Flanger,
    Phaser,
    Chorus,
    Reverb,
    Fade,
    Normalize,
    Lowpass,
    Highpass,
    Ringmod,
    Autopan,
    Compressor,
    Gain,
    Bassboost,
    Telephone,
    Gate,
    Stutter,
    Vinyl,
    Octaver,
}
impl StudioFx {
    fn label(self) -> &'static str {
        match self {
            StudioFx::Wah => "Wah-wah",
            StudioFx::Flanger => "Flanger",
            StudioFx::Phaser => "Phaser",
            StudioFx::Chorus => "Chorus",
            StudioFx::Reverb => "Reverb",
            StudioFx::Fade => "Fade in / out",
            StudioFx::Normalize => "Normalize",
            StudioFx::Lowpass => "Low-pass filter",
            StudioFx::Highpass => "High-pass filter",
            StudioFx::Ringmod => "Ring modulator",
            StudioFx::Autopan => "Auto-pan",
            StudioFx::Compressor => "Compressor",
            StudioFx::Gain => "Gain",
            StudioFx::Bassboost => "Bass boost",
            StudioFx::Telephone => "Telephone",
            StudioFx::Gate => "Noise gate",
            StudioFx::Stutter => "Stutter",
            StudioFx::Vinyl => "Vinyl crackle",
            StudioFx::Octaver => "Octaver (−12 st)",
        }
    }
    fn fields(self) -> StudioFields {
        use StudioFx::*;
        let modw = matches!(
            self,
            Wah | Flanger | Phaser | Chorus | Autopan | Compressor | Gate | Stutter | Vinyl
        );
        let rev = matches!(self, Reverb);
        let fade = matches!(self, Fade);
        let mixw = matches!(
            self,
            Fade | Normalize | Gain | Lowpass | Highpass | Compressor | Autopan | Bassboost
                | Telephone | Gate | Stutter
        );
        let fcw = matches!(self, Lowpass | Highpass | Ringmod);
        let gainw = matches!(self, Gain | Bassboost);
        StudioFields { modw, rev, fade, mixw, fcw, gainw }
    }
}
struct StudioFields {
    modw: bool,
    rev: bool,
    fade: bool,
    mixw: bool,
    fcw: bool,
    gainw: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum VocWave { Saw, Square, Pulse, Sine }

#[derive(Clone, Copy, PartialEq, Debug)]
enum TremWave { Sine, Triangle, Square, Saw, Isaw, Noise, Snoise }

#[derive(Clone, Copy, PartialEq, Debug)]
enum LookType { None, Grayscale, Sepia, Invert }

#[derive(Clone, Copy, PartialEq, Debug)]
enum MirrorType { None, Left, Right, Top, Bottom }

#[derive(Clone, Copy, PartialEq, Debug)]
enum TransType { None, Fade, Wipe, Slide, Zoom }


#[derive(Clone, Copy, PartialEq, Debug)]
enum CodecType { Vp9, H264, Vp8, Av1 }

#[derive(Clone, Copy, PartialEq, Debug)]
enum MaskType { None, Ellipse, Rect, Linear }

#[derive(Clone, Copy, PartialEq, Debug)]
enum CurveCh { Rgb, R, G, B }

// ===========================================================================
// Data structs
// ===========================================================================
#[derive(Clone)]
struct TextOverlay {
    uid: u64,
    t: String,
    size: f32,
    x: f32,
    y: f32,
    a: f32,
    b: f32, // 1e9 => to end
    col: Color32,
}

#[derive(Clone)]
struct OverlayClip {
    uid: u64,
    nm: String,
    dur: f32,
    start: f32,
    x: f32,
    y: f32,
    s: f32,
    o: f32,
    /// Imported media used as the actual PiP image/video poster frame.
    path: Option<std::path::PathBuf>,
}

#[derive(Clone)]
struct MusicClip {
    uid: u64,
    nm: String,
    dur: f32,
    start: f32,
    vol: f32,
    /// 0 = A2, 1 = A3, ... (A1 is the master waveform lane).
    track: usize,
    /// Real source for timeline playback. `None` is only used by the demo score.
    path: Option<std::path::PathBuf>,
}

#[derive(Clone)]
struct VideoClip {
    /// Stable identity used by egui widget IDs so drags survive reorders.
    uid: u64,
    name: String,
    /// Explicit timeline start in seconds (used by V2+ tracks; V1 stays
    /// sequential and ignores this during layout).
    start: f32,
    /// Video track index: 0 = V1, 1 = V2, ...
    track: usize,
    a: f32, // trim start fraction 0..1
    b: f32, // trim end fraction 0..1
    dur: f32,
    speed: f32,
    compound: Option<Vec<VideoClip>>, // Some => nested original clips (lossless ungroup)
    path: Option<std::path::PathBuf>,
    /// Procedural BananaV source (demo project / generator clips). `None` uses `path`.
    gen: Option<bananav_suite::GeneratorKind>,
}

impl VideoClip {
    fn new(name: String, dur: f32, path: Option<std::path::PathBuf>) -> Self {
        Self {
            uid: fresh_clip_uid(),
            name,
            start: 0.0,
            track: 0,
            a: 0.0,
            b: 1.0,
            dur,
            speed: 1.0,
            compound: None,
            path,
            gen: None,
        }
    }

    fn len(&self) -> f32 {
        (self.b - self.a) * self.dur / self.speed.max(0.01)
    }
}

static NEXT_CLIP_UID: AtomicU64 = AtomicU64::new(1);
fn fresh_clip_uid() -> u64 {
    NEXT_CLIP_UID.fetch_add(1, AtomicOrdering::Relaxed)
}

/// The built-in demo score: a short, real PCM signal (not a fake waveform).
fn make_demo_samples(duration: f32, sr: u32) -> Vec<f32> {
    let n = (duration.max(0.1) * sr.max(1) as f32) as usize;
    let mut samples = Vec::with_capacity(n);
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    for i in 0..n {
        let t = i as f32 / sr.max(1) as f32;
        let phrase = (t / 2.0).floor();
        let note = [220.0_f32, 277.18, 329.63, 246.94][(phrase as usize) % 4];
        let env = (1.0 - (t % 2.0) / 2.0).max(0.0) * 0.85 + 0.15;
        seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17;
        let noise = ((seed as f32) / (u32::MAX as f32) * 2.0 - 1.0) * 0.015;
        let s = (2.0 * std::f32::consts::PI * note * t).sin() * 0.22
            + (2.0 * std::f32::consts::PI * note * 2.0 * t).sin() * 0.07
            + (2.0 * std::f32::consts::PI * 55.0 * t).sin() * 0.09
            + noise;
        samples.push(s * env);
    }
    samples
}

/// Duplicate a mono score into interleaved L/R samples for the Audio FX rack.
fn make_demo_stereo_samples(duration: f32, sr: u32) -> Vec<f32> {
    let mono = make_demo_samples(duration, sr);
    let mut stereo = Vec::with_capacity(mono.len() * 2);
    for s in mono {
        stereo.push(s);
        stereo.push(s);
    }
    stereo
}

fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Keep the per-pixel grade interactive by previewing at a capped resolution,
/// while `VideoState.source_w/source_h` retain the real source dimensions.
fn preview_dims(w: u32, h: u32) -> (usize, usize) {
    let max_w = 960_u32;
    let max_h = 540_u32;
    let scale = (max_w.max(1) as f32 / w.max(1) as f32)
        .min(max_h.max(1) as f32 / h.max(1) as f32)
        .min(1.0);
    let pw = ((w.max(16) as f32 * scale).round() as usize).max(16);
    let ph = ((h.max(9) as f32 * scale).round() as usize).max(9);
    (pw, ph)
}

/// Where a timeline item lives in `VideoState`. The timeline UI works purely
/// with UIDs and resolves through this map only when an edit is committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ItemRef {
    V1(usize),
    Video { track: usize, index: usize },
    Overlay(usize),
    Music(usize),
    Text(usize),
}

/// A track lane as drawn by the new timeline. `state_idx` maps to the legacy
/// mute/solo arrays where applicable (-1 means the lane uses extended state).
#[derive(Clone)]
struct TlTrack {
    header: TrackHeader,
    state_idx: isize,
}

/// Fully resolved source for the current preview frame. Compounds are expanded
/// recursively so grouped clips still show their real nested media.
#[derive(Clone)]
struct SourceRef {
    path: Option<std::path::PathBuf>,
    gen: Option<bananav_suite::GeneratorKind>,
    time: f32,
}

#[derive(Clone)]
struct Keyframe {
    t: f32,
    v: [f32; 20],
}

#[derive(Clone, Copy)]
struct CurvePts {
    p0: [f32; 2],
    p1: [f32; 2],
}
impl Default for CurvePts {
    fn default() -> Self {
        CurvePts { p0: [1.0 / 3.0, 1.0 / 3.0], p1: [2.0 / 3.0, 2.0 / 3.0] }
    }
}

// An OFX effect staged on the selected clip. `kind` is derived from the plugin
// identifier (glow / blur / warp / vignette / sharpen / generic) and the param
// values drive the real per-pixel pass in `recompute_frame`.
#[derive(Clone)]
struct OfxFx {
    #[allow(dead_code)] ident: String,
    kind: OfxKind,
    mix: f32,
    intensity: f32,
    p_a: f32, // threshold / blur_amount / amplitude
    p_b: f32, // radius / frequency
    /// Full OFX plugin parameters (not just the four mapped render fields).
    params: Vec<ofx_host::OfxParamProperty>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum OfxKind { Generic, Glow, Blur, Warp, Vignette, Sharpen }

impl OfxFx {
    /// Classify a plugin by its identifier (matches ofx_host::default_ofx_plugin_params).
    fn classify(ident: &str) -> OfxKind {
        let s = ident.to_lowercase();
        if s.contains("glow") || s.contains("flare") || s.contains("ray") || s.contains("light") { OfxKind::Glow }
        else if s.contains("blur") || s.contains("defocus") { OfxKind::Blur }
        else if s.contains("warp") || s.contains("distort") || s.contains("wave") { OfxKind::Warp }
        else if s.contains("vignette") { OfxKind::Vignette }
        else if s.contains("sharpen") { OfxKind::Sharpen }
        else { OfxKind::Generic }
    }
}

// 20 keyframable video params, kfIds order:
//  vb vc vs vh vbl vv | vwh vwv vtilt vyaw vcube vrgb | vpx vgr vsc vgl vpo | vkl vsk vtl
struct VideoState {
    #[allow(dead_code)] loaded: bool,
    #[allow(dead_code)] filename: String,
    duration: f32,
    trim_start: f32,
    trim_end: f32,
    playhead: f32,
    is_playing: bool,
    /// Real source dimensions from `ffprobe`; preview canvas is downscaled.
    source_w: u32,
    source_h: u32,
    fps: f32,
    fw: usize,
    fh: usize,
    src_pix: Vec<u8>,
    out_tex: Option<TextureHandle>,
    /// Cached textures for PiP overlays added from imported media.
    overlay_tex: HashMap<std::path::PathBuf, TextureHandle>,
    /// Dedicated ffmpeg pipe for video overlays (keeps poster frames live).
    overlay_stream: Option<realtime_av::ContinuousVideoStream>,
    /// Set when pixels need recomputing; cleared after a successful frame.
    dirty: bool,
    /// Cheap signature of everything that affects the graded source frame.
    /// While paused with no changes, the already-uploaded texture is reused.
    last_frame_key: String,
    // basic color
    vb: f32, vc: f32, vs: f32, vh: f32, vbl: f32, vv: f32,
    look: LookType,
    speed: f32,
    // distort
    vwh: f32, vwv: f32, vtilt: f32, vyaw: f32, vcube: f32, vrgb: f32,
    mirror: MirrorType,
    vpx: f32, vgr: f32, vsc: f32, vgl: f32, vpo: f32,
    vkl: f32, vsk: f32, vtl: f32,
    // transition
    vtr_type: TransType,
    vtr_dur: f32,
    // curves
    vcv_ch: CurveCh,
    curves: [CurvePts; 4],
    // text
    vtx_text: String,
    vtx_size: f32, vtx_x: f32, vtx_y: f32, vtx_a: f32, vtx_b: f32,
    vtx_col: Color32,
    texts: Vec<TextOverlay>,
    // encoder

    venc_codec: CodecType,
    venc_br: f32,
    // color wheels (dx, dy in -1..1)
    wheel_sh: [f32; 2],
    wheel_mid: [f32; 2],
    wheel_hi: [f32; 2],
    // hsl
    vhsl_b: u8,
    vhsl_h: f32, vhsl_s: f32, vhsl_l: f32,
    // mask
    vmk_type: MaskType,
    vmk_x: f32, vmk_y: f32, vmk_s: f32, vmk_f: f32,
    vmk_i: bool,
    // keyframes
    keyframes: Vec<Keyframe>,
    // sequence
    /// Primary V1 track: contiguous clips (legacy sequential layout).
    seq: Vec<VideoClip>,
    /// Extra video tracks V2..Vn. Each clip carries an explicit `start` time.
    extra_tracks: Vec<Vec<VideoClip>>,
    extra_track_mute: Vec<bool>,
    extra_track_solo: Vec<bool>,
    /// Per-state-index visibility (V1/V2+/Overlay/Text); audio lanes stay audible.
    track_visible: Vec<bool>,
    /// A2 is always present; A3+ reuse `music` clips with `track >= 1`.
    extra_audio_mute: Vec<bool>,
    extra_audio_solo: Vec<bool>,
    /// Ruler markers (Premiere-style `M`).
    markers: Vec<f32>,
    sel: Vec<usize>,
    /// Selected clip uid on a V2+ track (V1 selection stays index-based).
    extra_sel: Option<u64>,
    cmp_n: u32,
    // overlay + music
    overlays: Vec<OverlayClip>,
    music: Vec<MusicClip>,
    vov_t: f32, vov_x: f32, vov_y: f32, vov_s: f32, vov_o: f32,
    vmu_t: f32, vmu_v: f32,
    tl_zoom: f32,
    grp_open: [bool; 6],
    track_mute: [bool; 5],
    track_solo: [bool; 5],
    processing: bool,
    progress: f32,
    done: bool,
    status: String,
    // imported media + real-time video stream (ffmpeg subprocess) + staged OFX effect
    media_path: Option<std::path::PathBuf>,
    video_stream: Option<realtime_av::ContinuousVideoStream>,
    ofx_active: Option<OfxFx>,
    last_rgba: Vec<u8>,
    /// True while the built-in generated demo timeline is loaded.
    is_demo: bool,
    /// Seek requested from preview/timeline widgets, applied by GrainApp.
    seek_request: Option<f32>,
    timeline_drop_rect: Option<Rect>,
    // BananaV v0.3.0 suite fields
    aces_enabled: bool,
    aces_view: bananav_suite::AcesViewTransform,
    lgg_lift: [f32; 3],
    lgg_gamma: [f32; 3],
    lgg_gain: [f32; 3],
    lgg_offset: [f32; 3],
    generator_kind: bananav_suite::GeneratorKind,
    fx_chain_level: bananav_suite::FxChainLevel,
    scope_kind: bananav_suite::VideoScopeKind,
    show_scopes: bool,
    preview_transform_mode: bool,
    fx_search: String,
    fx_cat_filter: String,
}

/// Result of an asynchronous Audio FX source load.
#[derive(Clone)]
struct AudioFxLoad {
    name: String,
    path: std::path::PathBuf,
    samples: Vec<f32>,
    sample_rate: u32,
    duration: f32,
}

struct AudioState {
    loaded: bool,
    is_demo: bool,
    filename: String,
    duration: f32,
    sample_rate: u32,
    channels: u32,
    samples: Vec<f32>,
    playhead: f32,
    is_playing: bool,
    mute: bool,
    solo: bool,
    track_gain_db: f32,
    track_pan: f32,
    sel_start: f32,
    sel_end: f32,
    tempo_bpm: u32,
    time_sig_num: u8,
    time_sig_den: u8,
    snap_enabled: bool,
    active_tool: u8, // 0=Select(I), 1=Envelope, 2=Draw, 3=Zoom, 4=TimeShift, 5=Multi
    fx_tab: FxTab,
    p_tech: PitchTech, p_amt: f32, fft_size: u32, fm_amt: f32, fm_keep: bool,
    t_tech: PitchTech, t_amt: f32,
    v_mode: VibMode, v_tech: PitchTech, v_rate: f32, v_depth: f32,
    s_amt: f32, s_start: f32, s_end: f32, s_sliding: bool,
    b_bits: i32, b_sr: i32,
    vc_wave: VocWave, vc_freq: f32, vc_bands: i32,
    tr_wave: TremWave, tr_rate: f32, tr_depth: f32,
    mp_tech: PitchTech, mp_intervals: [bool; 9],
    mf_type: MoreFx, mf_delay: f32, mf_fb: f32, mf_drive: f32,
    sf_type: StudioFx, sf_rate: f32, sf_depth: f32, sf_decay: f32, sf_mix: f32, sf_fc: f32, sf_gain: f32, sf_in: f32, sf_out: f32,
    processing: bool, progress: f32, done: bool, status: String,
    /// Async Audio FX loading with cancel/pause.
    fx_loading: bool,
    fx_paused: bool,
    fx_cancel_requested: bool,
    fx_loading_status: String,
    fx_rx: Option<std::sync::mpsc::Receiver<Result<AudioFxLoad, String>>>,
    /// Separate Audio FX popup window.
    audio_fx_popup_open: bool,
}

/// Vegas-style New Project properties.
#[derive(Clone)]
struct NewProjectDraft {
    name: String,
    width: u32,
    height: u32,
    fps: f32,
    sample_rate: u32,
    save_path: String,
}

impl Default for NewProjectDraft {
    fn default() -> Self {
        Self { name: "Untitled Project".into(), width: 1920, height: 1080, fps: 30.0, sample_rate: 48_000, save_path: String::new() }
    }
}

struct GrainApp {
    phase: AppPhase,
    phase_t: f32,
    logo_tex: Option<TextureHandle>,
    splash_bg_tex: Option<TextureHandle>,
    marlett_loaded: bool,
    dock: DockTab,
    audio: AudioState,
    video: VideoState,
    /// Real-time CPAL playback engine (silent clock stub when CPAL is disabled).
    audio_engine: realtime_av::RealtimeAudioEngine,
    /// Decoded A2 music clips, keyed by source path (avoids repeat ffmpeg work).
    audio_clip_cache: HashMap<std::path::PathBuf, std::sync::Arc<(Vec<f32>, Vec<f32>, u32)>>,
    /// Signature of the A2 mix; rebuilding only when it changes keeps playback stable.
    last_audio_mix_key: String,
    preview_drop_rect: Option<Rect>,
    ofx_plugins: Vec<ofx_host::OfxPluginInfo>,
    ofx_sel: Option<usize>,
    ofx_scanned: bool,
    import_path: String,
    drag_drop_enabled: bool,
    append_timeline_mode: bool,
    media_bin: Vec<(String, std::path::PathBuf, f32)>,
    dragging_bin_idx: Option<usize>,
    dragging_fx_label: Option<String>,
    /// Unified VST2 / VST3 / AudioUnit plugin host and mix chain.
    plugins: PluginChain,
    plugin_filter: String,
    plugin_slot_sel: usize,
    window_maximized: bool,
    fps_smoothed: f32,
    /// Editing workspaces: named panel arrangements recallable from the top bar.
    layouts: Vec<WorkspaceLayout>,
    active_layout: usize,
    layout_window_open: bool,
    /// Generator-clip insertion settings for the Media Generator dock tab.
    gen_clip_dur: f32,
    gen_clip_track: usize,
    /// Premiere/Filmora-style timeline interaction state (tools, snap, drag).
    timeline_ui: TimelineUi,
    /// Last hovered drop lane/time for media-bin drops onto the timeline.
    tl_drop_target: Option<(usize, f32)>,
    /// Selected effect shown in a modal popup.
    selected_fx: Option<String>,
    project_name: String,
    project_path: Option<std::path::PathBuf>,
    project_dirty: bool,
    close_project_open: bool,
    new_project_open: bool,
    new_project_draft: NewProjectDraft,
    pending_close_window: bool,
    /// Vegas-style Ctrl+G "Go to time" popup.
    go_to_time_open: bool,
    go_to_time_input: String,
    /// Async Video FX scan with cancel/pause.
    video_fx_loading: bool,
    video_fx_paused: bool,
    video_fx_cancel_requested: bool,
    video_fx_status: String,
    video_fx_rx: Option<std::sync::mpsc::Receiver<Vec<ofx_host::OfxPluginInfo>>>,
}

impl Default for VideoState {
    fn default() -> Self {
        VideoState {
            loaded: false,
            filename: String::new(),
            duration: 1.0,
            trim_start: 0.0, trim_end: 1.0,
            playhead: 0.0, is_playing: false,
            source_w: 1920, source_h: 1080, fps: 30.0,
            fw: 960, fh: 540,
            src_pix: Vec::new(), out_tex: None, overlay_tex: HashMap::new(), overlay_stream: None,
            dirty: true, last_frame_key: String::new(),
            vb: 100.0, vc: 100.0, vs: 100.0, vh: 0.0, vbl: 0.0, vv: 0.0,
            look: LookType::None, speed: 1.0,
            vwh: 0.0, vwv: 0.0, vtilt: 0.0, vyaw: 0.0, vcube: 0.0, vrgb: 0.0,
            mirror: MirrorType::None,
            vpx: 0.0, vgr: 0.0, vsc: 0.0, vgl: 0.0, vpo: 0.0,
            vkl: 0.0, vsk: 0.0, vtl: 0.0,
            vtr_type: TransType::None, vtr_dur: 0.8,
            vcv_ch: CurveCh::Rgb, curves: [CurvePts::default(); 4],
            vtx_text: String::new(), vtx_size: 8.0, vtx_x: 50.0, vtx_y: 85.0, vtx_a: 0.0, vtx_b: 0.0,
            vtx_col: Color32::WHITE,
            texts: Vec::new(),
            venc_codec: CodecType::H264, venc_br: 8.0,
            wheel_sh: [0.0, 0.0], wheel_mid: [0.0, 0.0], wheel_hi: [0.0, 0.0],
            vhsl_b: 0, vhsl_h: 0.0, vhsl_s: 0.0, vhsl_l: 0.0,
            vmk_type: MaskType::None, vmk_x: 50.0, vmk_y: 50.0, vmk_s: 60.0, vmk_f: 20.0, vmk_i: false,
            keyframes: Vec::new(),
            seq: Vec::new(), extra_tracks: vec![Vec::new()], extra_track_mute: vec![false],
            extra_track_solo: vec![false], track_visible: vec![true; 6],
            extra_audio_mute: Vec::new(), extra_audio_solo: Vec::new(),
            markers: Vec::new(), sel: Vec::new(), extra_sel: None, cmp_n: 0,
            overlays: Vec::new(),
            music: Vec::new(),
            vov_t: 2.0, vov_x: 75.0, vov_y: 75.0, vov_s: 30.0, vov_o: 100.0,
            vmu_t: 0.0, vmu_v: 60.0,
            tl_zoom: 60.0,
            grp_open: [true, true, false, false, false, false],
            track_mute: [false; 5],
            track_solo: [false; 5],
            processing: false, progress: 0.0, done: false, status: String::new(),
            media_path: None, video_stream: None, ofx_active: None, last_rgba: Vec::new(),
            is_demo: false, seek_request: None, timeline_drop_rect: None,
            aces_enabled: false,
            aces_view: bananav_suite::AcesViewTransform::SrgbRec709,
            lgg_lift: [0.0, 0.0, 0.0],
            lgg_gamma: [1.0, 1.0, 1.0],
            lgg_gain: [1.0, 1.0, 1.0],
            lgg_offset: [0.0, 0.0, 0.0],
            generator_kind: bananav_suite::GeneratorKind::None,
            fx_chain_level: bananav_suite::FxChainLevel::Event,
            scope_kind: bananav_suite::VideoScopeKind::Histogram,
            show_scopes: true,
            preview_transform_mode: false,
            fx_search: String::new(),
            fx_cat_filter: "All".into(),
        }
    }
}

impl Default for AudioState {
    fn default() -> Self {
        let duration = 12.0_f32;
        let samples = make_demo_stereo_samples(duration, 44_100);
        AudioState {
            loaded: true, is_demo: true, filename: "demo_clip.wav".into(), duration, sample_rate: 44_100, channels: 2,
            samples, playhead: 0.0, is_playing: false,
            mute: false, solo: false, track_gain_db: 0.0, track_pan: 0.0,
            sel_start: 0.0, sel_end: duration,
            tempo_bpm: 128, time_sig_num: 4, time_sig_den: 4,
            snap_enabled: false, active_tool: 0,
            fx_tab: FxTab::Pitch,
            p_tech: PitchTech::Fft, p_amt: 0.0, fft_size: 2048, fm_amt: 0.0, fm_keep: false,
            t_tech: PitchTech::Fft, t_amt: 1.0,
            v_mode: VibMode::A, v_tech: PitchTech::Fft, v_rate: 5.0, v_depth: 0.5,
            s_amt: 1.0, s_start: 1.0, s_end: 1.0, s_sliding: false, b_bits: 8, b_sr: 4,
            vc_wave: VocWave::Saw, vc_freq: 110.0, vc_bands: 16,
            tr_wave: TremWave::Sine, tr_rate: 5.0, tr_depth: 0.8,
            mp_tech: PitchTech::Fft, mp_intervals: [false, false, false, false, true, false, false, true, false],
            mf_type: MoreFx::Echo, mf_delay: 250.0, mf_fb: 0.4, mf_drive: 4.0,
            sf_type: StudioFx::Wah, sf_rate: 1.0, sf_depth: 0.7, sf_decay: 2.0, sf_mix: 0.6, sf_fc: 1000.0, sf_gain: 0.0, sf_in: 1.0, sf_out: 1.0,
            processing: false, progress: 0.0, done: false, status: String::new(),
            fx_loading: false, fx_paused: false, fx_cancel_requested: false,
            fx_loading_status: String::new(), fx_rx: None, audio_fx_popup_open: false,
        }
    }
}

impl Default for GrainApp {
    fn default() -> Self {
        let mut app = GrainApp {
            phase: AppPhase::Splash, phase_t: 0.0, logo_tex: None, splash_bg_tex: None, marlett_loaded: false,
            dock: DockTab::ProjectMedia, audio: AudioState::default(), video: VideoState::default(),
            audio_engine: realtime_av::RealtimeAudioEngine::new(),
            audio_clip_cache: HashMap::new(), last_audio_mix_key: String::new(),
            preview_drop_rect: None,
            ofx_plugins: Vec::new(), ofx_sel: None, ofx_scanned: false,
            import_path: String::new(),
            drag_drop_enabled: true,
            append_timeline_mode: true,
            media_bin: Vec::new(),
            dragging_bin_idx: None,
            dragging_fx_label: None,
            plugins: PluginChain::new(44_100),
            plugin_filter: String::new(),
            plugin_slot_sel: 0,
            window_maximized: false, fps_smoothed: 0.0,
            layouts: WorkspaceLayout::defaults(),
            active_layout: 0,
            layout_window_open: false,
            gen_clip_dur: 4.0,
            gen_clip_track: 0,
            timeline_ui: TimelineUi::default(),
            tl_drop_target: None,
            selected_fx: None,
            project_name: "Grain Demo Project".into(), project_path: None, project_dirty: false,
            close_project_open: false, new_project_open: false, new_project_draft: NewProjectDraft::default(),
            pending_close_window: false, go_to_time_open: false, go_to_time_input: String::new(),
            video_fx_loading: false, video_fx_paused: false, video_fx_cancel_requested: false,
            video_fx_status: String::new(), video_fx_rx: None,
        };
        app.load_demo_project();
        app
    }
}

// ===========================================================================
// Asset helpers
// ===========================================================================
fn load_embedded_png(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let image = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
    Some(ctx.load_texture(name, image, egui::TextureOptions::default()))
}

impl GrainApp {
    fn ensure_assets(&mut self, ctx: &egui::Context) {
        if self.logo_tex.is_some() && self.splash_bg_tex.is_some() {
            return;
        }
        self.marlett_loaded = false;
        let mut fonts = egui::FontDefinitions::default();
        for path in ["assets/marlett.ttf", "C:\\Windows\\Fonts\\marlett.ttf", "/usr/share/fonts/marlett.ttf", "/usr/share/fonts/truetype/marlett.ttf"] {
            if let Ok(bytes) = std::fs::read(path) {
                fonts.font_data.insert("Marlett".to_owned(), egui::FontData::from_owned(bytes));
                fonts.families.entry(egui::FontFamily::Name("Marlett".into())).or_default().push("Marlett".to_owned());
                self.marlett_loaded = true;
                break;
            }
        }
        ctx.set_fonts(fonts);
        if self.logo_tex.is_none() {
            self.logo_tex = load_embedded_png(ctx, "grain_logo", include_bytes!("../assets/grain_logo.png"));
        }
        if self.splash_bg_tex.is_none() {
            self.splash_bg_tex = load_embedded_png(ctx, "splash_bg", include_bytes!("../assets/splash_bg.png"));
        }
    }

    // -----------------------------------------------------------------------
    // Project + transport helpers. These are the single source of truth used
    // by the top bar, keyboard shortcuts, timeline and Audio FX transport.
    // -----------------------------------------------------------------------

    /// Load a fully playable demo project: three distinct generated video
    /// clips, a generated 12-second stereo score, and a title track. No fake
    /// file paths — generators produce real frames and the WAV buffer is real
    /// PCM sent to the audio device.
    fn load_demo_project(&mut self) {
        use bananav_suite::GeneratorKind as G;
        self.set_transport_playing(false);
        self.video = VideoState::default();
        self.video.loaded = true;
        self.video.is_demo = true;
        self.video.filename = "Grain Demo Project".into();
        self.video.duration = 12.0;
        self.video.source_w = 1920;
        self.video.source_h = 1080;
        self.video.fps = 30.0;
        let (pw, ph) = preview_dims(1920, 1080);
        self.video.fw = pw;
        self.video.fh = ph;
        // Better demo: every real media generator, on V1 and V2+, with titles.
        let gens = [
            G::SmpteBars, G::Checkerboard, G::ColorGradient, G::SolidColor,
            G::PerlinNoiseTexture, G::CreditRoll, G::TimecodeBurnIn, G::TestGrid,
        ];
        let names = ["SMPTE Bars","Checkerboard","Color Gradient","Solid Matte","Perlin Clouds","Credit Roll","Timecode Burn-In","Calibration Grid"];
        let seg_dur = 2.0_f32;
        self.video.duration = seg_dur * gens.len() as f32;
        self.video.seq = gens.iter().enumerate().map(|(i, gen)| {
            let mut c = VideoClip::new(format!("Demo · {}", names[i]), seg_dur, None);
            c.gen = Some(*gen);
            c
        }).collect();
        // V2+ overlay demo using the same generators on a separate track.
        if self.video.extra_tracks.is_empty() { self.video.extra_tracks.push(Vec::new()); }
        let mut overlay = VideoClip::new("Demo · V2 Calibration".into(), seg_dur * gens.len() as f32, None);
        overlay.gen = Some(G::TestGrid);
        overlay.track = 1; overlay.start = 0.0;
        self.video.extra_tracks[0].push(overlay);
        self.video.sel = vec![0];
        self.video.texts.push(TextOverlay {
            uid: fresh_clip_uid(),
            t: "GRAIN DEMO".into(), size: 9.0, x: 50.0, y: 12.0,
            a: 0.0, b: 1e9, col: Color32::WHITE,
        });
        let overcast = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets").join("Kevin MacLeod - Overcast.mp3");
        let (mut sr, mut audio_len, mut inter) = (44_100_u32, self.video.duration, Vec::new());
        if let Ok((l, r, rate)) = media_engine::decode_audio_stereo(&overcast) {
            sr = rate.max(1);
            let n = l.len().min(r.len());
            inter = Vec::with_capacity(n * 2);
            for i in 0..n { inter.push(l[i]); inter.push(r[i]); }
            audio_len = (n as f32 / sr as f32).max(0.1);
        }
        if inter.is_empty() {
            inter = make_demo_stereo_samples(self.video.duration, sr);
            audio_len = self.video.duration;
        }
        let music_dur = audio_len.min(self.video.duration).max(0.1);
        let music_path = if overcast.exists() { Some(overcast.clone()) } else { None };
        self.video.music.push(MusicClip {
            uid: fresh_clip_uid(),
            track: 0, nm: "Kevin MacLeod — Overcast".into(), dur: music_dur, start: 0.0, vol: 80.0,
            path: music_path,
        });
        self.audio.is_demo = true;
        self.audio.loaded = true;
        self.audio.filename = "Kevin MacLeod - Overcast.mp3".into();
        self.audio.sample_rate = sr;
        self.audio.channels = 2;
        self.audio.duration = audio_len;
        self.plugins.set_sample_rate(sr);
        self.audio.samples = inter;
        self.audio.playhead = 0.0;
        self.audio.sel_start = 0.0;
        self.audio.sel_end = audio_len;
        self.audio.is_playing = false;
        self.audio.status = "Demo project loaded · all 8 media generators + Kevin MacLeod — Overcast.".into();
        self.video.status = "Demo project ready · 8 generator clips on V1 + V2 · Kevin MacLeod — Overcast.".into();
        self.video.dirty = true;
        self.last_audio_mix_key.clear();
        self.reload_playback_buffer();
    }

    /// Replace the generated demo with real media on the first import.
    fn clear_demo_if_needed(&mut self) {
        let was_demo = self.video.is_demo || self.audio.is_demo;
        if self.video.is_demo {
            self.set_transport_playing(false);
            self.video.seq.clear();
            for t in &mut self.video.extra_tracks { t.clear(); }
            self.video.extra_track_mute.truncate(1);
            self.video.extra_track_solo.truncate(1);
            self.video.track_visible.truncate(6);
            self.video.extra_audio_mute.clear();
            self.video.extra_audio_solo.clear();
            self.video.markers.clear();
            self.video.extra_sel = None;
            self.timeline_ui.clear_selection();
            self.video.overlays.clear();
            self.video.texts.clear();
            self.video.music.clear();
            self.video.sel.clear();
            self.video.media_path = None;
            self.video.video_stream = None;
            self.video.playhead = 0.0;
            self.video.duration = 1.0;
            self.video.is_demo = false;
            self.video.dirty = true;
            self.video.status.clear();
        }
        if self.audio.is_demo {
            self.audio.is_demo = false;
            self.audio.samples.clear();
            self.audio.duration = 0.0;
            self.audio.filename.clear();
            self.audio.loaded = false;
            self.audio.playhead = 0.0;
            self.audio_engine.pause();
        }
        if was_demo { self.last_audio_mix_key.clear(); }
    }

    /// Push the current Audio FX buffer into the real-time engine.
    fn reload_playback_buffer(&mut self) {
        let ch = self.audio.channels.max(1) as usize;
        let n = self.audio.samples.len() / ch;
        let mut left = Vec::with_capacity(n);
        let mut right = Vec::with_capacity(n);
        for i in 0..n {
            left.push(self.audio.samples[i * ch]);
            right.push(if ch > 1 { self.audio.samples[i * ch + 1] } else { self.audio.samples[i * ch] });
        }
        self.audio_engine.load_stereo_buffer(left, right, self.audio.sample_rate);
        self.audio_engine.seek_seconds(self.audio.playhead as f64);
        self.sync_audio_controls();
    }

    fn sync_audio_controls(&self) {
        let gain = db_to_gain(self.audio.track_gain_db);
        self.audio_engine.set_gain(gain);
        self.audio_engine.set_pan(self.audio.track_pan);
        let muted = self.audio.mute || !self.video.track_active(2);
        self.audio_engine.set_muted(muted);
        self.audio_engine.set_rate(self.video.speed.clamp(0.25, 4.0));
    }

    fn audio_mix_key(&self) -> String {
        let clips = self.video.music.iter().map(|m| format!(
            "{}|{}|{}|{}|{}|{}|{}",
            m.nm,
            m.path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            (m.start * 1000.0).round() as i64,
            (m.dur * 1000.0).round() as i64,
            (m.vol * 10.0).round() as i64,
            m.track, self.video.audio_track_active(m.track),
        )).collect::<Vec<_>>().join(";");
        format!("{}|{}|{}", self.audio.sample_rate, self.audio.is_demo, clips)
    }

    /// Linear-resample a stereo pair to the project sample rate.
    fn resample_stereo(l: &[f32], r: &[f32], from: u32, to: u32) -> (Vec<f32>, Vec<f32>) {
        if from == 0 || l.is_empty() || from == to {
            return (l.to_vec(), r.to_vec());
        }
        let ratio = to as f64 / from as f64;
        let out_n = (l.len() as f64 * ratio).ceil() as usize;
        let mut ol = Vec::with_capacity(out_n);
        let mut or_ = Vec::with_capacity(out_n);
        for i in 0..out_n {
            let pos = i as f64 / ratio;
            let i0 = pos.floor() as usize;
            let i1 = (i0 + 1).min(l.len() - 1);
            let f = (pos - i0 as f64) as f32;
            ol.push(l[i0] * (1.0 - f) + l[i1] * f);
            let rr = if r.len() > i1 { r[i1] } else { r[i0.min(r.len().saturating_sub(1))] };
            let r0 = r[i0.min(r.len().saturating_sub(1))];
            or_.push(r0 * (1.0 - f) + rr * f);
        }
        (ol, or_)
    }

    /// Mix every A2 music clip into one timeline-aligned PCM buffer and feed it
    /// to the real-time engine. Unknown/generated clips use the demo score.
    fn maybe_rebuild_timeline_audio(&mut self) {
        let key = self.audio_mix_key();
        if key == self.last_audio_mix_key { return; }
        self.last_audio_mix_key = key;
        // No timeline audio clips: preserve the raw A1 source instead of
        // replacing it with generated silence.
        if self.video.music.is_empty() { return; }

        let sr = self.audio.sample_rate.max(1);
        let total = self.video.total()
            .max(self.video.music.iter().map(|m| m.start + m.dur).fold(0.0_f32, f32::max))
            .max(self.audio.duration);
        let frames = (total * sr as f32).ceil() as usize;
        let mut left = vec![0.0_f32; frames];
        let mut right = vec![0.0_f32; frames];

        for m in self.video.music.iter() {
            if !self.video.audio_track_active(m.track) { continue; }
            let (src_l, src_r) = if let Some(path) = m.path.as_ref() {
                let decoded = if let Some(cached) = self.audio_clip_cache.get(path) {
                    cached
                } else {
                    match media_engine::decode_audio_stereo(path) {
                        Ok(d) => {
                            self.audio_clip_cache.insert(path.clone(), std::sync::Arc::new(d));
                            self.audio_clip_cache.get(path).expect("just inserted").as_ref()
                        }
                        Err(e) => {
                            self.video.status = format!("A2 clip decode failed ({}): {}", m.nm, e);
                            continue;
                        }
                    }
                };
                Self::resample_stereo(&decoded.0, &decoded.1, decoded.2, sr)
            } else if self.audio.is_demo {
                let mono = make_demo_samples(m.dur.max(0.1), sr);
                (mono.clone(), mono)
            } else {
                let ch = self.audio.channels.max(1) as usize;
                let n = self.audio.samples.len() / ch;
                let mut sl = Vec::with_capacity(n);
                let mut sr_ = Vec::with_capacity(n);
                for i in 0..n {
                    sl.push(self.audio.samples[i * ch]);
                    sr_.push(if ch > 1 { self.audio.samples[i * ch + 1] } else { self.audio.samples[i * ch] });
                }
                (sl, sr_)
            };
            let gain = (m.vol / 100.0).clamp(0.0, 2.0);
            let start = (m.start.max(0.0) * sr as f32).round() as usize;
            let want = (m.dur.max(0.0) * sr as f32).round() as usize;
            let n = src_l.len().min(want).min(frames.saturating_sub(start));
            for i in 0..n {
                left[start + i] += src_l[i] * gain;
                right[start + i] += src_r.get(i).copied().unwrap_or(src_l[i]) * gain;
            }
        }

        let mut inter = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            inter.push(left[i]);
            inter.push(right[i]);
        }
        self.audio.samples = inter;
        self.audio.channels = 2;
        self.audio.sample_rate = sr;
        self.audio.duration = frames as f32 / sr as f32;
        self.audio.sel_end = self.audio.duration;
        if self.audio.duration > 0.0 && self.audio.playhead > self.audio.duration {
            self.audio.playhead = self.audio.duration;
        }
        self.reload_playback_buffer();
    }

    fn seek_transport(&mut self, t: f32) {
        let vt = t.clamp(0.0, self.video.total());
        let at = t.clamp(0.0, self.audio.duration.max(vt));
        self.video.playhead = vt;
        self.audio.playhead = at;
        self.audio_engine.seek_seconds(at as f64);
    }

    fn set_transport_playing(&mut self, play: bool) {
        if play {
            let at_end = self.video.playhead >= self.video.total() - 0.01
                || (self.audio.duration > 0.0 && self.audio.playhead >= self.audio.duration - 0.01);
            if at_end { self.seek_transport(0.0); }
            self.video.is_playing = true;
            let has_audio = !self.audio.samples.is_empty() && self.audio.duration > 0.0;
            self.audio.is_playing = has_audio;
            if has_audio {
                self.sync_audio_controls();
                self.audio_engine.play_from_seconds(self.audio.playhead as f64);
            } else {
                self.audio_engine.pause();
            }
        } else {
            self.video.is_playing = false;
            self.audio.is_playing = false;
            self.audio_engine.pause();
        }
    }

    fn toggle_transport(&mut self) {
        let playing = self.video.is_playing || self.audio.is_playing;
        self.set_transport_playing(!playing);
    }

    /// Recall a saved editing workspace (panel sizes, dock tab, visibility).
    fn apply_layout(&mut self, idx: usize) {
        if idx >= self.layouts.len() { return; }
        let l = self.layouts[idx].clone();
        self.active_layout = idx;
        self.dock = l.dock;
        self.video.tl_zoom = self.video.tl_zoom.max(10.0);
        self.video.status = format!("Workspace: {}", l.name);
    }

    /// Mirror a preview/timeline seek into the audio engine and both clocks.
    fn apply_pending_video_seek(&mut self) {
        if let Some(t) = self.video.seek_request.take() {
            self.seek_transport(t);
        }
    }

    /// Type-aware addition from the media bin / internal drag-and-drop.
    fn add_bin_item_to_timeline(&mut self, name: &str, path: &std::path::Path, dur: f32) {
        let lane = self.gen_clip_track.min(self.video.video_track_count() - 1);
        self.add_bin_item_to_timeline_at(name, path, dur, lane, self.video.playhead);
    }

    fn render_splash_or_loading(&self, ctx: &egui::Context) {
        let screen = ctx.screen_rect();
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(Color32::TRANSPARENT).stroke(Stroke::NONE).inner_margin(egui::Margin::ZERO).outer_margin(egui::Margin::ZERO))
            .show(ctx, |ui| {
                let p = ui.painter();
                if let Some(bg) = &self.splash_bg_tex { p.image(bg.id(), screen, uv, Color32::WHITE); } else { p.rect_filled(screen, 0.0, BG); }
                p.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 110));
                let cx = screen.center().x; let cy = screen.center().y;
                match self.phase {
                    AppPhase::Splash => {
                        let fade = (self.phase_t / 0.6).min(1.0);
                        let alpha = (255.0 * fade) as u8;
                        if let Some(logo) = &self.logo_tex {
                            let lr = Rect::from_center_size(pos2(cx, cy - 30.0), vec2(150.0, 150.0));
                            p.image(logo.id(), lr, uv, Color32::from_rgba_unmultiplied(255, 255, 255, alpha));
                        }
                        p.text(pos2(cx, cy + 72.0), egui::Align2::CENTER_CENTER, "Grain", FontId::proportional(48.0), Color32::from_rgba_unmultiplied(236, 236, 236, alpha));
                        p.text(pos2(cx, cy + 112.0), egui::Align2::CENTER_CENTER, "AUDIO · VIDEO LAB", FontId::proportional(12.0), Color32::from_rgba_unmultiplied(150, 165, 185, alpha));
                        p.text(pos2(cx, screen.bottom() - 28.0), egui::Align2::CENTER_CENTER, "Loading…", FontId::proportional(11.0), Color32::from_rgba_unmultiplied(120, 120, 120, alpha));
                    }
                    AppPhase::Loading => {
                        p.text(pos2(cx, cy - 34.0), egui::Align2::CENTER_CENTER, "Grain", FontId::proportional(30.0), INK);
                        let prog = (self.phase_t / 1.8).min(1.0);
                        let bar_w = 280.0;
                        let bar = Rect::from_min_size(pos2(cx - bar_w / 2.0, cy + 6.0), vec2(bar_w, 6.0));
                        p.rect_filled(bar, 3.0, Color32::from_rgb(38, 38, 38));
                        p.rect_filled(Rect::from_min_size(bar.min, vec2(bar_w * prog, 6.0)), 3.0, ACCENT);
                        p.text(pos2(cx, cy + 28.0), egui::Align2::CENTER_CENTER, format!("Loading project… {:.0}%", prog * 100.0), FontId::proportional(11.0), SUB);
                    }
                    AppPhase::Ready => {}
                }
            });
    }

    fn draw_marlett(&self, p: &egui::Painter, rect: Rect, glyph: MarlettGlyph, color: Color32) {
        if self.marlett_loaded {
            let ch = match glyph { MarlettGlyph::Minimize => '0', MarlettGlyph::Maximize => '1', MarlettGlyph::Restore => '2', MarlettGlyph::Close => 'r' };
            p.text(rect.center(), egui::Align2::CENTER_CENTER, ch.to_string(), FontId::new(13.0, egui::FontFamily::Name("Marlett".into())), color);
        } else {
            let s = Stroke::new(1.4_f32, color);
            match glyph {
                MarlettGlyph::Minimize => { p.line_segment([pos2(rect.left() + 5.0, rect.center().y + 3.0), pos2(rect.right() - 5.0, rect.center().y + 3.0)], s); }
                MarlettGlyph::Maximize => { p.rect_stroke(rect.shrink(5.0), 1.0, s); }
                MarlettGlyph::Restore => {
                    let a = Rect::from_min_size(rect.min + vec2(5.0, 3.0), vec2(9.0, 7.0));
                    let b = Rect::from_min_size(rect.min + vec2(8.0, 6.0), vec2(9.0, 7.0));
                    p.rect_stroke(b, 1.0, s);
                    p.line_segment([a.left_top(), a.right_top()], s);
                    p.line_segment([a.left_top(), a.left_bottom()], s);
                }
                MarlettGlyph::Close => { let a = rect.shrink(5.0); p.line_segment([a.left_top(), a.right_bottom()], s); p.line_segment([a.right_top(), a.left_bottom()], s); }
            }
        }
    }

    fn save_current_project(&mut self) -> bool {
        let path = self.project_path.clone()
            .unwrap_or_else(|| media_engine::default_export_path("grain_project.otio"));
        self.save_project_to(&path)
    }

    fn save_project_to(&mut self, path: &std::path::Path) -> bool {
        let name = self.project_name.clone();
        let fps = self.video.fps;
        let mut acc = 0.0_f32;
        let mut clips: Vec<(String, f32, f32, f32)> = Vec::new();
        for c in &self.video.seq {
            let l = c.len(); clips.push((c.name.clone(), acc, l, c.speed)); acc += l;
        }
        for track in &self.video.extra_tracks {
            for c in track { clips.push((c.name.clone(), c.start, c.len(), c.speed)); }
        }
        if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
        match bananav_suite::export_project_interchange(path, &name, fps, &clips) {
            Ok(msg) => {
                self.project_path = Some(path.to_path_buf());
                self.project_dirty = false;
                self.video.status = msg;
                true
            }
            Err(e) => { self.video.status = format!("Save project failed: {e}"); false }
        }
    }

    fn close_project(&mut self, ctx: &egui::Context) {
        let close_window = self.pending_close_window;
        self.create_new_project(ctx);
        self.close_project_open = false;
        self.pending_close_window = false;
        if close_window { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
    }

    fn create_new_project(&mut self, ctx: &egui::Context) {
        let name = self.new_project_draft.name.clone();
        let width = self.new_project_draft.width.max(16) & !1;
        let height = self.new_project_draft.height.max(16) & !1;
        let fps = self.new_project_draft.fps.clamp(1.0, 240.0);
        let sample_rate = self.new_project_draft.sample_rate.max(8000);
        let save_path = self.new_project_draft.save_path.trim().to_string();

        self.video = VideoState::default();
        self.audio = AudioState::default();
        self.timeline_ui = TimelineUi::default();
        self.layouts = WorkspaceLayout::defaults();
        self.active_layout = 0;
        self.video.loaded = true;
        self.video.is_demo = false;
        self.video.filename = name.clone();
        self.video.fw = width as usize; self.video.fh = height as usize;
        self.video.source_w = width; self.video.source_h = height;
        self.video.fps = fps; self.video.duration = 1.0; self.video.playhead = 0.0;
        self.video.dirty = true;
        self.audio.sample_rate = sample_rate;
        self.audio.channels = 2; self.audio.duration = 0.0; self.audio.samples.clear(); self.audio.loaded = false; self.audio.is_demo = false;
        self.audio.playhead = 0.0; self.audio.sel_start = 0.0; self.audio.sel_end = 0.0;
        self.plugins.set_sample_rate(sample_rate);
        self.project_name = name;
        self.project_path = if save_path.is_empty() { None } else { Some(std::path::PathBuf::from(save_path)) };
        self.project_dirty = false;
        self.video.status = format!("New project · {}×{} · {:.3} fps · {} Hz", width, height, fps, sample_rate);
        self.reload_playback_buffer();
        self.new_project_open = false;
        let _ = ctx;
    }

    fn render_project_popups(&mut self, ctx: &egui::Context) {
        if self.close_project_open {
            let mut open = true;
            egui::Window::new("Close Project").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0,0.0]).open(&mut open).show(ctx, |ui| {
                ui.label(RichText::new(format!("Close “{}”?", self.project_name)).color(INK).size(12.0));
                ui.label(RichText::new("Save changes, discard them, or cancel.").color(SUB).size(10.0));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        if self.save_current_project() { self.close_project(ctx); }
                    }
                    if ui.button("Don't Save").clicked() { self.close_project(ctx); }
                    if ui.button("Cancel").clicked() { self.close_project_open = false; self.pending_close_window = false; }
                });
            });
            if !open { self.close_project_open = false; self.pending_close_window = false; }
        }

        if self.new_project_open {
            let mut open = true;
            egui::Window::new("New Project").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0,0.0]).default_width(360.0).min_width(360.0).max_width(360.0).open(&mut open).show(ctx, |ui| {
                ui.label(RichText::new("Project properties").color(ACCENT).size(11.0));
                let d = &mut self.new_project_draft;
                ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut d.name); });
                ui.horizontal(|ui| { ui.label("Width"); ui.add(egui::DragValue::new(&mut d.width).speed(1).clamp_range(16..=7680)); });
                ui.horizontal(|ui| { ui.label("Height"); ui.add(egui::DragValue::new(&mut d.height).speed(1).clamp_range(16..=4320)); });
                ui.horizontal(|ui| { ui.label("Frame rate"); ui.add(egui::DragValue::new(&mut d.fps).speed(0.01).clamp_range(1.0..=240.0)); });
                ui.horizontal(|ui| { ui.label("Audio rate"); ui.add(egui::DragValue::new(&mut d.sample_rate).speed(1).clamp_range(8000..=192000)); });
                ui.horizontal(|ui| { ui.label("Save to"); ui.text_edit_singleline(&mut d.save_path); });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() { self.create_new_project(ctx); }
                    if ui.button("Cancel").clicked() { self.new_project_open = false; }
                });
            });
            if !open { self.new_project_open = false; }
        }
    }

    fn render_go_to_time_popup(&mut self, ctx: &egui::Context) {
        if !self.go_to_time_open { return; }
        let mut open = true;
        egui::Window::new("Go to Time")
            .collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open).default_width(300.0).min_width(300.0).max_width(300.0).show(ctx, |ui| {
                ui.label(RichText::new("Jump playhead to exact timecode").color(SUB).size(10.0));
                let response = ui.add(egui::TextEdit::singleline(&mut self.go_to_time_input)
                    .hint_text(format!("{} or seconds", fmt_tc_frames(self.video.playhead))).desired_width(ui.available_width()));
                ui.label(RichText::new(format!("fps {:.3} · total {}", self.video.fps, fmt_tc_frames(self.video.total()))).color(SUB).size(9.5));
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Go").clicked() || enter {
                    if let Some(t) = parse_timecode(&self.go_to_time_input, self.video.fps) {
                        self.seek_transport(t.min(self.timeline_end()));
                        self.go_to_time_open = false;
                    }
                }
                if ui.button("Cancel").clicked() { self.go_to_time_open = false; }
            });
        if !open { self.go_to_time_open = false; }
    }

    fn render_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("grain_top")
            .exact_height(46.0)
            .frame(egui::Frame::default().fill(PANEL2).stroke(Stroke::new(1.0_f32, LINE)).inner_margin(egui::Margin::symmetric(10.0, 0.0)))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let p = ui.painter().clone();
                let cy = rect.center().y;
                // Grain wordmark
                p.text(pos2(rect.left() + 2.0, cy), egui::Align2::LEFT_CENTER, "Grain", FontId::proportional(16.0), INK);
                p.text(pos2(rect.left() + 58.0, cy), egui::Align2::LEFT_CENTER, "NLE", FontId::proportional(10.0), ACCENT);
                // Vegas-style project toolbar.
                let nr = Rect::from_min_size(pos2(rect.left() + 90.0, cy - 10.0), vec2(82.0, 20.0));
                if ui.put(nr, egui::Button::new(RichText::new("New Project").size(9.5))).clicked() {
                    self.new_project_draft.name = format!("{} Copy", self.project_name);
                    self.new_project_draft.width = self.video.source_w.max(16);
                    self.new_project_draft.height = self.video.source_h.max(16);
                    self.new_project_draft.fps = self.video.fps;
                    self.new_project_draft.sample_rate = self.audio.sample_rate;
                    self.new_project_draft.save_path.clear();
                    self.new_project_open = true;
                }
                let cr = Rect::from_min_size(pos2(rect.left() + 176.0, cy - 10.0), vec2(92.0, 20.0));
                if ui.put(cr, egui::Button::new(RichText::new("Close Project").size(9.5))).clicked() {
                    self.pending_close_window = false; self.close_project_open = true;
                }
                p.text(pos2(rect.left() + 272.0, cy), egui::Align2::LEFT_CENTER, &self.project_name, FontId::proportional(9.5), SUB);
                // transport cluster
                let mut x = rect.left() + 392.0;
                let total = self.video.total();
                for (icon, id) in [("⏮", "tr_home"), ("⏪", "tr_back"), ("▶", "tr_play"), ("⏩", "tr_fwd"), ("⏭", "tr_end")] {
                    let r = Rect::from_center_size(pos2(x + 14.0, cy), vec2(28.0, 28.0));
                    let resp = ui.interact(r, Id::new(id), Sense::click());
                    if resp.hovered() { p.rect_filled(r, 4.0, LINE); }
                    let label = if id == "tr_play" { if self.video.is_playing { "❚❚" } else { "▶" } } else { icon };
                    let col = if id == "tr_play" { ACCENT } else if resp.hovered() { INK } else { SUB };
                    p.text(r.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(14.0), col);
                    if resp.clicked() {
                        match id {
                            "tr_home" => self.seek_transport(0.0),
                            "tr_end" => self.seek_transport(total),
                            "tr_play" => self.toggle_transport(),
                            "tr_back" => self.seek_transport(self.video.playhead - 1.0 / self.video.fps.max(1.0)),
                            "tr_fwd" => self.seek_transport(self.video.playhead + 1.0 / self.video.fps.max(1.0)),
                            _ => {}
                        }
                    }
                    x += 30.0;
                }
                // big timecode
                p.text(pos2(x + 12.0, cy), egui::Align2::LEFT_CENTER, fmt_tc_frames(self.video.playhead), FontId::monospace(18.0), INK);
                p.text(pos2(x + 12.0, cy + 12.0), egui::Align2::LEFT_TOP, format!("{:.0} fps UI · {:.0} fps video · dur {}", self.fps_smoothed.max(0.0), self.video.fps, fmt_tc_frames(total)), FontId::proportional(9.0), SUB);
                // Workspace layout switcher — never auto-restyles or resizes
                // the interface when a different dock tab is selected.
                let lr = Rect::from_min_size(pos2(rect.right() - 154.0, cy - 11.0), vec2(88.0, 22.0));
                let lresp = ui.interact(lr, Id::new("ws_layout_btn"), Sense::click());
                if lresp.hovered() { p.rect_filled(lr, 4.0, LINE); }
                let lname = self.layouts.get(self.active_layout).map(|l| l.name.as_str()).unwrap_or("Editing");
                p.rect_stroke(lr, 4.0, Stroke::new(1.0_f32, LINE));
                p.text(lr.center(), egui::Align2::CENTER_CENTER, format!("▦ {}", lname), FontId::proportional(9.5), ACCENT);
                if lresp.clicked() { self.layout_window_open = !self.layout_window_open; }

                // Marlett window controls (far right) — real viewport commands.
                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(self.window_maximized));
                self.window_maximized = maximized;
                let max_glyph = if maximized { MarlettGlyph::Restore } else { MarlettGlyph::Maximize };
                let mut xc = rect.max.x;
                for (glyph, col, id) in [
                    (MarlettGlyph::Close, ACCENT2, "win_close"),
                    (max_glyph, INK, "win_max"),
                    (MarlettGlyph::Minimize, INK, "win_min"),
                ] {
                    let r = Rect::from_min_size(pos2(xc - 20.0, rect.min.y + 7.0), vec2(20.0, 30.0));
                    let resp = ui.interact(r, Id::new(id), Sense::click());
                    if resp.hovered() { p.rect_filled(r, 2.0, if id == "win_close" { Color32::from_rgb(0xc4, 0x2f, 0x34) } else { LINE }); }
                    self.draw_marlett(&p, r, glyph, col);
                    if resp.clicked() {
                        match id {
                            "win_close" => { self.pending_close_window = true; self.close_project_open = true; }
                            "win_min" => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                            "win_max" => {
                                self.window_maximized = !maximized;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                            }
                            _ => {}
                        }
                    }
                    xc -= 20.0;
                }
            });
        self.render_project_popups(ctx);
        self.render_go_to_time_popup(ctx);
    }
}

// ===========================================================================
// eframe::App
// ===========================================================================
impl eframe::App for GrainApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_assets(ctx);
        let mut v = egui::Visuals::dark();
        v.panel_fill = BG;
        v.window_fill = PANEL;
        v.faint_bg_color = PANEL;
        v.extreme_bg_color = TLBG;
        v.widgets.noninteractive.bg_fill = PANEL;
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, INK);
        v.widgets.inactive.bg_fill = TBBG;
        v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, INK);
        v.widgets.hovered.bg_fill = LINE;
        v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, INK);
        v.widgets.active.bg_fill = ACCENT;
        v.widgets.active.fg_stroke = Stroke::new(1.0_f32, CLIP_TXT);
        v.selection.bg_fill = ACCENT;
        v.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(120, 220, 210));
        v.window_stroke = Stroke::new(1.0_f32, LINE);
        ctx.set_visuals(v);
        ctx.style_mut(|s| {
            s.spacing.scroll.floating = false;
            s.spacing.scroll.bar_width = 10.0;
            s.spacing.scroll.bar_inner_margin = 2.0;
            s.spacing.scroll.bar_outer_margin = 0.0;
            s.spacing.scroll.handle_min_length = 18.0;
            s.spacing.slider_width = 130.0;
            s.spacing.item_spacing = vec2(6.0, 4.0);
        });

        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        if dt > 0.0001 {
            let instant_fps = 1.0 / dt;
            self.fps_smoothed = if self.fps_smoothed <= 0.0 { instant_fps } else { self.fps_smoothed * 0.92 + instant_fps * 0.08 };
        }
        self.phase_t += dt;
        match self.phase {
            AppPhase::Splash => { if self.phase_t >= 2.6 { self.phase = AppPhase::Loading; self.phase_t = 0.0; } }
            AppPhase::Loading => { if self.phase_t >= 1.8 { self.phase = AppPhase::Ready; self.phase_t = 0.0; } }
            AppPhase::Ready => {}
        }
        if self.phase != AppPhase::Ready {
            self.render_splash_or_loading(ctx);
            ctx.request_repaint();
            return;
        }
        self.apply_pending_video_seek();
        self.maybe_rebuild_timeline_audio();

        if !ctx.wants_keyboard_input() {
            let (space, home, end, left, right, alt) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::Space),
                    i.key_pressed(egui::Key::Home),
                    i.key_pressed(egui::Key::End),
                    i.key_pressed(egui::Key::ArrowLeft),
                    i.key_pressed(egui::Key::ArrowRight),
                    i.modifiers.alt,
                )
            });
            let frame = 1.0 / self.video.fps.max(1.0);
            if space { self.toggle_transport(); }
            if home { self.seek_transport(0.0); }
            if end { self.seek_transport(self.timeline_end().max(self.audio.duration)); }
            if alt && (left || right) && !self.timeline_ui.selection.is_empty() {
                self.nudge_selected(if left { -frame } else { frame });
            } else {
                if left { self.seek_transport(self.video.playhead - frame); }
                if right { self.seek_transport(self.video.playhead + frame); }
            }

            // Premiere-style single-key tools + edit shortcuts.
            let mut new_tool = None;
            for key in [egui::Key::V, egui::Key::C, egui::Key::U, egui::Key::Y, egui::Key::Z, egui::Key::H] {
                if ctx.input(|i| i.key_pressed(key)) { new_tool = TimelineTool::from_key(key); }
            }
            if let Some(t) = new_tool { self.timeline_ui.tool = t; }
            let keys = ctx.input(|i| {
                let cmd = i.modifiers.command;
                let del = i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace);
                (
                    i.key_pressed(egui::Key::S),
                    i.key_pressed(egui::Key::M),
                    del && !i.modifiers.shift,
                    del && i.modifiers.shift,
                    i.key_pressed(egui::Key::D) && cmd,
                    i.key_pressed(egui::Key::A) && cmd,
                    i.key_pressed(egui::Key::B) && cmd,
                )
            });
            if keys.0 { self.timeline_ui.snap = !self.timeline_ui.snap; }
            if keys.1 { let t = self.video.playhead; self.video.add_marker(t); }
            if keys.2 { self.delete_selected(true); }
            if keys.3 { self.delete_selected(false); }
            if keys.4 { self.duplicate_selected(); }
            if keys.6 { self.split_at_playhead(); }
            if keys.5 {
                let (_, _, refs) = self.timeline_snapshot();
                self.timeline_ui.set_selection(refs.keys().copied().collect());
            }

            // Vegas-style keyboard navigation + grouping.
            let (go_to_time, group, ungroup) = ctx.input(|i| {
                let cmd = i.modifiers.command;
                let g = i.key_pressed(egui::Key::G);
                (g && cmd && !i.modifiers.shift && !i.modifiers.alt,
                 g && cmd && i.modifiers.shift && !i.modifiers.alt,
                 g && cmd && i.modifiers.shift && i.modifiers.alt)
            });
            if go_to_time {
                self.go_to_time_input = fmt_tc_frames(self.video.playhead);
                self.go_to_time_open = true;
            }
            if group { self.group_selected(); }
            if ungroup { self.ungroup_selected(); }

            let (zoom_in, zoom_out) = ctx.input(|i| (
                i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals),
                i.key_pressed(egui::Key::Minus),
            ));
            if zoom_in { self.timeline_ui.zoom = (self.timeline_ui.zoom * 1.2).clamp(8.0, 400.0); self.video.tl_zoom = self.timeline_ui.zoom; }
            if zoom_out { self.timeline_ui.zoom = (self.timeline_ui.zoom / 1.2).clamp(8.0, 400.0); self.video.tl_zoom = self.timeline_ui.zoom; }
        }
        self.sync_audio_controls();
        let transport_was_playing = self.video.is_playing || self.audio.is_playing;
        self.audio_engine.advance(dt);
        self.audio.tick(dt, &self.audio_engine);
        self.video.tick(dt, ctx);
        if self.audio.is_playing && !self.audio.samples.is_empty() {
            // Hardware audio clock is the master clock for A/V sync.
            self.video.playhead = self.audio.playhead.min(self.video.total());
            if self.video.playhead >= self.video.total() - 0.001 {
                self.set_transport_playing(false);
            }
        }
        // The CPAL callback stops itself at the end of the PCM buffer. Mirror
        // that into the video transport so a short score never leaves preview
        // running silently to the end of a longer timeline.
        if transport_was_playing
            && !self.audio.samples.is_empty()
            && self.audio.duration > 0.0
            && !self.audio_engine.is_playing()
            && self.audio.playhead >= self.audio.duration - 0.02
        {
            self.set_transport_playing(false);
        }
        if self.video.is_playing || self.audio.is_playing {
            // Drive the transport independently of mouse motion at 120 Hz.
            ctx.request_repaint_after(std::time::Duration::from_millis(8));
        }

        // Drag-and-drop media & OpenFX import: hover overlay + drop handler
        if self.drag_drop_enabled {
            let hovered = ctx.input(|i| i.raw.hovered_files.clone());
            if !hovered.is_empty() {
                let screen = ctx.screen_rect();
                let fg = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    Id::new("grain_drop_overlay"),
                ));
                fg.rect_filled(
                    screen.shrink(10.0),
                    8.0,
                    Color32::from_rgba_unmultiplied(16, 22, 30, 215),
                );
                fg.rect_stroke(
                    screen.shrink(10.0),
                    8.0,
                    Stroke::new(2.5_f32, ACCENT),
                );
                fg.text(
                    screen.center(),
                    egui::Align2::CENTER_CENTER,
                    "⬇ Drop Video (.mp4/.mov/.mkv), Audio (.wav/.mp3/.flac), Image (.png/.jpg), or OpenFX (.ofx.bundle)",
                    FontId::proportional(17.0),
                    ACCENT,
                );
            }
            let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
            for p in dropped { self.load_media(&p); }
        }

        self.render_top_bar(ctx);

        // Editing workspaces. Panel IDs include the active layout index so a
        // recalled preset applies its stored sizes on the very next frame.
        let li = self.active_layout.min(self.layouts.len().saturating_sub(1));
        let layout = self.layouts[li].clone();

        // Bottom: multi-track timeline (full width, resizable)
        let mut tl_imported: Option<std::path::PathBuf> = None;
        if layout.timeline_open {
            let resp = egui::TopBottomPanel::bottom(format!("grain_timeline_{}", li))
                .resizable(false)
                .min_height(120.0)
                .default_height(layout.timeline_height.max(120.0))
                .frame(egui::Frame::default().fill(PANEL).stroke(Stroke::new(1.0_f32, LINE)).inner_margin(egui::Margin::same(0.0)))
                .show(ctx, |ui| { tl_imported = self.render_timeline(ui); });
            self.layouts[li].timeline_height = resp.response.rect.height();
        }
        if let Some(p) = tl_imported {
            self.load_media(&p);
        }

        // Left dock: dynamically sized by the active workspace.
        if layout.left_open {
            let resp = egui::SidePanel::left(format!("grain_dock_{}", li))
                .resizable(false)
                .min_width(200.0)
                .default_width(layout.left_width.max(200.0))
                .frame(egui::Frame::default().fill(PANEL).stroke(Stroke::new(1.0_f32, LINE)).inner_margin(egui::Margin::same(0.0)))
                .show(ctx, |ui| { self.render_left_dock(ui, ctx); });
            self.layouts[li].left_width = resp.response.rect.width();
        }

        // Right dock: properties inspector (video grade).
        if layout.right_open {
            let resp = egui::SidePanel::right(format!("grain_inspector_{}", li))
                .resizable(false)
                .min_width(220.0)
                .default_width(layout.right_width.max(220.0))
                .frame(egui::Frame::default().fill(PANEL).stroke(Stroke::new(1.0_f32, LINE)).inner_margin(egui::Margin::same(0.0)))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        self.video.render_inspector(ui);
                    });
                });
            self.layouts[li].right_width = resp.response.rect.width();
        }

        // Center: preview
        let mut preview_rect = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(BG).inner_margin(egui::Margin::same(8.0)))
            .show(ctx, |ui| { preview_rect = Some(self.video.render_preview(ui, ctx)); });
        self.preview_drop_rect = preview_rect;
        self.apply_pending_video_seek();

        // Workspace layout editor: presets + toggles + save current arrangement.
        if self.layout_window_open {
            let li = self.active_layout.min(self.layouts.len().saturating_sub(1));
            egui::Window::new("Workspace Layouts")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::RIGHT_TOP, vec2(-78.0, 54.0))
                .show(ctx, |ui| {
                    ui.label(RichText::new("Editing layouts").color(INK).size(12.0));
                    ui.label(RichText::new("Panel sizes are saved as you resize them.").color(SUB).size(9.5));
                    ui.separator();
                    let names: Vec<String> = self.layouts.iter().map(|l| l.name.clone()).collect();
                    for (i, name) in names.iter().enumerate() {
                        let active = i == li;
                        if ui.add(egui::Button::new(RichText::new(format!("{}  {}", if active { "●" } else { "○" }, name)).color(if active { CLIP_TXT } else { INK }).size(10.5))
                            .fill(if active { ACCENT } else { Color32::TRANSPARENT })
                            .stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0))
                            .min_size(vec2(ui.available_width().max(120.0) - 8.0, 22.0))).clicked() {
                            self.apply_layout(i);
                        }
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        let l = &mut self.layouts[li];
                        ui.checkbox(&mut l.left_open, "Project dock");
                        ui.checkbox(&mut l.right_open, "Inspector");
                    });
                    ui.horizontal(|ui| {
                        let l = &mut self.layouts[li];
                        ui.checkbox(&mut l.timeline_open, "Timeline");
                        if ui.small_button("Close").clicked() { self.layout_window_open = false; }
                    });
                    ui.separator();
                    if ui.add(egui::Button::new(RichText::new("💾 Save Current Layout").color(Color32::WHITE).size(10.5)).fill(ACCENT2).rounding(Rounding::same(4.0))).clicked() {
                        let l = &mut self.layouts[li];
                        l.dock = self.dock;
                        l.name = if l.name.starts_with("Custom") { l.name.clone() } else { format!("Custom {}", li + 1) };
                        self.video.status = format!("Saved workspace {}", l.name);
                    }
                });
        }

        // Handle internal Drag & Drop of Media Bin clips or Video FX / OpenFX onto Preview or Timeline
        if self.drag_drop_enabled {
            if let Some(bi) = self.dragging_bin_idx {
                if let Some((bname, _, bdur)) = self.media_bin.get(bi) {
                    if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                        let fg = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("bin_drag_tooltip")));
                        let r = Rect::from_min_size(pos + vec2(12.0, 12.0), vec2(210.0, 22.0));
                        fg.rect_filled(r, 4.0, ACCENT);
                        fg.text(r.center(), egui::Align2::CENTER_CENTER, format!("+ Drop Clip to Timeline: {} ({:.1}s)", bname, bdur), FontId::proportional(10.5), Color32::BLACK);
                    }
                }
            }
            if let Some(fx_lbl) = &self.dragging_fx_label {
                if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                    let fg = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("fx_drag_tooltip")));
                    let r = Rect::from_min_size(pos + vec2(12.0, 12.0), vec2(180.0, 22.0));
                    fg.rect_filled(r, 4.0, ACCENT2);
                    fg.text(r.center(), egui::Align2::CENTER_CENTER, format!("+ Drop FX: {}", fx_lbl), FontId::proportional(10.5), Color32::WHITE);
                }
            }
            if ctx.input(|i| i.pointer.any_released()) {
                let release_pos = ctx.input(|i| i.pointer.hover_pos());
                let over_preview = release_pos.map(|p| self.preview_drop_rect.map(|r| r.contains(p)).unwrap_or(false)).unwrap_or(false);
                let over_timeline = release_pos.map(|p| self.video.timeline_drop_rect.map(|r| r.contains(p)).unwrap_or(false)).unwrap_or(false);
                let over_drop_zone = over_preview || over_timeline;
                if let Some(bi) = self.dragging_bin_idx.take() {
                    if let Some((bname, bpath, bdur)) = self.media_bin.get(bi).cloned() {
                        if over_drop_zone {
                            if over_timeline {
                                if let Some((lane, t)) = self.tl_drop_target {
                                    self.add_bin_item_to_timeline_at(&bname, &bpath, bdur, lane, t);
                                } else {
                                    self.add_bin_item_to_timeline(&bname, &bpath, bdur);
                                }
                            } else {
                                self.add_bin_item_to_timeline(&bname, &bpath, bdur);
                            }
                            self.video.status = format!("Dropped {} onto timeline", bname);
                        } else {
                            self.video.status = "Drop cancelled — release over Preview or Timeline.".into();
                        }
                    }
                }
                if let Some(fx_lbl) = self.dragging_fx_label.take() {
                    if over_drop_zone { self.apply_video_fx_preset(&fx_lbl); }
                }
            }
        }
    }
}

// ===========================================================================
// Left dock (Media / Audio FX / Video FX / Titles / Export)
// ===========================================================================
impl GrainApp {
    fn render_left_dock(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.vertical(|ui| {
            // tab bar
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                for &t in DockTab::ALL {
                    let active = self.dock == t;
                    let btn = egui::Button::new(RichText::new(t.label()).color(if active { CLIP_TXT } else { SUB }).size(11.0))
                        .fill(if active { ACCENT } else { Color32::TRANSPARENT })
                        .stroke(if active { Stroke::NONE } else { Stroke::new(1.0_f32, LINE) })
                        .rounding(Rounding::same(4.0));
                    if ui.add(btn).clicked() { self.dock = t; }
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.dock {
                DockTab::ProjectMedia => self.render_media_tab(ui),
                DockTab::MediaGenerator => self.render_generator_tab(ui),
                DockTab::Transitions => self.render_transitions_tab(ui),
                DockTab::VideoFx => self.render_video_fx_tab(ui),
                DockTab::ColorGrading => self.render_color_grading_tab(ui),
                DockTab::AudioFx => {}
                DockTab::Titles => self.render_titles_tab(ui),
                DockTab::Ofx => self.render_ofx_tab(ui),
                DockTab::Plugins => self.render_plugins_tab(ui),
                DockTab::Export => self.render_export_tab(ui, ctx),
            });
        });
        if self.dock == DockTab::AudioFx { self.render_audio_tab_window(ctx); }
    }

    /// Audio FX rewritten as a standalone `egui` window rather than a cramped
    /// dock strip. The left dock keeps its current size and layout.
    fn render_audio_tab_window(&mut self, ctx: &egui::Context) {
        egui::Window::new("Audio FX")
            .collapsible(true)
            .resizable(false)
            .default_width(460.0)
            .default_height(560.0)
            .min_width(460.0)
            .max_width(460.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let mut seek = None;
                    self.audio.render_rack(
                        ui,
                        ctx,
                        &mut self.plugins,
                        &mut self.video.track_mute[2],
                        &mut self.video.music,
                        &self.audio_engine,
                        &mut seek,
                    );
                    self.video.is_playing = self.audio.is_playing;
                    self.video.track_solo[2] = self.audio.solo;
                    if let Some(t) = seek { self.seek_transport(t); }
                });
            });
    }

    /// VST2 / VST3 / Apple AudioUnit browser and insertion chain.
    ///
    /// Discovery is manual (scanning plug-ins can load third-party code), but
    /// everything after that is real: slots instantiate the plug-in, expose its
    /// parameters, and the chain is rendered into the Audio FX WAV/export path.
    fn render_plugins_tab(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Audio Plug-in Host").color(INK).size(13.0));
        ui.label(RichText::new("VST2 · VST3 · Apple Audio Units · real audio processing").color(SUB).size(10.0));
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 3.0);
            let all = PluginFormat::ALL;
            if ui.add(egui::Button::new(RichText::new("↻ Rescan all").color(Color32::WHITE).size(10.0)).fill(ACCENT).rounding(Rounding::same(3.0))).clicked() {
                let note = self.plugins.scan(&all);
                self.video.status = format!("Plug-in scan — {note}");
                self.audio.status = self.video.status.clone();
            }
            for fmt in all {
                let label = match fmt {
                    PluginFormat::Vst2 => "VST2",
                    PluginFormat::Vst3 => "VST3",
                    PluginFormat::Au => "AU",
                };
                if ui.add(egui::Button::new(RichText::new(label).color(INK).size(10.0)).fill(TBBG).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0))).clicked() {
                    let note = self.plugins.scan(&[fmt]);
                    self.video.status = format!("{label} scan — {note}");
                    self.audio.status = self.video.status.clone();
                }
            }
            if ui.add(egui::Button::new(RichText::new("＋ Path").color(INK).size(10.0)).fill(TBBG).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0))).clicked() {
                if let Some(p) = media_engine::pick_folder_dialog("Add VST2 / VST3 plug-in folder") {
                    if !self.plugins.scan_paths.contains(&p) {
                        self.plugins.scan_paths.push(p);
                    }
                    self.video.status = format!("Added scan path: {}", self.plugins.scan_paths.last().map(|p| p.display().to_string()).unwrap_or_default());
                }
            }
        });
        if !self.plugins.last_scan.is_empty() {
            ui.add_space(2.0);
            ui.label(RichText::new(&self.plugins.last_scan).color(ACCENT).size(10.0));
        } else {
            ui.label(RichText::new("Scan once to populate the browser. Default OS plug-in folders are searched automatically.").color(SUB).size(9.5));
        }

        ui.add_space(6.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new("FX Chain").color(ACCENT).size(11.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(format!("{} active", self.plugins.active_count())).color(SUB).size(9.5));
            });
        });
        ui.label(RichText::new("Selected slot receives plug-ins from the browser below.").color(SUB).size(9.5));

        for i in 0..self.plugins.slots.len() {
            let (name, status, enabled, has_desc, has_error) = {
                let slot = &self.plugins.slots[i];
                (
                    slot.descriptor.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| "— empty —".into()),
                    slot.status(),
                    slot.enabled,
                    slot.descriptor.is_some(),
                    slot.last_error.is_some(),
                )
            };
            let selected = self.plugin_slot_sel == i;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(3.0, 2.0);
                let sel = egui::Button::new(RichText::new(format!("{:02}", i + 1)).monospace().size(10.0).color(if selected { CLIP_TXT } else { SUB }))
                    .fill(if selected { ACCENT2 } else { TBBG })
                    .rounding(Rounding::same(3.0))
                    .min_size(vec2(24.0, 18.0));
                if ui.add(sel).clicked() { self.plugin_slot_sel = i; }
                let mut enabled = enabled;
                if has_desc && ui.checkbox(&mut enabled, "").clicked() {
                    self.plugins.set_enabled(i, enabled);
                }
                let col = if !has_desc { SUB } else if has_error { AMBER } else { INK };
                ui.add(egui::Label::new(RichText::new(name).color(col).size(10.5)).truncate());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if has_desc && ui.add(egui::Button::new(RichText::new("×").color(SUB).size(11.0)).fill(Color32::TRANSPARENT).rounding(Rounding::same(3.0))).clicked() {
                        self.plugins.remove(i);
                    }
                    ui.add(egui::Label::new(RichText::new(status).color(SUB).size(9.0)).truncate());
                });
            });
        }

        let sel = self.plugin_slot_sel.min(self.plugins.slots.len() - 1);
        let slot_desc = self.plugins.slots[sel].descriptor.clone();
        ui.add_space(4.0);
        if let Some(desc) = &slot_desc {
            ui.collapsing(RichText::new(format!("Parameters — {}", desc.name)).color(ACCENT).size(10.5), |ui| {
                let params: Vec<_> = self.plugins.slots[sel].params.iter().cloned().collect();
                if params.is_empty() {
                    ui.label(RichText::new("This plug-in exposes no automatable parameters.").color(SUB).size(9.5));
                }
                for p in params {
                    if p.max > p.min {
                        let mut v = p.value;
                        if ui.add(Slider::new(&mut v, p.min..=p.max).text(&p.name)).changed() {
                            self.plugins.set_param(sel, p.id, v);
                        }
                    } else {
                        ui.label(RichText::new(format!("{}: {:.3}", p.name, p.value)).color(SUB).size(10.0));
                    }
                }
            });
        }

        ui.add_space(6.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Browser").color(ACCENT).size(11.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(format!("{} found", self.plugins.descriptors.len())).color(SUB).size(9.5));
            });
        });
        ui.add(egui::TextEdit::singleline(&mut self.plugin_filter).hint_text("Filter plug-ins…").desired_width(ui.available_width()));

        let filter = self.plugin_filter.trim().to_lowercase();
        let rows: Vec<(usize, String, String, String)> = self.plugins.descriptors.iter().enumerate()
            .filter(|(_, d)| {
                filter.is_empty()
                    || d.name.to_lowercase().contains(&filter)
                    || d.vendor.to_lowercase().contains(&filter)
                    || d.format.label().to_lowercase().contains(&filter)
            })
            .map(|(i, d)| {
                let sub = if d.vendor.is_empty() {
                    format!("{} · {}", d.format.label(), d.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                } else {
                    format!("{} · {}", d.format.label(), d.vendor)
                };
                (i, d.name.clone(), sub, d.format.label().to_string())
            })
            .collect();

        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(240.0).show(ui, |ui| {
            for (idx, name, sub, fmt) in rows {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
                    ui.label(RichText::new(format!("{}", fmt)).color(match fmt.as_str() { "VST2" => CLIP_V, "VST3" => CLIP_O, _ => CLIP_M }).size(9.0).strong());
                    ui.vertical(|ui| {
                        ui.add(egui::Label::new(RichText::new(name).color(INK).size(10.5)).truncate());
                        ui.add(egui::Label::new(RichText::new(sub).color(SUB).size(9.0)).truncate());
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new("＋").color(Color32::WHITE).size(11.0)).fill(ACCENT2).rounding(Rounding::same(3.0)).min_size(vec2(22.0, 20.0))).clicked() {
                            let sr = self.audio.sample_rate;
                            self.plugins.set_sample_rate(sr);
                            let desc = self.plugins.descriptors[idx].clone();
                            let target = self.plugin_slot_sel;
                            match self.plugins.add(target, &desc) {
                                Ok(_) => {
                                    self.video.status = format!("Loaded {} → slot {}", desc.name, target + 1);
                                    self.audio.status = self.video.status.clone();
                                }
                                Err(e) => {
                                    self.video.status = format!("Could not load {}: {e}", desc.name);
                                    self.audio.status = self.video.status.clone();
                                }
                            }
                        }
                    });
                });
                ui.separator();
            }
        });
        ui.add_space(3.0);
        ui.label(RichText::new("Plug-ins are rendered in slot order when applying Audio FX or exporting WAV.").color(SUB).size(9.0));
    }

    /// Import a media file by path (also used by drag-and-drop). Dispatches by
    /// extension: audio -> real `media_engine` decode into the Audio FX rack;
    /// video -> `ContinuousVideoStream` (ffmpeg) feeds the preview + timeline;
    /// image -> decoded once as the source frame.
    fn load_media(&mut self, path: &std::path::Path) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("media").to_string();
        let name_lower = name.to_lowercase();
        if name_lower.ends_with(".ofx.bundle") || name_lower.ends_with(".ofx") {
            let mac_bin = if path.is_dir() {
                path.join("Contents")
                    .join("MacOS")
                    .join(format!("{}.ofx", name.trim_end_matches(".ofx.bundle")))
            } else {
                path.to_path_buf()
            };
            let mut loaded = ofx_host::inspect_ofx_binary(&name, path, &mac_bin);
            for plug in &mut loaded {
                let (clean, _) = ofx_host::format_ofx_display_name(&plug.identifier, &plug.bundlename);
                plug.identifier = clean;
            }
            let count = loaded.len();
            self.ofx_plugins.extend(loaded);
            self.ofx_scanned = true;
            if self.ofx_sel.is_none() && !self.ofx_plugins.is_empty() {
                self.ofx_sel = Some(self.ofx_plugins.len() - 1);
            }
            self.video.status = format!("Loaded {} OpenFX plugin(s) from {}", count, name);
            self.dock = DockTab::Ofx;
            return;
        }
        self.clear_demo_if_needed();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if matches!(ext.as_str(), "wav" | "mp3" | "flac" | "ogg" | "oga" | "m4a" | "aac") {
            match media_engine::decode_audio_stereo(path) {
                Ok((l, r, sr)) => {
                    let n = l.len().min(r.len());
                    let mut inter = Vec::with_capacity(n * 2);
                    for i in 0..n { inter.push(l[i]); inter.push(r[i]); }
                    let dur = (n as f32 / sr.max(1) as f32).max(0.1);
                    self.audio.samples = inter;
                    self.audio.sample_rate = sr;
                    self.plugins.set_sample_rate(sr);
                    self.audio.channels = 2;
                    self.audio.duration = dur;
                    self.audio.filename = name.clone();
                    self.audio.loaded = true;
                    self.audio.is_demo = false;
                    self.audio.playhead = 0.0;
                    self.audio.sel_start = 0.0;
                    self.audio.sel_end = dur;
                    self.audio.status = format!("Imported {} · {:.1}s · {} Hz · {} smp", name, dur, sr, n);
                    if !self.append_timeline_mode { self.video.music.clear(); }
                    self.video.music.push(MusicClip {
                        uid: fresh_clip_uid(),
                        nm: name.clone(), dur, start: self.video.playhead, vol: 80.0,
                        track: 0, path: Some(path.to_path_buf()),
                    });
                    if !self.media_bin.iter().any(|(_, p, _)| p == path) {
                        self.media_bin.push((name.clone(), path.to_path_buf(), dur));
                    }
                    self.reload_playback_buffer();
                    self.set_transport_playing(false);
                    self.video.status = format!("Imported audio {} ({:.1}s) → Audio FX & A2; playback ready", name, dur);
                    self.import_path = path.display().to_string();
                }
                Err(e) => {
                    self.audio.status = format!("Import failed ({}): {}", name, e);
                    self.video.status = self.audio.status.clone();
                }
            }
        } else if matches!(ext.as_str(), "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" | "flv" | "ts" | "mpg" | "mpeg") {
            self.video.generator_kind = bananav_suite::GeneratorKind::None;
            self.video.video_stream = None;
            self.video.media_path = Some(path.to_path_buf());
            let meta_warning = match media_engine::probe_video_meta(path) {
                Ok(m) => {
                    self.video.duration = m.duration.max(0.5);
                    self.video.fps = if m.fps > 0.0 { m.fps } else { 30.0 };
                    self.video.source_w = m.width;
                    self.video.source_h = m.height;
                    let (pw, ph) = preview_dims(m.width, m.height);
                    self.video.fw = pw;
                    self.video.fh = ph;
                    None
                }
                Err(e) => {
                    self.video.duration = ffprobe_duration(path).unwrap_or(12.0).max(0.5);
                    Some(e)
                }
            };
            let dur = self.video.duration;
            self.video.filename = name.clone();
            self.video.loaded = true;
            self.video.dirty = true;
            if !self.append_timeline_mode {
                self.video.seq.clear();
                self.video.playhead = 0.0;
            }
            self.video.seq.push(VideoClip::new(name.clone(), dur, Some(path.to_path_buf())));
            self.video.sel = vec![self.video.seq.len() - 1];
            if !self.media_bin.iter().any(|(_, p, _)| p == path) {
                self.media_bin.push((name.clone(), path.to_path_buf(), dur));
            }
            // Decode embedded audio so the real-time engine has sound on playback.
            if let Ok((l, r, sr)) = media_engine::decode_audio_stereo(path) {
                let n = l.len().min(r.len());
                if n > 0 {
                    let mut inter = Vec::with_capacity(n * 2);
                    for i in 0..n { inter.push(l[i]); inter.push(r[i]); }
                    self.audio.samples = inter;
                    self.audio.sample_rate = sr;
                    self.plugins.set_sample_rate(sr);
                    self.audio.channels = 2;
                    self.audio.duration = n as f32 / sr.max(1) as f32;
                    self.audio.filename = name.clone();
                    self.audio.loaded = true;
                    self.audio.is_demo = false;
                    self.audio.playhead = 0.0;
                    self.audio.sel_end = self.audio.duration;
                    if !self.video.music.iter().any(|m| m.path.as_deref() == Some(path)) {
                        self.video.music.push(MusicClip {
                            uid: fresh_clip_uid(),
                            nm: name.clone(), dur: self.audio.duration, start: 0.0,
                            track: 0, vol: 80.0, path: Some(path.to_path_buf()),
                        });
                    }
                    self.reload_playback_buffer();
                }
            }
            self.set_transport_playing(false);
            self.video.status = format!(
                "Imported video {} · {:.1}s · {}×{} @ {:.2}fps → V1{}",
                name, dur, self.video.source_w, self.video.source_h, self.video.fps,
                meta_warning.map(|e| format!(" · metadata fallback: {}", e)).unwrap_or_default()
            );
            self.import_path = path.display().to_string();
        } else if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "webp" | "gif") {
            self.video.generator_kind = bananav_suite::GeneratorKind::None;
            self.video.video_stream = None;
            self.video.media_path = Some(path.to_path_buf());
            let dur = 5.0_f32;
            if let Ok(dims) = image::image_dimensions(path) {
                self.video.source_w = dims.0;
                self.video.source_h = dims.1;
                let (pw, ph) = preview_dims(dims.0, dims.1);
                self.video.fw = pw;
                self.video.fh = ph;
            }
            self.video.duration = dur;
            self.video.filename = name.clone();
            self.video.loaded = true;
            self.video.dirty = true;
            if !self.append_timeline_mode {
                self.video.seq.clear();
                self.video.playhead = 0.0;
            }
            self.video.seq.push(VideoClip::new(name.clone(), dur, Some(path.to_path_buf())));
            self.video.sel = vec![self.video.seq.len() - 1];
            if !self.media_bin.iter().any(|(_, p, _)| p == path) {
                self.media_bin.push((name.clone(), path.to_path_buf(), dur));
            }
            self.set_transport_playing(false);
            self.video.status = format!("Imported image {} · {}×{} → V1 (5.0s)", name, self.video.source_w, self.video.source_h);
            self.import_path = path.display().to_string();
        } else {
            self.video.status = format!("Unsupported file type: .{} (try .wav/.mp3/.mp4/.mov/.png)", ext);
        }
    }

    fn render_media_tab(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Project Media Bin").color(INK).size(12.0));
        ui.add_space(4.0);
        ui.label(RichText::new("Drop files anywhere in the window, browse, or paste a path.").color(SUB).size(10.0));
        ui.add_space(4.0);
        ui.checkbox(&mut self.drag_drop_enabled, "✋ Enable Drag & Drop (Files, FX & Timeline)");
        ui.checkbox(&mut self.append_timeline_mode, "➕ Append imports to timeline (unchecked = replace)");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("📂 Browse Media...").color(Color32::WHITE)).fill(ACCENT).rounding(Rounding::same(3.0))).clicked() {
                if let Some(path) = media_engine::pick_file_dialog("Import Video, Audio, or Image into Grain") {
                    self.load_media(&path);
                }
            }
            if ui.add(egui::Button::new(RichText::new("📂 +OFX Bundle").color(INK)).fill(TBBG).rounding(Rounding::same(3.0))).clicked() {
                if let Some(path) = media_engine::pick_folder_dialog("Select OpenFX (.ofx.bundle) Plugin Folder") {
                    self.load_media(&path);
                }
            }
        });
        ui.add_space(4.0);
        if ui.add(egui::Button::new(RichText::new("🎬 Load Demo Project (playable)").color(Color32::WHITE).size(10.5)).fill(ACCENT2).rounding(Rounding::same(3.0)).min_size(vec2(ui.available_width(), 24.0))).clicked() {
            self.media_bin.clear();
            self.load_demo_project();
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let te = ui.add(egui::TextEdit::singleline(&mut self.import_path).hint_text("/path/to/clip.mp4 · .wav · .png").desired_width(150.0));
            let enter_pressed = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.add(egui::Button::new(RichText::new("Import Path").color(Color32::WHITE)).fill(ACCENT2).rounding(Rounding::same(3.0))).clicked() || enter_pressed {
                let raw = self.import_path.trim().trim_matches(|c| c == '"' || c == '\'');
                let cleaned = raw.strip_prefix("file://").unwrap_or(raw).to_string();
                if !cleaned.is_empty() {
                    self.load_media(std::path::Path::new(&cleaned));
                }
            }
        });
        if !self.video.status.is_empty() {
            ui.add_space(3.0);
            ui.label(RichText::new(&self.video.status).color(ACCENT).size(9.5));
        }
        if !self.media_bin.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new("Imported Media Bin (drag to timeline or click +V1 / +V2)").color(ACCENT).size(10.0));
            let bin_snap = self.media_bin.clone();
            let sense = if self.drag_drop_enabled { Sense::click_and_drag() } else { Sense::click() };
            for (bi, (bname, bpath, bdur)) in bin_snap.iter().enumerate() {
                ui.horizontal(|ui| {
                    let r = ui.add(egui::Button::new(RichText::new(format!("🎞 {} ({:.1}s)", bname, bdur)).color(INK).size(10.5))
                        .fill(PANEL2).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0)).min_size(vec2(160.0, 22.0)).sense(sense));
                    if self.drag_drop_enabled && r.drag_started() {
                        self.dragging_bin_idx = Some(bi);
                    }
                    if r.clicked() {
                        self.add_bin_item_to_timeline(bname, bpath, *bdur);
                    }
                    if ui.small_button("+V2").on_hover_text("Add as Picture-in-Picture overlay on V2 at playhead").clicked() {
                        self.video.overlays.push(OverlayClip { uid: fresh_clip_uid(), nm: bname.clone(), dur: (*bdur).min(10.0), start: self.video.playhead, x: 75.0, y: 75.0, s: 35.0, o: 100.0, path: Some(bpath.clone()) });
                        self.video.status = format!("Added {} to V2 overlay at {:.1}s", bname, self.video.playhead);
                    }
                });
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Timeline V1 Sequence Clips").color(SUB).size(10.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("🗑 Clear V1").clicked() {
                    self.video.seq.clear();
                    self.video.sel.clear();
                    self.video.playhead = 0.0;
                }
            });
        });
        let seq = self.video.seq.clone();
        let mut acc_t = 0.0_f32;
        for (i, c) in seq.iter().enumerate() {
            let selected = self.video.sel.contains(&i);
            let clip_start = acc_t;
            let clen = c.len();
            acc_t += clen;
            ui.horizontal(|ui| {
                let resp = ui.add(egui::Button::new(RichText::new(format!("🎬  {}  ({:.1}s @ {:.1}s)", c.name, clen, clip_start)).color(if selected { CLIP_TXT } else { INK }).size(10.5))
                    .fill(if selected { ACCENT2 } else { Color32::TRANSPARENT }).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0)).min_size(vec2(ui.available_width() - 28.0, 22.0)));
                if resp.clicked() {
                    self.video.sel = vec![i];
                    self.video.playhead = clip_start;
                    if let Some(p) = &c.path {
                        self.video.media_path = Some(p.clone());
                        self.video.video_stream = None;
                    }
                }
                if ui.small_button("✕").clicked() && i < self.video.seq.len() {
                    self.video.seq.remove(i);
                    self.video.sel.clear();
                }
            });
        }
        // V2+ clips (explicit start times) stay visible and selectable here too.
        let extra_snapshot = self.video.extra_tracks.clone();
        for (ti, track) in extra_snapshot.iter().enumerate() {
            for c in track {
                let selected = self.video.extra_sel == Some(c.uid);
                ui.horizontal(|ui| {
                    let resp = ui.add(egui::Button::new(RichText::new(format!("🎞  V{}  {}  ({:.1}s @ {:.1}s)", ti + 2, c.name, c.len(), c.start)).color(if selected { CLIP_TXT } else { INK }).size(10.0))
                        .fill(if selected { ACCENT2 } else { Color32::TRANSPARENT }).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0)).min_size(vec2(ui.available_width() - 28.0, 22.0)));
                    if resp.clicked() {
                        self.video.extra_sel = Some(c.uid);
                        self.video.sel.clear();
                        self.video.playhead = c.start;
                        self.video.seek_request = Some(c.start);
                        if let Some(p) = &c.path { self.video.media_path = Some(p.clone()); self.video.video_stream = None; }
                    }
                    if ui.small_button("✕").clicked() {
                        if let Some(t) = self.video.extra_tracks.get_mut(ti) { t.retain(|cc| cc.uid != c.uid); }
                        if self.video.extra_sel == Some(c.uid) { self.video.extra_sel = None; }
                        self.video.dirty = true;
                    }
                });
            }
        }
        ui.add_space(6.0);
        ui.label(RichText::new("Audio FX Rack Source").color(SUB).size(10.0));
        if ui.add(egui::Button::new(RichText::new(format!("🎵  {}  {:.1}s (click to open Audio FX)", self.audio.filename, self.audio.duration)).color(INK).size(10.5))
            .fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0)).min_size(vec2(ui.available_width(), 22.0))).clicked() {
            self.dock = DockTab::AudioFx;
        }
        ui.add_space(6.0);
        ui.label(RichText::new("V2 Overlays & A2 Music Clips").color(SUB).size(10.0));
        for i in (0..self.video.overlays.len()).rev() {
            let o = self.video.overlays[i].clone();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("🖼  {}  {:.1}s @ {:.1}s", o.nm, o.dur, o.start)).color(SUB).size(10.0));
                if ui.small_button("✕").clicked() { self.video.overlays.remove(i); }
            });
        }
        for i in (0..self.video.music.len()).rev() {
            let m = self.video.music[i].clone();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("♪  {}  {:.1}s @ {:.1}s", m.nm, m.dur, m.start)).color(SUB).size(10.0));
                if ui.small_button("✕").clicked() { self.video.music.remove(i); }
            });
        }
    }

    fn apply_video_fx_preset(&mut self, name: &str) {
        match name {
            "Brightness" => { self.video.vb = 125.0; self.video.grp_open[1] = true; }
            "Contrast" => { self.video.vc = 130.0; self.video.grp_open[1] = true; }
            "Saturation" => { self.video.vs = 145.0; self.video.grp_open[1] = true; }
            "Hue" => { self.video.vh = 45.0; self.video.grp_open[1] = true; }
            "Color curves" => { self.video.curves[0].p0 = [0.25, 0.15]; self.video.curves[0].p1 = [0.75, 0.88]; self.video.grp_open[1] = true; }
            "Color wheels" => { self.video.wheel_sh = [-0.25, 0.2]; self.video.wheel_hi = [0.25, -0.15]; self.video.grp_open[1] = true; }
            "HSL" => { self.video.vhsl_s = 35.0; self.video.grp_open[1] = true; }
            "Blur" => { self.video.vbl = 4.5; self.video.grp_open[2] = true; }
            "Vignette" => { self.video.vv = 0.55; self.video.grp_open[2] = true; }
            "Film grain" => { self.video.vgr = 0.35; self.video.grp_open[2] = true; }
            "Scanlines" => { self.video.vsc = 0.45; self.video.grp_open[2] = true; }
            "Pixelate" => { self.video.vpx = 10.0; self.video.grp_open[2] = true; }
            "Posterize" => { self.video.vpo = 0.45; self.video.grp_open[2] = true; }
            "RGB split" => { self.video.vrgb = 8.0; self.video.grp_open[2] = true; }
            "Mirror" => { self.video.mirror = MirrorType::Left; self.video.grp_open[0] = true; }
            "Glitch" => { self.video.vgl = 0.4; self.video.vrgb = 6.0; self.video.grp_open[2] = true; }
            "Kaleidoscope" => { self.video.vkl = 6.0; self.video.grp_open[2] = true; }
            "Grayscale" => { self.video.look = LookType::Grayscale; self.video.grp_open[0] = true; }
            "Sepia" => { self.video.look = LookType::Sepia; self.video.grp_open[0] = true; }
            "Invert" => { self.video.look = LookType::Invert; self.video.grp_open[0] = true; }
            "3D tilt" => { self.video.vtilt = 0.35; self.video.grp_open[3] = true; }
            "Cube" => { self.video.vcube = 35.0; self.video.grp_open[3] = true; }
            "Mask" | "Bezier / Shape Mask" | "Cookie Cutter" => { self.video.vmk_type = MaskType::Ellipse; self.video.grp_open[4] = true; }
            "Text overlay" => { self.video.grp_open[4] = true; }
            "Wave" | "Wave Warp (Dual A/B)" | "Ripple & Pond Puddle" | "Heatwave & Mirage" | "Dreamy Ripple Dissolve" => { self.video.vwh = 18.0; self.video.vwv = 12.0; self.video.grp_open[3] = true; }
            "ACES 1.3 Transform" => { self.video.aces_enabled = true; self.video.grp_open[1] = true; }
            "Lift Gamma Gain Offset" => { self.video.lgg_gain = [1.12, 1.04, 0.94]; self.video.lgg_lift = [-0.02, 0.0, 0.03]; self.video.grp_open[1] = true; }
            "LUT Teal & Orange Blockbuster" => { self.video.wheel_sh = [-0.32, 0.18]; self.video.wheel_hi = [0.35, -0.18]; self.video.vc = 118.0; self.video.grp_open[1] = true; }
            "Black and White Noir" => { self.video.look = LookType::Grayscale; self.video.vc = 135.0; self.video.vv = 0.45; self.video.grp_open[0] = true; }
            "Sepia & Cyanotype" => { self.video.look = LookType::Sepia; self.video.vc = 115.0; self.video.grp_open[0] = true; }
            "Bleach Bypass" => { self.video.vs = 45.0; self.video.vc = 145.0; self.video.vgr = 0.22; self.video.grp_open[1] = true; }
            "Cross Process" => { self.video.curves[1].p0 = [0.3, 0.18]; self.video.curves[3].p1 = [0.7, 0.88]; self.video.vs = 135.0; self.video.grp_open[1] = true; }
            "Invert & Solarize" => { self.video.look = LookType::Invert; self.video.grp_open[0] = true; }
            "Posterize & Bit-Depth" | "Threshold (1-Bit B/W)" | "RetroGrade (GameBoy / CGA)" | "Cartoon & Cel Shader" => { self.video.vpo = 0.65; self.video.vc = 130.0; self.video.grp_open[2] = true; }
            "Gaussian Blur" | "Rack Defocus & Bokeh" | "Tilt-Shift Miniature" | "Directional Motion Blur" | "Radial Spin & Zoom Blur" | "Anamorphic Streak Blur" => { self.video.vbl = 6.0; self.video.grp_open[2] = true; }
            "TV Simulator (Trinitron CRT)" | "VHS Tape & Tracking" | "Holomatrix Sci-Fi HUD" => { self.video.vsc = 0.6; self.video.vrgb = 5.0; self.video.vgr = 0.25; self.video.grp_open[2] = true; }
            "Digital Glitch & Datamosh" | "Glitch Memories Transition" => { self.video.vgl = 0.55; self.video.vrgb = 9.0; self.video.grp_open[2] = true; }
            "Pixelate & Mosaic" | " Ceramic Tile Mosaic" | "Halftone Print (CMYK)" | "ASCII & Matrix Terminal" => { self.video.vpx = 12.0; self.video.grp_open[2] = true; }
            "Add Noise & Film Grain" | "Film Damage & Scratches" => { self.video.vgr = 0.45; self.video.vv = 0.35; self.video.grp_open[2] = true; }
            "3D Track Motion / Perspective" | "3D Page Peel" => { self.video.vtilt = 0.42; self.video.vyaw = 0.25; self.video.grp_open[3] = true; }
            "3D Cube & Page Turn" | "3D Cube Spin Transition" => { self.video.vcube = 42.0; self.video.grp_open[3] = true; }
            "Camera Shake (Handheld)" | "Whip / Swish Pan" => { self.video.vsk = 16.0; self.video.grp_open[2] = true; }
            "Chromatic Aberration" | "RGB Split" => { self.video.vrgb = 10.0; self.video.grp_open[2] = true; }
            "Optical Vignette" | "Cinema Letterbox (2.39:1)" => { self.video.vv = 0.65; self.video.grp_open[2] = true; }
            "SMPTE HD Color Bars" => { self.video.generator_kind = bananav_suite::GeneratorKind::SmpteBars; }
            "Checkerboard Generator" => { self.video.generator_kind = bananav_suite::GeneratorKind::Checkerboard; }
            "Color Gradient Generator" => { self.video.generator_kind = bananav_suite::GeneratorKind::ColorGradient; }
            "Solid Color Matte" => { self.video.generator_kind = bananav_suite::GeneratorKind::SolidColor; }
            "Perlin Noise Texture" => { self.video.generator_kind = bananav_suite::GeneratorKind::PerlinNoiseTexture; }
            "Credit Roll Generator" => { self.video.generator_kind = bananav_suite::GeneratorKind::CreditRoll; }
            "SMPTE Timecode Burn-In" => { self.video.generator_kind = bananav_suite::GeneratorKind::TimecodeBurnIn; }
            "Calibration Geometry Grid" => { self.video.generator_kind = bananav_suite::GeneratorKind::TestGrid; }
            other => {
                if let Some(kind) = bananav_suite::GeneratorKind::from_label(other) {
                    self.video.generator_kind = kind;
                    self.video.dirty = true;
                } else {
                    let kind = OfxFx::classify(other);
                    self.video.ofx_active = Some(OfxFx {
                        ident: other.to_string(),
                        kind,
                        mix: 100.0,
                        intensity: 1.2,
                        p_a: 0.5,
                        p_b: 25.0,
                        params: Vec::new(),
                    });
                }
            }
        }
        self.video.status = format!("Applied effect: {} (tune in Properties →)", name);
        self.project_dirty = true;
        self.video.done = false;
        self.video.dirty = true;
    }

    /// BananaV media generators: choose the live preview source and/or insert
    /// a real procedural clip onto any video track.
    fn render_generator_tab(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Media Generator").color(INK).size(13.0));
        ui.label(RichText::new("Procedural sources render real frames in the preview.").color(SUB).size(10.0));
        ui.add_space(4.0);
        ui.label(RichText::new("Preview source").color(ACCENT).size(11.0));
        for &g in bananav_suite::GeneratorKind::GENERATORS {
            let active = self.video.generator_kind == g;
            let b = egui::Button::new(RichText::new(g.label()).color(if active { CLIP_TXT } else { INK }).size(10.5))
                .fill(if active { ACCENT } else { Color32::TRANSPARENT })
                .stroke(Stroke::new(1.0_f32, LINE))
                .rounding(Rounding::same(3.0))
                .min_size(vec2(ui.available_width().max(120.0) - 8.0, 22.0));
            if ui.add(b).clicked() {
                self.video.generator_kind = g;
                self.video.dirty = true;
                self.video.status = format!("Generator source: {}", g.label());
            }
        }
        ui.separator();
        ui.label(RichText::new("Insert generator clip").color(ACCENT).size(11.0));
        slider_row_f(ui, "Duration", &mut self.gen_clip_dur, 0.5, 60.0, 0.5, |v| format!("{:.1}s", v));
        let track_count = self.video.extra_tracks.len() + 1;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Track").color(SUB).size(10.5));
            for t in 0..track_count {
                let active = self.gen_clip_track == t;
                if ui.add(egui::Button::new(RichText::new(format!("V{}", t + 1)).color(if active { CLIP_TXT } else { INK }).size(10.0))
                    .fill(if active { ACCENT2 } else { Color32::TRANSPARENT })
                    .stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0))).clicked() {
                    self.gen_clip_track = t;
                }
            }
        });
        if ui.add(egui::Button::new(RichText::new("＋ Add Generator Clip").color(Color32::WHITE).size(11.0)).fill(ACCENT).rounding(Rounding::same(4.0))).clicked() {
            let kind = if self.video.generator_kind == bananav_suite::GeneratorKind::None {
                bananav_suite::GeneratorKind::SmpteBars
            } else { self.video.generator_kind };
            let mut clip = VideoClip::new(format!("{}", kind.label()), self.gen_clip_dur.max(0.1), None);
            clip.gen = Some(kind);
            clip.start = self.video.playhead;
            self.video.insert_clip_on_track(clip, self.gen_clip_track);
            self.video.status = format!("Added {} clip on V{} at {:.1}s", kind.label(), self.gen_clip_track + 1, self.video.playhead);
        }
        ui.add_space(4.0);
        ui.label(RichText::new("Tip: generator clips use the same color grade and OFX chain as imported media.").color(SUB).size(9.5));
    }

    /// Timeline transitions (Vegas-style edge fades) with a live duration control.
    fn render_transitions_tab(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Transitions").color(INK).size(13.0));
        ui.label(RichText::new("Applied to every clip edge; drag the playhead to preview.").color(SUB).size(10.0));
        ui.add_space(4.0);
        let presets: [(TransType, &str); 5] = [
            (TransType::None, "None"), (TransType::Fade, "Fade through black"),
            (TransType::Wipe, "Wipe"), (TransType::Slide, "Slide"),
            (TransType::Zoom, "Zoom"),
        ];
        for (kind, label) in presets {
            let active = self.video.vtr_type == kind;
            if ui.add(egui::Button::new(RichText::new(label).color(if active { CLIP_TXT } else { INK }).size(10.5))
                .fill(if active { ACCENT } else { Color32::TRANSPARENT })
                .stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0))
                .min_size(vec2(ui.available_width().max(120.0) - 8.0, 22.0))).clicked() {
                self.video.vtr_type = kind;
                self.video.dirty = true;
            }
        }
        ui.separator();
        slider_row_f(ui, "Duration", &mut self.video.vtr_dur, 0.1, 5.0, 0.05, |v| format!("{:.2}s", v));
        ui.label(RichText::new("Keyframes are preserved; transitions are non-destructive.").color(SUB).size(9.5));
    }

    /// Focused color-grading panel (the same real per-pixel controls as the
    /// inspector, without the unrelated track/encoder settings).
    fn render_color_grading_tab(&mut self, ui: &mut Ui) {
        let kf_before = self.video.kf_pack();
        ui.label(RichText::new("Color Grading").color(INK).size(13.0));
        ui.label(RichText::new("Live per-pixel grade · cached while paused").color(SUB).size(10.0));
        ui.add_space(4.0);
        CollapsingHeader::new(RichText::new("Primary").color(INK).size(12.0)).default_open(true).show(ui, |ui| {
            slider_row_f(ui, "Brightness", &mut self.video.vb, 0.0, 200.0, 1.0, |v| format!("{:.0}%", v));
            slider_row_f(ui, "Contrast", &mut self.video.vc, 0.0, 200.0, 1.0, |v| format!("{:.0}%", v));
            slider_row_f(ui, "Saturation", &mut self.video.vs, 0.0, 200.0, 1.0, |v| format!("{:.0}%", v));
            slider_row_f(ui, "Hue", &mut self.video.vh, -180.0, 180.0, 1.0, |v| format!("{:.0}°", v));
            slider_row_f(ui, "Blur", &mut self.video.vbl, 0.0, 20.0, 0.1, |v| format!("{:.1}", v));
            slider_row_f(ui, "Vignette", &mut self.video.vv, 0.0, 1.0, 0.01, |v| format!("{:.2}", v));
            self.video.look = select_row(ui, "Look", self.video.look, &[(LookType::None, "None"), (LookType::Grayscale, "Grayscale"), (LookType::Sepia, "Sepia"), (LookType::Invert, "Invert")]);
        });
        CollapsingHeader::new(RichText::new("Lift / Gamma / Gain (ASC-CDL)").color(INK).size(12.0)).default_open(false).show(ui, |ui| {
            let mut lift = self.video.lgg_lift;
            let mut gamma = self.video.lgg_gamma;
            let mut gain = self.video.lgg_gain;
            let mut off = self.video.lgg_offset;
            for (label, arr) in [("Lift", &mut lift), ("Gamma", &mut gamma), ("Gain", &mut gain), ("Offset", &mut off)] {
                ui.label(RichText::new(label).color(SUB).size(11.0));
                let (min, max) = if label == "Gamma" || label == "Gain" { (0.2, 2.5) } else { (-0.5, 0.5) };
                slider_row_f(ui, "R", &mut arr[0], min, max, 0.01, |v| format!("{v:+.2}"));
                slider_row_f(ui, "G", &mut arr[1], min, max, 0.01, |v| format!("{v:+.2}"));
                slider_row_f(ui, "B", &mut arr[2], min, max, 0.01, |v| format!("{v:+.2}"));
            }
            self.video.lgg_lift = lift;
            self.video.lgg_gamma = gamma;
            self.video.lgg_gain = gain;
            self.video.lgg_offset = off;
            ui.checkbox(&mut self.video.aces_enabled, "BananaV ACES 1.3");
        });
        CollapsingHeader::new(RichText::new("Curves").color(INK).size(12.0)).default_open(false).show(ui, |ui| {
            self.video.vcv_ch = select_row(ui, "Channel", self.video.vcv_ch, &[(CurveCh::Rgb, "RGB"), (CurveCh::R, "Red"), (CurveCh::G, "Green"), (CurveCh::B, "Blue")]);
            self.video.render_curves(ui);
        });
        CollapsingHeader::new(RichText::new("HSL & Grain").color(INK).size(12.0)).default_open(false).show(ui, |ui| {
            slider_row_f(ui, "HSL Saturation", &mut self.video.vhsl_s, -100.0, 100.0, 1.0, |v| format!("{:.0}", v));
            slider_row_f(ui, "Grain", &mut self.video.vgr, 0.0, 1.0, 0.01, |v| format!("{:.2}", v));
            slider_row_f(ui, "Scanlines", &mut self.video.vsc, 0.0, 1.0, 0.01, |v| format!("{:.2}", v));
            slider_row_f(ui, "Glitch", &mut self.video.vgl, 0.0, 1.0, 0.01, |v| format!("{:.2}", v));
        });
        let kf_after = self.video.kf_pack();
        if !self.video.keyframes.is_empty() && kf_after != kf_before { self.video.upsert_kf(kf_after); }
        self.video.dirty = true;
    }

    fn start_video_fx_scan(&mut self) {
        if self.video_fx_loading { return; }
        let (tx, rx) = std::sync::mpsc::channel();
        self.video_fx_rx = Some(rx);
        self.video_fx_loading = true;
        self.video_fx_paused = false;
        self.video_fx_cancel_requested = false;
        self.video_fx_status = "Scanning OpenFX plugins…".into();
        std::thread::spawn(move || {
            let found = ofx_host::scan_ofx_directories(&[]);
            let _ = tx.send(found);
        });
    }

    fn cancel_video_fx_loading(&mut self) {
        self.video_fx_cancel_requested = true;
        self.video_fx_loading = false;
        self.video_fx_rx = None;
        self.video_fx_status = "Video FX loading cancelled.".into();
    }

    fn pump_video_fx(&mut self) {
        if !self.video_fx_loading || self.video_fx_paused { return; }
        let Some(rx) = &self.video_fx_rx else { return };
        match rx.try_recv() {
            Ok(found) => {
                self.video_fx_loading = false;
                self.video_fx_rx = None;
                if self.video_fx_cancel_requested {
                    self.video_fx_cancel_requested = false;
                    self.video_fx_status = "Video FX loading cancelled.".into();
                    return;
                }
                self.ofx_plugins = found;
                if self.ofx_sel.is_none() && !self.ofx_plugins.is_empty() { self.ofx_sel = Some(0); }
                if self.ofx_sel.map(|i| i >= self.ofx_plugins.len()).unwrap_or(false) { self.ofx_sel = None; }
                self.ofx_scanned = true;
                self.video_fx_status = format!("OpenFX scan complete — {} plugin(s).", self.ofx_plugins.len());
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.video_fx_loading = false;
                self.video_fx_rx = None;
                self.video_fx_status = "Video FX scan stopped unexpectedly.".into();
            }
        }
    }

    fn render_video_fx_tab(&mut self, ui: &mut Ui) {
        if !self.ofx_scanned && !self.video_fx_loading {
            self.ofx_scanned = true;
            self.start_video_fx_scan();
        }
        self.pump_video_fx();
        if self.video_fx_loading { ui.ctx().request_repaint(); }
        if self.video_fx_loading || !self.video_fx_status.is_empty() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(if self.video_fx_paused { "Paused" } else { &self.video_fx_status }).color(SUB).size(10.0));
                if self.video_fx_loading {
                    if ui.small_button(if self.video_fx_paused { "Resume" } else { "Pause" }).clicked() { self.video_fx_paused = !self.video_fx_paused; }
                    if ui.small_button("Cancel").clicked() { self.cancel_video_fx_loading(); }
                } else if ui.small_button("↺ Reload").clicked() {
                    self.ofx_scanned = true;
                    self.start_video_fx_scan();
                }
            });
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("Video FX & Third Party OpenFX").color(INK).size(12.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("↺ Reset FX").clicked() {
                    let v = &mut self.video;
                    v.vb = 100.0; v.vc = 100.0; v.vs = 100.0; v.vh = 0.0; v.vbl = 0.0; v.vv = 0.0;
                    v.vwh = 0.0; v.vwv = 0.0; v.vtilt = 0.0; v.vyaw = 0.0; v.vcube = 0.0; v.vrgb = 0.0;
                    v.vpx = 0.0; v.vgr = 0.0; v.vsc = 0.0; v.vgl = 0.0; v.vpo = 0.0;
                    v.vkl = 0.0; v.vsk = 0.0; v.vtl = 0.0;
                    v.look = LookType::None; v.mirror = MirrorType::None;
                    v.curves = [CurvePts::default(); 4];
                    v.wheel_sh = [0.0; 2]; v.wheel_mid = [0.0; 2]; v.wheel_hi = [0.0; 2];
                    v.vhsl_b = 0; v.vhsl_h = 0.0; v.vhsl_s = 0.0; v.vhsl_l = 0.0;
                    v.vtr_type = TransType::None; v.vtr_dur = 0.8;
                    v.vmk_type = MaskType::None; v.vmk_x = 50.0; v.vmk_y = 50.0; v.vmk_s = 60.0; v.vmk_f = 20.0; v.vmk_i = false;
                    v.ofx_active = None; v.keyframes.clear();
                    v.dirty = true;
                    v.status = "Reset all Video FX & OpenFX to neutral.".into();
                }
            });
        });
        ui.add_space(4.0);
        // BananaV 4-Level Effect Chain selector
        ui.horizontal_wrapped(|ui| {
            for &lvl in bananav_suite::FxChainLevel::ALL {
                let active = self.video.fx_chain_level == lvl;
                if ui.add(egui::Button::new(RichText::new(lvl.label()).color(if active { CLIP_TXT } else { SUB }).size(9.5))
                    .fill(if active { ACCENT2 } else { TBBG }).rounding(Rounding::same(3.0))).clicked() {
                    self.video.fx_chain_level = lvl;
                }
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.video.fx_search).hint_text("🔍 Search 125+ BananaV FX, Transitions & Generators...").desired_width(180.0));
            if ui.small_button("✕").clicked() { self.video.fx_search.clear(); }
        });
        ui.horizontal_wrapped(|ui| {
            for cat in ["All", "Core 25", "Color & ACES", "Blur & Sharpen", "Light & Glow", "Warp & 3D", "Stylize & Retro", "Key & Mask", "Transitions (GLSL)", "Generators"] {
                let active = self.video.fx_cat_filter == cat;
                if ui.add(egui::Button::new(RichText::new(cat).color(if active { CLIP_TXT } else { SUB }).size(9.0))
                    .fill(if active { ACCENT } else { Color32::TRANSPARENT }).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(10.0))).clicked() {
                    self.video.fx_cat_filter = cat.to_string();
                }
            }
        });
        ui.add_space(4.0);
        let effects = [
            "Brightness", "Contrast", "Saturation", "Hue", "Color curves", "Color wheels", "HSL",
            "Blur", "Vignette", "Film grain", "Scanlines", "Pixelate", "Posterize", "RGB split",
            "Mirror", "Glitch", "Kaleidoscope", "Grayscale", "Sepia", "Invert", "3D tilt", "Cube",
            "Mask", "Text overlay", "Wave",
        ];
        let mut clicked_fx: Option<String> = None;
        let mut dragged_fx: Option<String> = None;
        let sense = if self.drag_drop_enabled { Sense::click_and_drag() } else { Sense::click() };
        let q = self.video.fx_search.trim().to_lowercase();
        if self.video.fx_cat_filter == "All" || self.video.fx_cat_filter == "Core 25" {
            ui.label(RichText::new("Core Quick FX (25)").color(SUB).size(10.0));
            ui.horizontal_wrapped(|ui| {
                for e in effects.iter() {
                    if !q.is_empty() && !e.to_lowercase().contains(&q) { continue; }
                    let r = ui.add(egui::Button::new(RichText::new(format!("fx · {}", *e)).color(INK).size(10.0))
                        .fill(PANEL2).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0)).min_size(vec2(124.0, 26.0)).sense(sense));
                    if r.clicked() { clicked_fx = Some((*e).to_string()); }
                    if self.drag_drop_enabled && r.drag_started() { dragged_fx = Some((*e).to_string()); }
                }
            });
            ui.add_space(6.0);
        }
        ui.label(RichText::new(format!("BananaV v0.3.0 Complete Suite ({} effects/transitions/generators)", bananav_suite::BANANAV_120_VIDEO_CATALOG.len())).color(ACCENT).size(10.5));
        ui.horizontal_wrapped(|ui| {
            for def in bananav_suite::BANANAV_120_VIDEO_CATALOG {
                if self.video.fx_cat_filter != "All" && self.video.fx_cat_filter != "Core 25" && def.category != self.video.fx_cat_filter {
                    continue;
                }
                if self.video.fx_cat_filter == "Core 25" {
                    continue;
                }
                if !q.is_empty() && !def.name.to_lowercase().contains(&q) && !def.category.to_lowercase().contains(&q) {
                    continue;
                }
                let r = ui.add(egui::Button::new(RichText::new(format!("🍌 {}", def.name)).color(INK).size(9.5))
                    .fill(PANEL2).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0)).min_size(vec2(128.0, 25.0)).sense(sense))
                    .on_hover_text(format!("[{}] {}", def.category, def.description));
                if r.clicked() { clicked_fx = Some(def.name.to_string()); }
                if self.drag_drop_enabled && r.drag_started() { dragged_fx = Some(def.name.to_string()); }
            }
        });
        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new("Third Party (Named OpenFX Effects & Transitions)").color(ACCENT).size(11.0));
        ui.add_space(4.0);
        let ofx_names: Vec<String> = self.ofx_plugins.iter().map(|p| p.identifier.clone()).collect();
        for (i, nm) in ofx_names.iter().enumerate() {
            let is_active = self.video.ofx_active.as_ref().map(|a| &a.ident == nm).unwrap_or(false);
            let r = ui.add(egui::Button::new(RichText::new(format!("OFX · {}", nm)).color(if is_active { CLIP_TXT } else { INK }).size(10.0))
                .fill(if is_active { ACCENT2 } else { PANEL2 }).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0)).min_size(vec2(ui.available_width(), 24.0)).sense(sense));
            if r.clicked() {
                self.ofx_sel = Some(i);
                clicked_fx = Some(nm.clone());
            }
            if self.drag_drop_enabled && r.drag_started() {
                self.ofx_sel = Some(i);
                dragged_fx = Some(nm.clone());
            }
        }
        if let Some(fx_name) = clicked_fx {
            self.apply_video_fx_preset(&fx_name);
            self.selected_fx = Some(fx_name);
        }
        if let Some(fx_name) = dragged_fx {
            self.dragging_fx_label = Some(fx_name);
        }
        if !self.video.status.is_empty() && !self.video.processing {
            ui.add_space(4.0);
            ui.label(RichText::new(&self.video.status).color(ACCENT).size(10.0));
        }

        // Effect popup / event-FX list.
        let ctx = ui.ctx().clone();
        if let Some(name) = self.selected_fx.clone() {
            let mut open = true;
            egui::Window::new(format!("Effect — {}", name))
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .default_width(320.0)
                .min_width(320.0)
                .max_width(320.0)
                .show(&ctx, |ui| {
                    ui.label(RichText::new(format!("Applied to timeline · {:?} event", self.video.fx_chain_level)).color(SUB).size(10.0));
                    ui.separator();
                    ui.label(RichText::new("Tune in Properties. Reorderable event FX and per-clip assignment are preserved.").color(INK).size(10.5));
                    ui.horizontal(|ui| {
                        if ui.button("Apply again").clicked() { self.apply_video_fx_preset(&name); }
                        if ui.button("Close").clicked() { self.selected_fx = None; }
                    });
                });
            if !open { self.selected_fx = None; }
        }
    }

    fn render_titles_tab(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Titles").color(INK).size(12.0));
        ui.add_space(4.0);
        ui.label(RichText::new("Click a preset to add a text overlay at the playhead.").color(SUB).size(10.0));
        ui.add_space(6.0);
        let presets = [("Title", "Grain", 12.0), ("Lower Third", "LOWER THIRD", 8.0), ("Caption", "Caption", 6.0), ("Credits", "Credits", 10.0)];
        for (name, text, size) in presets {
            let r = ui.add(egui::Button::new(RichText::new(format!("T  {}  “{}”", name, text)).color(INK).size(11.0))
                .fill(PANEL2).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0)).min_size(vec2(ui.available_width(), 30.0)));
            if r.clicked() {
                self.video.texts.push(TextOverlay { uid: fresh_clip_uid(), t: text.into(), size, x: 50.0, y: if name == "Lower Third" { 80.0 } else { 50.0 }, a: self.video.playhead, b: 1e9, col: Color32::WHITE });
                self.video.status = format!("Added “{}” at {:.1}s.", text, self.video.playhead);
            }
        }
    }

    fn render_ofx_tab(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("OpenFX Host").color(INK).size(12.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("↻ Rescan").color(SUB).size(10.0)).fill(TBBG).rounding(Rounding::same(3.0))).clicked() {
                    self.ofx_scanned = true;
                    self.start_video_fx_scan();
                }
            });
        });
        ui.label(RichText::new(format!("Engine: {} · libloading dlopen", media_engine::engine_label())).color(SUB).size(10.0));
        ui.add_space(4.0);
        if !self.ofx_scanned && !self.video_fx_loading {
            self.ofx_scanned = true;
            self.start_video_fx_scan();
        }
        self.pump_video_fx();
        if self.video_fx_loading { ui.ctx().request_repaint(); }
        if self.video_fx_loading || !self.video_fx_status.is_empty() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(if self.video_fx_paused { "Paused" } else { &self.video_fx_status }).color(SUB).size(10.0));
                if self.video_fx_loading {
                    if ui.small_button(if self.video_fx_paused { "Resume" } else { "Pause" }).clicked() { self.video_fx_paused = !self.video_fx_paused; }
                    if ui.small_button("Cancel").clicked() { self.cancel_video_fx_loading(); }
                }
            });
        }
        let n = self.ofx_plugins.len();
        ui.label(RichText::new(format!("{} plugin(s) registered", n)).color(SUB).size(10.0));
        ui.add_space(4.0);
        let names: Vec<String> = self.ofx_plugins.iter().map(|p| p.identifier.clone()).collect();
        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
            for (i, nm) in names.iter().enumerate() {
                let sel = self.ofx_sel == Some(i);
                let r = ui.add(egui::Button::new(RichText::new(nm).color(if sel { CLIP_TXT } else { INK }).size(10.0))
                    .fill(if sel { ACCENT2 } else { Color32::TRANSPARENT }).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0)).min_size(vec2(ui.available_width(), 22.0)));
                if r.clicked() { self.ofx_sel = Some(i); }
            }
        });
        ui.add_space(4.0);
        if let Some(i) = self.ofx_sel {
            // Snapshot display fields so we can mutate ofx_plugins freely below.
            let snap = self.ofx_plugins.get(i).map(|p| (
                p.identifier.clone(), p.apiname.clone(), p.version_major, p.version_minor, p.status.clone(), p.bundlename.clone(),
                p.params.iter().map(|q| (q.label.clone(), q.value, q.min, q.max)).collect::<Vec<_>>(),
            ));
            if let Some((ident, apiname, vmaj, vmin, status, bundle, params)) = snap {
                ui.separator();
                ui.label(RichText::new(&ident).color(INK).size(11.0));
                ui.label(RichText::new(format!("{} v{}.{} · {}", apiname, vmaj, vmin, status)).color(SUB).size(9.0));
                ui.label(RichText::new(&bundle).color(SUB).size(9.0));
                ui.add_space(4.0);
                for (pi, (label, val, min, max)) in params.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).color(SUB).size(10.0));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(format!("{:.2}", val)).color(INK).font(FontId::monospace(9.0)));
                        });
                    });
                    let mut v = *val;
                    ui.add(Slider::new(&mut v, *min..=*max).show_value(false));
                    if let Some(plug) = self.ofx_plugins.get_mut(i) {
                        if let Some(pp) = plug.params.get_mut(pi) { pp.value = v; }
                    }
                }
                ui.add_space(4.0);
                let apply = ui.add(egui::Button::new(RichText::new("Apply to timeline").color(Color32::WHITE)).fill(ACCENT).rounding(Rounding::same(3.0)).min_size(vec2(ui.available_width(), 24.0)));
                if apply.clicked() {
                    if let Some(plug) = self.ofx_plugins.get(i) {
                        let mut mix = 100.0_f32; let mut intensity = 1.0_f32; let mut pa = 0.5_f32; let mut pb = 10.0_f32;
                        for q in &plug.params {
                            match q.name.as_str() {
                                "mix" => mix = q.value,
                                "intensity" => intensity = q.value,
                                "threshold" | "blur_amount" | "amplitude" => pa = q.value,
                                "radius" | "frequency" => pb = q.value,
                                _ => {}
                            }
                        }
                        let ident = plug.identifier.clone();
                        let kind = OfxFx::classify(&ident);
                        self.video.ofx_active = Some(OfxFx { ident: ident.clone(), kind, mix, intensity, p_a: pa, p_b: pb, params: plug.params.clone() });
                        self.video.status = format!("OFX “{}” ({:?}) applied — {} params, mix {:.0}%.", ident, kind, plug.params.len(), mix);
                        self.video.done = false;
                        self.video.dirty = true;
                    }
                }
                ui.add_space(2.0);
                if ui.add(egui::Button::new(RichText::new("Bypass OFX").color(SUB).size(10.0)).fill(TBBG).rounding(Rounding::same(3.0))).clicked() {
                    self.video.ofx_active = None;
                    self.video.status = "OFX bypassed.".into();
                    self.video.dirty = true;
                }
            }
        }
        self.render_ofx_properties_window(ui.ctx());
    }

    /// Separate full-property window for the active OFX effect. This exposes
    /// every plugin parameter, not only the four mapped render controls.
    fn render_ofx_properties_window(&mut self, ctx: &egui::Context) {
        if self.video.ofx_active.is_none() { return; }
        egui::Window::new("OFX Effect Properties")
            .collapsible(true)
            .resizable(false)
            .default_width(380.0)
            .min_width(380.0)
            .max_width(380.0)
            .show(ctx, |ui| {
                let Some(fx) = &mut self.video.ofx_active else { return };
                ui.label(RichText::new(&fx.ident).color(INK).size(12.0));
                ui.label(RichText::new(format!("{:?} · mix {:.0}%", fx.kind, fx.mix)).color(SUB).size(10.0));
                ui.separator();
                ui.horizontal(|ui| { ui.label("Mix"); ui.add(Slider::new(&mut fx.mix, 0.0..=100.0)); });
                ui.horizontal(|ui| { ui.label("Intensity"); ui.add(Slider::new(&mut fx.intensity, 0.0..=5.0)); });
                ui.horizontal(|ui| { ui.label("Primary"); ui.add(Slider::new(&mut fx.p_a, -10.0..=10.0)); });
                ui.horizontal(|ui| { ui.label("Secondary"); ui.add(Slider::new(&mut fx.p_b, -100.0..=100.0)); });
                ui.separator();
                ui.label(RichText::new("Full OFX plugin properties").color(ACCENT).size(11.0));
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for (pi, pp) in fx.params.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&pp.label).color(SUB).size(10.0));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(format!("{} · {:.2}", pp.name, pp.value)).color(INK).font(FontId::monospace(9.0)));
                            });
                        });
                        let min = if pp.min.is_finite() { pp.min } else { -100.0 };
                        let max = if pp.max.is_finite() && pp.max > min { pp.max } else { min + 1.0 };
                        ui.add(Slider::new(&mut pp.value, min..=max).show_value(false));
                        if pi % 4 == 3 { ui.add_space(2.0); }
                    }
                });
                ui.add_space(4.0);
                if ui.button("Reset properties").clicked() {
                    for pp in &mut fx.params { pp.value = pp.default_val; }
                }
                ui.label(RichText::new("Rendering uses full property data; current pixel pass maps mix/intensity/primary/secondary.").color(SUB).size(9.0));
            });
        self.video.dirty = true;
    }

    /// Fully render the timeline to a real MP4. Every frame is recomputed with
    /// the current NLE state (V1/V2+, Overlay, Text, effects, OFX, visibility)
    /// and mixed timeline audio is bounced to a temp WAV for FFmpeg. This is
    /// non-destructive: source files are never modified.
    fn export_timeline_video(&mut self, out_path: &std::path::Path, ctx: &egui::Context) -> Result<String, String> {
        let fps = self.video.fps.max(1.0);
        let duration = {
            let v = &self.video;
            let video_end = v.total();
            let overlay_end = if v.track_active(1) { v.overlays.iter().map(|o| o.start + o.dur).fold(0.0_f32, f32::max) } else { 0.0 };
            let text_end = if v.track_active(4) {
                v.texts.iter().map(|t| t.a.max(if t.b > 1e8 { video_end } else { t.b })).fold(0.0_f32, f32::max)
            } else { 0.0 };
            let music_end = v.music.iter().filter(|m| v.audio_track_active(m.track)).map(|m| m.start + m.dur).fold(0.0_f32, f32::max);
            video_end.max(overlay_end).max(text_end).max(music_end)
        };
        let frame_count = ((duration * fps).ceil() as usize).max(1);
        let w = self.video.fw;
        let h = self.video.fh;
        if w == 0 || h == 0 { return Err("invalid output resolution".into()); }
        let even_w = w & !1;
        let even_h = h & !1;
        if even_w == 0 || even_h == 0 { return Err("invalid output resolution".into()); }

        let tmp = std::env::temp_dir().join(format!("grain_timeline_export_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).map_err(|e| format!("temp dir: {e}"))?;
        if let Some(parent) = out_path.parent() { let _ = std::fs::create_dir_all(parent); }

        let old_playhead = self.video.playhead;
        let old_dirty = self.video.dirty;
        let old_kf = self.video.kf_pack();
        let orig_audio_samples = self.audio.samples.clone();
        let orig_audio_channels = self.audio.channels;
        let orig_audio_sample_rate = self.audio.sample_rate;
        let orig_audio_duration = self.audio.duration;
        let orig_audio_playhead = self.audio.playhead;
        let orig_mix_key = self.last_audio_mix_key.clone();

        // Always restore interactive + audio state, then clean temp files.
        let result: Result<(bool, Vec<String>, bool), String> = (|| {
            for i in 0..frame_count {
                let t = i as f32 / fps;
                self.video.playhead = t;
                self.video.dirty = true;
                self.video.apply_kf_at(t);
                self.video.recompute_frame(ctx);
                if self.video.last_rgba.len() != w * h * 4 {
                    return Err(format!("frame {i} render produced no pixels"));
                }
                let frame_path = tmp.join(format!("frame_{i:05}.png"));
                image::save_buffer(&frame_path, &self.video.last_rgba, w as u32, h as u32, image::ColorType::Rgba8)
                    .map_err(|e| format!("frame {i} png: {e}"))?;
            }

            // Bounce the real timeline audio mix + DSP + plugin chain to WAV.
            let audio_wav = tmp.join("timeline_audio.wav");
            let mut audio_notes: Vec<String> = Vec::new();
            let mut audio_out = false;
            if !self.audio.samples.is_empty() {
                self.maybe_rebuild_timeline_audio();
                if !self.audio.samples.is_empty() {
                    audio_notes = self.audio.render_real(&audio_wav.to_string_lossy(), &mut self.plugins)?;
                    audio_out = audio_wav.exists();
                }
            }

            let br_kbps = (self.video.venc_br.max(1.0) * 1000.0) as u32;
            let vcodec = match self.video.venc_codec {
                CodecType::H264 => "libx264",
                CodecType::Vp9 => "libvpx-vp9",
                CodecType::Vp8 => "libvpx",
                CodecType::Av1 => "libaom-av1",
            };
            let mut args: Vec<String> = vec![
                "-y".into(), "-loglevel".into(), "error".into(),
                "-framerate".into(), format!("{fps:.6}"),
                "-i".into(), tmp.join("frame_%05d.png").to_string_lossy().into_owned(),
            ];
            if audio_out {
                args.push("-i".into());
                args.push(audio_wav.to_string_lossy().into_owned());
            }
            args.push("-c:v".into()); args.push(vcodec.into());
            args.push("-vf".into()); args.push(format!("scale={even_w}:{even_h}"));
            args.push("-r".into()); args.push(format!("{fps:.6}"));
            args.push("-pix_fmt".into()); args.push("yuv420p".into());
            args.push("-b:v".into()); args.push(format!("{br_kbps}k"));
            if audio_out {
                args.push("-c:a".into()); args.push("aac".into());
                args.push("-af".into()); args.push("apad".into());
            } else {
                args.push("-an".into());
            }
            args.push("-t".into()); args.push(format!("{duration:.6}"));
            args.push("-movflags".into()); args.push("+faststart".into());
            args.push(out_path.to_string_lossy().into_owned());

            let spawned = std::process::Command::new("ffmpeg").args(args.iter()).output();
            let out = spawned.map_err(|e| format!("ffmpeg failed ({e}) — install FFmpeg for MP4 export"))?;
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                return Err(if err.is_empty() { "ffmpeg failed".into() } else { err });
            }
            if !out_path.exists() { return Err("ffmpeg produced no output file".into()); }
            Ok((audio_out, audio_notes, true))
        })();

        // Restore live timeline/audio after non-destructive export.
        self.video.playhead = old_playhead;
        self.video.dirty = old_dirty;
        self.video.kf_apply(old_kf);
        self.video.recompute_frame(ctx);
        self.audio.samples = orig_audio_samples;
        self.audio.channels = orig_audio_channels;
        self.audio.sample_rate = orig_audio_sample_rate;
        self.audio.duration = orig_audio_duration;
        self.audio.playhead = orig_audio_playhead;
        self.last_audio_mix_key = orig_mix_key;
        let _ = std::fs::remove_dir_all(&tmp);

        let (audio_out, audio_notes, _) = result?;
        let notes = if audio_notes.is_empty() { String::new() } else { format!(" · {}", audio_notes.join(" · ")) };
        Ok(format!(
            "Rendered timeline → {} ({} frames · {:.3}s · {:.0} fps{}{})",
            out_path.display(), frame_count, duration, fps,
            if audio_out { " · audio" } else { "" }, notes
        ))
    }

    fn render_export_tab(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.label(RichText::new("Export").color(INK).size(12.0));
        ui.add_space(6.0);
        self.video.venc_codec = select_row(ui, "Codec", self.video.venc_codec, &[(CodecType::H264, "H.264"), (CodecType::Vp9, "VP9"), (CodecType::Vp8, "VP8"), (CodecType::Av1, "AV1")]);
        slider_row_f(ui, "Bitrate (Mbps)", &mut self.video.venc_br, 1.0, 40.0, 1.0, |v| format!("{:.0}", v));
        ui.add_space(8.0);
        let v_btn = ui.add(egui::Button::new(RichText::new("🎬 Export Video → MP4 (FFmpeg)").color(Color32::WHITE)).fill(ACCENT).rounding(Rounding::same(4.0)).min_size(vec2(ui.available_width(), 32.0)));
        if v_btn.clicked() {
            let out_path = media_engine::default_export_path("grain_video_export.mp4");
            match self.export_timeline_video(&out_path, ctx) {
                Ok(msg) => { self.video.done = true; self.video.progress = 1.0; self.video.status = msg; }
                Err(e) => { self.video.done = false; self.video.progress = 0.0; self.video.status = format!("Video export error: {e}"); }
            }
        }
        ui.add_space(4.0);
        let run = ui.add(egui::Button::new(RichText::new("🖼 Export Graded Frame → PNG").color(Color32::WHITE)).fill(ACCENT2).rounding(Rounding::same(4.0)).min_size(vec2(ui.available_width(), 30.0)));
        if run.clicked() {
            self.video.recompute_frame(ctx);
            let out_path = media_engine::default_export_path("grain_export_frame.png");
            let out_str = out_path.to_string_lossy().to_string();
            match self.video.export_png(&out_str) {
                Ok(_) => { self.video.done = true; self.video.progress = 1.0; self.video.status = format!("Exported graded frame → {}", out_str); }
                Err(e) => { self.video.done = false; self.video.progress = 0.0; self.video.status = format!("Export error: {e}"); }
            }
        }
        ui.add_space(4.0);
        let awav = ui.add(egui::Button::new(RichText::new("🔊 Render Audio → WAV (DSP + VST/VST3/AU)").color(INK)).fill(TBBG).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(4.0)).min_size(vec2(ui.available_width(), 28.0)));
        if awav.clicked() {
            let out_path = media_engine::default_export_path("grain_audio_export.wav");
            let out_str = out_path.to_string_lossy().to_string();
            match self.audio.render_real(&out_str, &mut self.plugins) {
                Ok(notes) => {
                    self.video.done = true;
                    self.video.progress = 1.0;
                    self.video.status = format!(
                        "Rendered audio (non-destructive) → {}{}",
                        out_str,
                        if notes.is_empty() { String::new() } else { format!(" · {}", notes.join(" · ")) }
                    );
                }
                Err(e) => {
                    self.video.done = false;
                    self.video.progress = 0.0;
                    self.video.status = format!("Audio export error: {e}");
                }
            }
        }
        ui.add_space(4.0);
        let bounce = ui.add(egui::Button::new(RichText::new("💾 Bounce Audio → WAV (keep live track)").color(SUB).size(10.0)).fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(4.0)).min_size(vec2(ui.available_width(), 24.0)));
        if bounce.clicked() {
            let out_path = media_engine::default_export_path("grain_audio_bounce.wav");
            let out_str = out_path.to_string_lossy().to_string();
            match self.audio.render_real(&out_str, &mut self.plugins) {
                Ok(notes) => {
                    self.video.done = true;
                    self.video.progress = 1.0;
                    self.video.status = format!(
                        "Bounced audio → {}{}",
                        out_str,
                        if notes.is_empty() { String::new() } else { format!(" · {}", notes.join(" · ")) }
                    );
                }
                Err(e) => { self.video.done = false; self.video.progress = 0.0; self.video.status = format!("Audio bounce error: {e}"); }
            }
        }
        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new("BananaV / VEGAS Pro Project & Timeline Interchange").color(ACCENT).size(11.0));
        ui.add_space(4.0);
        let clips_meta: Vec<(String, f32, f32, f32)> = {
            let mut acc = 0.0_f32;
            let mut v = Vec::new();
            for c in &self.video.seq {
                let l = c.len();
                v.push((c.name.clone(), acc, l, c.speed));
                acc += l;
            }
            v
        };
        ui.horizontal_wrapped(|ui| {
            for (lbl, fname) in [
                ("💾 Save .veg (VEGAS Pro)", "grain_project.veg"),
                ("💾 Export .otio (OpenTimelineIO)", "grain_timeline.otio"),
                ("💾 Export .edl (CMX 3600)", "grain_timeline.edl"),
                ("💾 Export .fcpxml (Final Cut)", "grain_timeline.fcpxml"),
            ] {
                if ui.add(egui::Button::new(RichText::new(lbl).color(INK).size(10.0)).fill(PANEL2).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0))).clicked() {
                    let out_p = media_engine::default_export_path(fname);
                    match bananav_suite::export_project_interchange(&out_p, "Grain_BananaV_Project", 30.0, &clips_meta) {
                        Ok(msg) => { self.video.done = true; self.video.status = msg; }
                        Err(e) => { self.video.done = false; self.video.status = format!("Interchange error: {e}"); }
                    }
                }
            }
        });
        ui.add_space(6.0);
        ui.separator();
        ui.label(RichText::new("Last export").color(ACCENT).size(11.0));
        let (bar_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 7.0), Sense::hover());
        let bp = ui.painter();
        bp.rect_filled(bar_rect, 3.0, TBBG);
        let frac = if self.video.done { 1.0_f32 } else { self.video.progress.clamp(0.0, 1.0) };
        let bar_col = if self.video.done { Color32::from_rgb(0x3b, 0xb5, 0x4a) } else if self.video.processing { AMBER } else { LINE };
        if frac > 0.0 {
            bp.rect_filled(Rect::from_min_size(bar_rect.min, vec2(bar_rect.width() * frac, bar_rect.height())), 3.0, bar_col);
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{:.0}%", frac * 100.0)).color(SUB).size(9.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(if self.video.done { "Complete" } else if self.video.processing { "Working…" } else { "Idle" }).color(if self.video.done { Color32::from_rgb(0x6b, 0xd1, 0x7b) } else { SUB }).size(9.0));
            });
        });
        if !self.video.status.is_empty() {
            ui.label(RichText::new(&self.video.status).color(SUB).size(10.0));
        }
        ui.label(RichText::new(format!("PNG: {}×{} RGBA · Audio: f32 WAV · engine {}", self.video.fw, self.video.fh, media_engine::engine_label())).color(SUB).size(9.0));
    }

    // -----------------------------------------------------------------------
    // Timeline model. The editor talks in UIDs and lanes; the legacy clip
    // vectors below are only touched when an edit is actually committed.
    // -----------------------------------------------------------------------
    fn timeline_snapshot(&self) -> (Vec<TlTrack>, Vec<TimelineItem>, HashMap<u64, ItemRef>) {
        let nv = self.video.video_track_count();
        let na = self.video.audio_track_count();
        let v = &self.video;
        let tui = &self.timeline_ui;
        let mut tracks: Vec<TlTrack> = Vec::new();

        let mut push_track = |name: &str, kind: TrackKind, height: f32, state_idx: isize, mute: bool, solo: bool| {
            let i = tracks.len();
            tracks.push(TlTrack {
                header: TrackHeader {
                    name: name.into(),
                    kind,
                    height,
                    mute,
                    solo,
                    lock: tui.locked.get(i).copied().unwrap_or(false),
                    visible: tui.visible.get(i).copied().unwrap_or(true),
                    target: tui.target.get(i).copied().unwrap_or(false),
                },
                state_idx,
            });
        };

        for i in 0..nv {
            let (mute, solo, si) = if i == 0 {
                (v.track_mute[0], v.track_solo[0], 0isize)
            } else {
                let e = i - 1;
                (v.extra_track_mute[e], v.extra_track_solo[e], (5 + e) as isize)
            };
            push_track(&format!("V{}", i + 1), TrackKind::Video, 44.0, si, mute, solo);
        }
        push_track("Overlay", TrackKind::Overlay, 30.0, 1, v.track_mute[1], v.track_solo[1]);
        push_track("A1 · Master", TrackKind::Audio, 42.0, 2, v.track_mute[2], v.track_solo[2]);
        for a in 0..na {
            let (mute, solo, si) = if a == 0 {
                (v.track_mute[3], v.track_solo[3], 3isize)
            } else {
                let e = a - 1;
                let base = 6 + v.extra_track_mute.len();
                (v.extra_audio_mute[e], v.extra_audio_solo[e], (base + e) as isize)
            };
            push_track(&format!("A{}", a + 2), TrackKind::Audio, 36.0, si, mute, solo);
        }
        push_track("Text", TrackKind::Text, 26.0, 4, v.track_mute[4], v.track_solo[4]);

        let end = v.total()
            .max(v.overlays.iter().map(|o| o.start + o.dur).fold(0.0_f32, f32::max))
            .max(v.music.iter().map(|m| m.start + m.dur).fold(0.0_f32, f32::max))
            .max(v.texts.iter().map(|t| t.a.max(if t.b > 1e8 { v.total() } else { t.b })).fold(0.0_f32, f32::max));

        let mut items = Vec::new();
        let mut refs = HashMap::new();
        let mut add = |item: TimelineItem, r: ItemRef| {
            refs.insert(item.uid, r);
            items.push(item);
        };
        let mk = |uid, track, start, dur, label: String, kind, compound: bool| TimelineItem {
            uid,
            track,
            start,
            dur,
            label,
            kind,
            selected: tui.is_selected(uid),
            locked: tui.locked.get(track).copied().unwrap_or(false),
            generator: false,
            compound,
        };

        let mut acc = 0.0_f32;
        for (i, c) in v.seq.iter().enumerate() {
            let start = acc;
            acc += c.len();
            add(mk(c.uid, 0, start, c.len(), c.name.clone(), TrackKind::Video, c.compound.is_some()), ItemRef::V1(i));
        }
        for (ti, track) in v.extra_tracks.iter().enumerate() {
            for (i, c) in track.iter().enumerate() {
                let mut item = mk(c.uid, 1 + ti, c.start, c.len(), c.name.clone(), TrackKind::Video, c.compound.is_some());
                item.generator = c.gen.is_some();
                add(item, ItemRef::Video { track: 1 + ti, index: i });
            }
        }
        for (i, o) in v.overlays.iter().enumerate() {
            add(mk(o.uid, nv, o.start, o.dur, o.nm.clone(), TrackKind::Overlay, false), ItemRef::Overlay(i));
        }
        // A1 is a continuous master waveform lane, not a clip lane.
        let a1_lane = nv + 1;
        let first_audio_lane = nv + 2;
        for (i, m) in v.music.iter().enumerate() {
            add(mk(m.uid, first_audio_lane + m.track, m.start, m.dur, m.nm.clone(), TrackKind::Audio, false), ItemRef::Music(i));
        }
        let text_lane = first_audio_lane + na;
        for (i, t) in v.texts.iter().enumerate() {
            let dur = if t.b > 1e8 { (end - t.a).max(0.25) } else { (t.b - t.a).max(0.25) };
            add(mk(t.uid, text_lane, t.a, dur, t.t.clone(), TrackKind::Text, false), ItemRef::Text(i));
        }
        let _ = a1_lane;
        (tracks, items, refs)
    }

    fn set_track_state(&mut self, idx: isize, mute: Option<bool>, solo: Option<bool>) {
        if idx < 0 { return; }
        let i = idx as usize;
        if i < self.video.track_mute.len() {
            if let Some(v) = mute { self.video.track_mute[i] = v; }
            if let Some(v) = solo { self.video.track_solo[i] = v; }
        } else if i >= 5 && i < 5 + self.video.extra_track_mute.len() {
            let e = i - 5;
            if let Some(v) = mute { self.video.extra_track_mute[e] = v; }
            if let Some(v) = solo { self.video.extra_track_solo[e] = v; }
        } else {
            let base = 6 + self.video.extra_track_mute.len();
            if i >= base && i < base + self.video.extra_audio_mute.len() {
                let e = i - base;
                if let Some(v) = mute { self.video.extra_audio_mute[e] = v; }
                if let Some(v) = solo { self.video.extra_audio_solo[e] = v; }
            }
        }
        if i == 2 { self.audio.mute = !self.video.track_active(2); }
        self.sync_audio_controls();
        self.video.dirty = true;
    }

    /// Visibility hides a video/FX/text lane from preview and export. Audio
    /// lanes use the eye only to dim their waveform; they stay audible.
    fn set_track_visibility(&mut self, lane: usize, state_idx: isize, kind: TrackKind, visible: bool) {
        if lane < self.timeline_ui.visible.len() { self.timeline_ui.visible[lane] = visible; }
        if kind != TrackKind::Audio && state_idx >= 0 {
            let i = state_idx as usize;
            if i < self.video.track_visible.len() { self.video.track_visible[i] = visible; }
        }
        self.video.dirty = true;
    }

    fn item_bounds(&self, r: ItemRef) -> Option<(f32, f32)> {
        match r {
            ItemRef::V1(i) => self.video.seq.get(i).map(|c| {
                let start: f32 = self.video.seq[..i].iter().map(|c| c.len()).sum();
                (start, c.len())
            }),
            ItemRef::Video { track, index } => self.video.extra_tracks.get(track.wrapping_sub(1))
                .and_then(|t| t.get(index)).map(|c| (c.start, c.len())),
            ItemRef::Overlay(i) => self.video.overlays.get(i).map(|o| (o.start, o.dur)),
            ItemRef::Music(i) => self.video.music.get(i).map(|m| (m.start, m.dur)),
            ItemRef::Text(i) => self.video.texts.get(i).map(|t| {
                let end = self.video.total();
                (t.a, if t.b > 1e8 { (end - t.a).max(0.25) } else { (t.b - t.a).max(0.25) })
            }),
        }
    }

    fn selected_refs(&self, refs: &HashMap<u64, ItemRef>) -> Vec<ItemRef> {
        self.timeline_ui.selection.iter()
            .filter(|uid| !self.is_uid_locked(**uid))
            .filter_map(|uid| refs.get(uid).copied())
            .collect()
    }

    fn select_uids_from_refs(&mut self, rs: &[ItemRef], refs: &HashMap<u64, ItemRef>) {
        let uids: Vec<u64> = refs.iter()
            .filter(|(_, r)| rs.contains(r))
            .map(|(uid, _)| *uid)
            .collect();
        self.timeline_ui.set_selection(uids);
        if let Some(&ItemRef::V1(i)) = rs.first() { self.video.sel = vec![i]; }
        else { self.video.sel.clear(); }
        self.video.extra_sel = rs.iter().find_map(|r| match r {
            ItemRef::Video { .. } => refs.iter().find(|(_, rr)| *rr == r).map(|(uid, _)| *uid),
            _ => None,
        });
    }

    fn is_uid_locked(&self, uid: u64) -> bool {
        self.timeline_snapshot().1.iter().find(|it| it.uid == uid).map(|it| it.locked).unwrap_or(false)
    }

    fn timeline_end(&self) -> f32 {
        let v = &self.video;
        v.total()
            .max(v.overlays.iter().map(|o| o.start + o.dur).fold(0.0_f32, f32::max))
            .max(v.music.iter().map(|m| m.start + m.dur).fold(0.0_f32, f32::max))
            .max(v.texts.iter().map(|t| t.a.max(if t.b > 1e8 { v.total() } else { t.b })).fold(0.0_f32, f32::max))
            .max(v.markers.iter().copied().fold(0.0_f32, f32::max))
    }

    fn render_timeline(&mut self, ui: &mut Ui) -> Option<std::path::PathBuf> {
        let mut imported: Option<std::path::PathBuf> = None;
        self.timeline_ui.zoom = self.video.tl_zoom.max(8.0);
        self.video.tl_zoom = self.timeline_ui.zoom;

        // Toolbar: tools / edit commands / track commands.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
            if ui.add(egui::Button::new(RichText::new("📂 Import").color(Color32::WHITE).size(10.5))
                .fill(ACCENT).rounding(Rounding::same(4.0))).clicked()
            {
                imported = media_engine::pick_file_dialog("Import Video, Audio, or Image onto Timeline");
            }
            ui.separator();
            for (tool, key, tip) in TimelineTool::ALL {
                let active = self.timeline_ui.tool == *tool;
                let label = format!("{} {}", key, tool.label());
                if ui.add(egui::Button::new(RichText::new(label).color(if active { CLIP_TXT } else { INK }).size(10.0))
                    .fill(if active { ACCENT2 } else { TBBG })
                    .stroke(Stroke::new(1.0_f32, if active { ACCENT } else { LINE }))
                    .rounding(Rounding::same(3.0))
                    .min_size(vec2(64.0, 21.0))).on_hover_text(*tip).clicked()
                {
                    self.timeline_ui.tool = *tool;
                }
            }
            ui.separator();
            let snap = self.timeline_ui.snap;
            if ui.add(egui::Button::new(RichText::new(if snap { "🧲 Snap ON" } else { "Snap OFF" })
                .color(if snap { CLIP_TXT } else { SUB }).size(10.0))
                .fill(if snap { ACCENT2 } else { TBBG }).rounding(Rounding::same(3.0))).clicked()
            {
                self.timeline_ui.snap = !snap;
            }
            for (label, action) in [
                ("✂ Split", 0u8), ("✕ Ripple Del", 1), ("⌫ Lift", 6),
                ("⧉ Duplicate", 2), ("◆ Marker", 3), ("⧉ Group", 4), ("Ungroup", 5),
            ] {
                if ui.add(egui::Button::new(RichText::new(label).color(INK).size(10.0))
                    .fill(TBBG).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0))).clicked()
                {
                    match action {
                        0 => self.split_at_playhead(),
                        1 => self.delete_selected(true),
                        6 => self.delete_selected(false),
                        2 => self.duplicate_selected(),
                        3 => { let t = self.video.playhead; self.video.add_marker(t); }
                        4 => self.group_selected(),
                        5 => self.ungroup_selected(),
                        _ => {}
                    }
                }
            }
            ui.separator();
            for (label, action) in [
                ("+V", 0u8), ("−V", 1), ("+A", 2), ("−A", 3),
                ("+Clip", 4), ("+FX", 5), ("+Title", 6),
            ] {
                if ui.add(egui::Button::new(RichText::new(label).color(INK).size(10.0))
                    .fill(TBBG).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0))).clicked()
                {
                    match action {
                        0 => {
                            self.video.add_video_track();
                            self.timeline_ui.ensure_tracks(5 + self.video.extra_tracks.len() + self.video.extra_audio_mute.len());
                        }
                        1 => { self.video.delete_video_track(); }
                        2 => {
                            self.video.add_audio_track();
                            self.timeline_ui.ensure_tracks(5 + self.video.extra_tracks.len() + self.video.extra_audio_mute.len());
                        }
                        3 => self.video.delete_audio_track(),
                        4 => {
                            let mut c = VideoClip::new(format!("clip_{}", self.video.seq.len() + 1), self.video.duration.max(1.0), self.video.media_path.clone());
                            c.a = self.video.trim_start.clamp(0.0, 0.99);
                            c.b = c.a.max(0.01) + (self.video.trim_end - self.video.trim_start).max(0.01);
                            c.b = c.b.min(1.0);
                            c.speed = self.video.speed.max(0.01);
                            self.video.insert_clip_on_track(c, self.gen_clip_track.min(self.video.video_track_count() - 1));
                        }
                        5 => self.video.overlays.push(OverlayClip {
                            uid: fresh_clip_uid(),
                            nm: format!("overlay_{}", self.video.overlays.len() + 1),
                            dur: 5.0, start: self.video.playhead,
                            x: 75.0, y: 75.0, s: 30.0, o: 100.0,
                            path: self.video.media_path.clone(),
                        }),
                        6 => self.video.texts.push(TextOverlay {
                            uid: fresh_clip_uid(),
                            t: "Title".into(), size: 10.0, x: 50.0, y: 50.0,
                            a: self.video.playhead,
                            b: (self.video.playhead + 4.0).min(self.timeline_end().max(4.0)),
                            col: Color32::WHITE,
                        }),
                        _ => {}
                    }
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(Slider::new(&mut self.video.tl_zoom, 12.0..=320.0).show_value(false).fixed_decimals(0));
                ui.label(RichText::new("Zoom").color(SUB).size(10.0));
            });
        });

        let lane_count = 5 + self.video.extra_tracks.len() + self.video.extra_audio_mute.len();
        self.timeline_ui.shrink_tracks(lane_count);
        self.timeline_ui.ensure_tracks(lane_count);
        let (tracks, items, _refs) = self.timeline_snapshot();

        let header_w = 132.0_f32;
        let ruler_h = 26.0_f32;
        let heights: Vec<f32> = tracks.iter().map(|t| t.header.height).collect();
        let mut lane_tops = Vec::with_capacity(tracks.len());
        let mut ly = ruler_h;
        for h in &heights { lane_tops.push(ly); ly += *h; }
        let total_h = ly;
        let kinds: Vec<TrackKind> = tracks.iter().map(|t| t.header.kind).collect();
        let nv = self.video.video_track_count();
        let na = self.video.audio_track_count();
        let first_audio_lane = nv + 2;
        let text_lane = first_audio_lane + na;
        let _ = text_lane;

        let end = self.timeline_end().max(1.0);
        let zoom = self.timeline_ui.zoom;
        let new_video_lane = tracks.len();
        let last_lane_bottom = lane_tops.last().copied().unwrap_or(0.0)
            + heights.last().copied().unwrap_or(0.0);
        let lane_at = |y: f32| -> Option<usize> {
            for (i, top) in lane_tops.iter().enumerate() {
                if y >= *top && y < *top + heights[i] { return Some(i); }
            }
            // Empty space below all tracks is a new Vn+1 lane: clips can always
            // be dropped onto a track that did not exist when the drag began.
            if y >= last_lane_bottom { return Some(new_video_lane); }
            None
        };
        let mut candidates = timeline::snap_candidates(&items, self.video.playhead, end);
        candidates.extend(self.video.markers.iter().copied());
        let zoom_now = zoom;
        let snap_t = self.timeline_ui.snap;
        let snap_time = |t: f32| {
            if snap_t {
                let mut best = t.max(0.0);
                let mut best_d = 8.0 / zoom_now.max(1.0);
                for &c in &candidates {
                    let d = (t - c).abs();
                    if d < best_d { best_d = d; best = c; }
                }
                best.max(0.0)
            } else { t.max(0.0) }
        };

        // One vertical scroll area keeps headers and lanes aligned when many
        // tracks are added; only time uses the nested horizontal scroll area.
        egui::ScrollArea::vertical()
            .id_source("grain_timeline_vertical")
            .auto_shrink([false, false])
            .show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // Track headers -------------------------------------------------
            let (hdr, _) = ui.allocate_exact_size(vec2(header_w, total_h), Sense::hover());
            let hp = ui.painter();
            hp.rect_filled(hdr, 0.0, PANEL2);
            hp.rect_filled(Rect::from_min_size(hdr.min, vec2(header_w, ruler_h)), 0.0, PANEL);
            hp.text(pos2(hdr.left() + 8.0, hdr.top() + ruler_h / 2.0), egui::Align2::LEFT_CENTER,
                "TRACKS", FontId::monospace(9.0), SUB);
            hp.line_segment([pos2(hdr.left(), hdr.top() + ruler_h), pos2(hdr.right(), hdr.top() + ruler_h)], Stroke::new(1.0_f32, LINE));
            for (i, tr) in tracks.iter().enumerate() {
                let y = hdr.top() + lane_tops[i];
                let h = tr.header.height;
                let col = tr.header.kind.color();
                hp.rect_filled(Rect::from_min_size(pos2(hdr.left(), y), vec2(4.0, h)), 0.0, col);
                hp.text(pos2(hdr.left() + 10.0, y + h / 2.0), egui::Align2::LEFT_CENTER,
                    &tr.header.name, FontId::proportional(10.0), if tr.header.visible { INK } else { SUB });
                let name_r = Rect::from_min_size(pos2(hdr.left() + 8.0, y), vec2(44.0, h));
                if ui.interact(name_r, Id::new(("tlhdr_target", i)), Sense::click()).on_hover_text("Toggle record target").clicked() {
                    self.timeline_ui.target[i] = !tr.header.target;
                }
                if tr.header.target {
                    hp.rect_stroke(Rect::from_min_size(pos2(hdr.left() + 5.0, y + 2.0), vec2(header_w - 10.0, h - 4.0)), 2.0, Stroke::new(1.0_f32, ACCENT));
                }
                let btn = |x: f32, label: &str, on: bool, id: &str| {
                    let r = Rect::from_center_size(pos2(hdr.left() + x, y + h / 2.0), vec2(16.0, 15.0));
                    let resp = ui.interact(r, Id::new((id, i)), Sense::click());
                    if on { hp.rect_filled(r, 2.0, ACCENT2); }
                    else if resp.hovered() { hp.rect_filled(r, 2.0, LINE); }
                    hp.text(r.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(8.5),
                        if on { CLIP_TXT } else { SUB });
                    resp.clicked()
                };
                if btn(58.0, "M", tr.header.mute, "tlhdr_m") { self.set_track_state(tr.state_idx, Some(!tr.header.mute), None); }
                if btn(76.0, "S", tr.header.solo, "tlhdr_s") { self.set_track_state(tr.state_idx, None, Some(!tr.header.solo)); }
                if btn(94.0, "L", tr.header.lock, "tlhdr_l") { self.timeline_ui.locked[i] = !tr.header.lock; }
                if btn(112.0, "👁", tr.header.visible, "tlhdr_v") {
                    self.set_track_visibility(i, tr.state_idx, tr.header.kind, !tr.header.visible);
                }
                hp.line_segment([pos2(hdr.left(), y + h), pos2(hdr.right(), y + h)], Stroke::new(1.0_f32, LINE));
            }
            hp.line_segment([pos2(hdr.right(), hdr.top()), pos2(hdr.right(), hdr.bottom())], Stroke::new(1.0_f32, LINE));

            // Timeline canvas ------------------------------------------------
            let avail_w = ui.available_width().max(160.0);
            let body_w = (end * zoom + 120.0).max(avail_w);
            egui::ScrollArea::horizontal()
                .id_source("grain_timeline_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let (body, _) = ui.allocate_exact_size(vec2(body_w, total_h), Sense::hover());
                    self.video.timeline_drop_rect = Some(body);
                    let p = ui.painter_at(body);
                    let pointer = ui.input(|i| i.pointer.hover_pos());
                    let over_body = pointer.map(|q| body.contains(q)).unwrap_or(false);
                    let time_at = |x: f32| ((x - body.left()) / zoom).clamp(0.0, end);

                    if over_body {
                        let q = pointer.unwrap();
                        let lane = lane_at(q.y - body.top());
                        self.tl_drop_target = lane.map(|l| (l, time_at(q.x)));
                    } else {
                        self.tl_drop_target = None;
                    }

                    // Lane backgrounds.
                    for (i, tr) in tracks.iter().enumerate() {
                        let r = Rect::from_min_size(pos2(body.left(), body.top() + lane_tops[i]), vec2(body_w, tr.header.height));
                        let bg = if i % 2 == 0 { TLBG } else { Color32::from_rgb(0x21, 0x21, 0x21) };
                        p.rect_filled(r, 0.0, bg);
                        p.line_segment([pos2(r.left(), r.bottom()), pos2(r.right(), r.bottom())], Stroke::new(1.0_f32, LINE));
                    }
                    p.rect_filled(Rect::from_min_size(body.min, vec2(body_w, ruler_h)), 0.0, Color32::from_rgb(0x26, 0x26, 0x26));

                    // Ruler.
                    let step = if zoom >= 160.0 { 1.0 } else if zoom >= 80.0 { 2.0 } else if zoom >= 40.0 { 5.0 } else { 10.0 };
                    let mut t = 0.0_f32;
                    while t <= end + step {
                        let x = body.left() + t * zoom;
                        p.line_segment([pos2(x, body.top()), pos2(x, body.top() + ruler_h)], Stroke::new(1.0_f32, Color32::from_rgb(90, 90, 90)));
                        p.text(pos2(x + 3.0, body.top() + ruler_h / 2.0), egui::Align2::LEFT_CENTER,
                            fmt_tc(t), FontId::monospace(9.0), SUB);
                        t += step;
                    }

                    // Markers + keyframes.
                    for &m in &self.video.markers {
                        let mx = body.left() + m * zoom;
                        p.add(egui::Shape::convex_polygon(vec![
                            pos2(mx - 5.0, body.top() + ruler_h), pos2(mx + 5.0, body.top() + ruler_h),
                            pos2(mx, body.top() + ruler_h + 9.0),
                        ], AMBER, Stroke::NONE));
                    }
                    for k in &self.video.keyframes {
                        let kx = body.left() + k.t * zoom;
                        p.add(egui::Shape::convex_polygon(vec![
                            pos2(kx - 4.0, body.top() + ruler_h), pos2(kx + 4.0, body.top() + ruler_h),
                            pos2(kx, body.top() + ruler_h + 7.0),
                        ], ACCENT2, Stroke::NONE));
                    }

                    // A1 master waveform.
                    let a1_lane = nv + 1;
                    let a1r = Rect::from_min_size(pos2(body.left(), body.top() + lane_tops[a1_lane] + 3.0),
                        vec2((self.video.total() * zoom).max(8.0), heights[a1_lane] - 6.0));
                    p.rect_filled(a1r, 2.0, CLIP_A);
                    if self.timeline_ui.show_waveforms { Self::draw_waveform(&p, a1r, &self.audio.samples); }
                    p.text(pos2(a1r.left() + 6.0, a1r.center().y), egui::Align2::LEFT_CENTER,
                        format!("{} · master", self.audio.filename), FontId::proportional(9.0), CLIP_TXT);

                    // Clips. Dragged clips render live at their target lane/time.
                    let drag_snapshot = self.timeline_ui.drag.clone();
                    for it in &items {
                        if it.track == a1_lane { continue; }
                        let first_drag_start = drag_snapshot.as_ref()
                            .and_then(|d| d.starts.first().copied()).unwrap_or(0.0);
                        let (render_track, render_start) = if let Some(d) = &drag_snapshot {
                            if d.uids.contains(&it.uid) && d.mode == TlDragMode::Move {
                                let original = d.uids.iter().position(|u| *u == it.uid)
                                    .and_then(|idx| d.starts.get(idx).copied())
                                    .unwrap_or(it.start);
                                (d.target_track.min(tracks.len().saturating_sub(1)),
                                 (d.target_start + original - first_drag_start).max(0.0))
                            } else { (it.track, it.start) }
                        } else { (it.track, it.start) };
                        if render_track >= tracks.len() { continue; }
                        let r = Rect::from_min_size(pos2(body.left() + render_start * zoom, body.top() + lane_tops[render_track] + 3.0),
                            vec2((it.dur * zoom).max(8.0), heights[render_track] - 6.0));
                        let mut fill = it.kind.color();
                        if !tracks[render_track].header.visible { fill = Color32::from_gray(48); }
                        p.rect_filled(r, 2.0, fill);
                        if it.compound { p.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 3.0)), 0.0, CLIP_CMP); }
                        // Thumbnail / waveform strip.
                        if self.timeline_ui.show_waveforms && it.kind == TrackKind::Audio {
                            Self::draw_waveform(&p, r, &self.audio.samples);
                        } else if it.kind == TrackKind::Video {
                            let mut x = r.left() + 1.0;
                            let mut n = 0u32;
                            while x < r.right() - 1.0 {
                                let w = 14.0_f32.min(r.right() - x);
                                if n % 2 == 0 {
                                    p.rect_filled(Rect::from_min_size(pos2(x, r.top()), vec2(w, r.height())), 0.0,
                                        Color32::from_rgba_unmultiplied(0, 0, 0, 45));
                                }
                                x += w; n += 1;
                            }
                        }
                        // Trim handles.
                        let hw = 10.0_f32.min(r.width() * 0.33);
                        p.rect_filled(Rect::from_min_size(r.min, vec2(hw, r.height())), 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 70));
                        p.rect_filled(Rect::from_min_size(pos2(r.right() - hw, r.top()), vec2(hw, r.height())), 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 70));
                        let label = if it.kind == TrackKind::Text { it.label.clone() } else { format!("{}  {:.1}s", it.label, it.dur) };
                        p.text(pos2(r.left() + 8.0, r.center().y), egui::Align2::LEFT_CENTER, label,
                            FontId::proportional(9.5), CLIP_TXT);
                        if it.selected {
                            p.rect_stroke(r, 2.0, Stroke::new(2.0_f32, Color32::WHITE));
                            p.rect_stroke(r.shrink(2.0), 1.0, Stroke::new(1.0_f32, ACCENT));
                        }
                    }

                    // Drag ghost / marquee.
                    if let Some(d) = &self.timeline_ui.drag {
                        if d.target_track < tracks.len() {
                            let ghost = Rect::from_min_size(
                                pos2(body.left() + d.target_start * zoom, body.top() + lane_tops[d.target_track] + 2.0),
                                vec2(12.0_f32.max(zoom * 0.4), heights[d.target_track] - 4.0),
                            );
                            p.rect_stroke(ghost, 2.0, Stroke::new(2.0_f32, AMBER));
                        }
                    }
                    if let Some(m) = self.timeline_ui.marquee { p.rect_filled(m, 0.0, Color32::from_rgba_unmultiplied(59, 196, 232, 35)); p.rect_stroke(m, 0.0, Stroke::new(1.0_f32, ACCENT)); }

                    // Playhead.
                    let phx = body.left() + self.video.playhead.clamp(0.0, end) * zoom;
                    p.line_segment([pos2(phx, body.top()), pos2(phx, body.bottom())], Stroke::new(1.5_f32, Color32::WHITE));
                    p.add(egui::Shape::convex_polygon(vec![
                        pos2(phx - 6.0, body.top()), pos2(phx + 6.0, body.top()), pos2(phx, body.top() + 10.0),
                    ], Color32::WHITE, Stroke::NONE));

                    // Scrub on the ruler.
                    let ruler_rect = Rect::from_min_size(body.min, vec2(body_w, ruler_h));
                    let sr = ui.interact(ruler_rect, Id::new("tl_ruler_seek"), Sense::click_and_drag());
                    if sr.clicked() || sr.dragged() || sr.drag_released() {
                        if let Some(q) = sr.interact_pointer_pos() {
                            let t = snap_time(time_at(q.x));
                            self.seek_transport(t);
                        }
                    }

                    // Tools / drag handling.
                    let (pressed, down, released, alt) = ui.input(|i| (
                        i.pointer.button_pressed(egui::PointerButton::Primary),
                        i.pointer.button_down(egui::PointerButton::Primary),
                        i.pointer.button_released(egui::PointerButton::Primary),
                        i.modifiers.alt,
                    ));
                    let tool = self.timeline_ui.tool;

                    if over_body && pressed && !sr.clicked() {
                        let q = pointer.unwrap();
                        let qy = q.y - body.top();
                        if let Some(_lane) = lane_at(qy) {
                            match tool {
                                TimelineTool::Zoom => {
                                    let anchor = time_at(q.x);
                                    self.timeline_ui.zoom_at(if alt { 0.8 } else { 1.25 }, anchor, q.x);
                                }
                                TimelineTool::Hand => {}
                                _ => {
                                    if let Some((hi, mode)) = timeline::hit_item(&items, q, body.min, zoom, &lane_tops, &heights) {
                                        let it = &items[hi];
                                        if !tracks[it.track].header.lock && tracks[it.track].header.visible {
                                            if tool == TimelineTool::Razor {
                                                let t = snap_time(time_at(q.x));
                                                self.split_item_at(it.uid, t);
                                            } else {
                                                let additive = ui.input(|i| i.modifiers.ctrl || i.modifiers.shift);
                                                if !it.selected {
                                                    self.timeline_ui.select(it.uid, additive);
                                                } else if additive {
                                                    self.timeline_ui.select(it.uid, true);
                                                }
                                                let uids: Vec<u64> = self.timeline_ui.selection.iter().copied()
                                                    .filter(|u| {
                                                        items.iter().find(|it| it.uid == *u)
                                                            .map(|it| !it.locked && !tracks[it.track].header.lock)
                                                            .unwrap_or(false)
                                                    }).collect();
                                                let starts: Vec<f32> = uids.iter()
                                                    .filter_map(|u| items.iter().find(|it| it.uid == *u).map(|it| it.start))
                                                    .collect();
                                                let mode = match tool {
                                                    TimelineTool::Slip => TlDragMode::Slip,
                                                    TimelineTool::Slide => TlDragMode::Slide,
                                                    _ => mode,
                                                };
                                                self.timeline_ui.drag = Some(ItemDrag {
                                                    uids, from_track: it.track, pointer_start: q,
                                                    starts, target_track: it.track,
                                                    target_start: it.start, mode,
                                                });
                                            }
                                        }
                                    } else if tool == TimelineTool::Select {
                                        if !ui.input(|i| i.modifiers.ctrl || i.modifiers.shift) {
                                            self.timeline_ui.clear_selection();
                                            self.video.sel.clear();
                                            self.video.extra_sel = None;
                                        }
                                        self.timeline_ui.marquee = Some(Rect::from_min_size(q, egui::Vec2::ZERO));
                                    }
                                }
                            }
                        }
                    }

                    if let Some(d) = self.timeline_ui.drag.as_mut() {
                        if down {
                            if let Some(q) = pointer {
                                let delta = (q.x - d.pointer_start.x) / zoom;
                                let first = d.starts.first().copied().unwrap_or(0.0);
                                let first_item = items.iter().find(|it| d.uids.first().copied() == Some(it.uid));
                                let first_dur = first_item.map(|it| it.dur).unwrap_or(0.0);
                                let mut ns = first + delta;
                                if snap_t && d.mode != TlDragMode::Slip && delta.abs() * zoom >= 3.0 {
                                    // Snap the edge actually being dragged; never snap slip to timeline edges.
                                    let undragged: Vec<TimelineItem> = items.iter()
                                        .filter(|it| !d.uids.contains(&it.uid)).cloned().collect();
                                    let mut drag_candidates = timeline::snap_candidates(&undragged, self.video.playhead, end);
                                    drag_candidates.extend(self.video.markers.iter().copied());
                                    let base = if d.mode == TlDragMode::TrimOut { first + first_dur } else { first };
                                    drag_candidates.retain(|c| (*c - base).abs() > 0.001);
                                    let mut best = base + delta;
                                    let mut best_d = 6.0 / zoom.max(1.0);
                                    let edge = best;
                                    for &c in &drag_candidates {
                                        let dd = (edge - c).abs();
                                        if dd < best_d { best_d = dd; best = c; }
                                    }
                                    ns = match d.mode {
                                        TlDragMode::TrimOut => first + best - base,
                                        _ => best,
                                    };
                                }
                                if d.mode == TlDragMode::Slip { d.target_start = ns.max(0.0); }
                                else { d.target_start = ns.clamp(0.0, end); }
                                if let Some(lane) = lane_at(q.y - body.top()) { d.target_track = lane; }
                            }
                        }
                    }
                    if let Some(m) = self.timeline_ui.marquee.as_mut() {
                        if down {
                            if let Some(q) = pointer { *m = Rect::from_two_pos(m.min, q); }
                        }
                    }
                    if tool == TimelineTool::Hand && down && over_body {
                        let delta = ui.input(|i| i.pointer.delta());
                        ui.scroll_with_delta(vec2(-delta.x, -delta.y));
                    }

                    if released {
                        if let Some(m) = self.timeline_ui.marquee.take() {
                            let mut picked = Vec::new();
                            for it in &items {
                                let r = Rect::from_min_size(pos2(body.left() + it.start * zoom, body.top() + lane_tops[it.track]),
                                    vec2((it.dur * zoom).max(8.0), heights[it.track]));
                                if !it.locked && r.intersects(m) { picked.push(it.uid); }
                            }
                            self.timeline_ui.set_selection(picked);
                            let picked_refs = self.selected_refs(&_refs);
                            self.select_uids_from_refs(&picked_refs, &_refs);
                        }
                        if let Some(d) = self.timeline_ui.drag.take() {
                            self.commit_timeline_drag(&d, &items, &kinds, first_audio_lane, zoom);
                        }
                    }
                });
        });
            });

        // Every scrub/seek path calls `seek_transport`, which owns both clocks.
        // Do not overwrite `audio.playhead` here; `render_timeline` runs after
        // transport reconciliation and would otherwise break A/V sync.
        self.video.tl_zoom = self.timeline_ui.zoom;
        self.video.status = format!("{} · {} track(s) · {} selected",
            self.timeline_ui.tool.label(), tracks.len(), self.timeline_ui.selection.len());
        imported
    }

    fn draw_waveform(p: &egui::Painter, rect: Rect, samples: &[f32]) {
        let mid = rect.center().y;
        let n = 64u32;
        let bw = rect.width() / n as f32;
        let col = Color32::from_rgba_unmultiplied(255, 255, 255, 90);
        if !samples.is_empty() {
            // real peaks: min/max per column across the clip's slice of the buffer
            let per = (samples.len() as f32 / n as f32).ceil() as usize;
            for i in 0..n {
                let s0 = (i as usize * per).min(samples.len());
                let s1 = (s0 + per).min(samples.len());
                let (mut mn, mut mx) = (0.0_f32, 0.0_f32);
                for s in &samples[s0..s1] { let v = *s; if v < mn { mn = v; } if v > mx { mx = v; } }
                let amp = (mx - mn).min(2.0) * 0.5;
                let h = amp * rect.height() * 0.9;
                let x = rect.left() + i as f32 * bw;
                p.rect_filled(Rect::from_min_max(pos2(x, mid - h / 2.0), pos2(x + bw * 0.7, mid + h / 2.0)), 0.0, col);
            }
            return;
        }
        // synthetic fallback when no audio is loaded
        for i in 0..n {
            let x = rect.left() + i as f32 * bw;
            let amp = ((i as f32 * 0.4).sin() * 0.5 + 0.5) * (0.45 + 0.55 * ((i as f32 * 0.13).sin()).abs());
            let h = amp * rect.height() * 0.8;
            p.rect_filled(Rect::from_min_max(pos2(x, mid - h / 2.0), pos2(x + bw * 0.7, mid + h / 2.0)), 0.0, Color32::from_rgba_unmultiplied(255, 255, 255, 55));
        }
    }


    /// Commit a drag from the timeline UI. Every edit is resolved by UID so a
    /// reorder mid-drag can never retarget the wrong clip.
    fn commit_timeline_drag(
        &mut self,
        d: &ItemDrag,
        _items: &[TimelineItem],
        kinds: &[TrackKind],
        first_audio_lane: usize,
        zoom: f32,
    ) {
        let (_, _, refs) = self.timeline_snapshot();
        let first = d.starts.first().copied().unwrap_or(d.target_start);
        let delta = d.target_start - first;
        let kinds_len = kinds.len();
        let new_video_track = kinds.iter().take_while(|k| **k == TrackKind::Video).count();
        let target_video_track = if d.target_track == kinds_len { new_video_track } else { d.target_track };
        let target_kind = if d.target_track == kinds_len {
            TrackKind::Video
        } else {
            kinds.get(d.target_track).copied().unwrap_or(TrackKind::Video)
        };

        match d.mode {
            TlDragMode::Move | TlDragMode::Slide => {
                let mut v1: Vec<(u64, f32)> = Vec::new();
                for (uid, orig) in d.uids.iter().zip(d.starts.iter()) {
                    if self.is_uid_locked(*uid) { continue; }
                    let r = match refs.get(uid) { Some(r) => *r, None => continue };
                    let ns = (d.target_start + (*orig - first)).max(0.0);
                    match r {
                        ItemRef::V1(_) => v1.push((*uid, ns)),
                        _ => self.move_item_to(r, ns, target_video_track, target_kind, first_audio_lane, d.mode == TlDragMode::Slide),
                    }
                }
                if !v1.is_empty() {
                    if target_kind == TrackKind::Video && target_video_track == 0 {
                        self.move_v1_items(&v1);
                    } else if target_kind == TrackKind::Video {
                        let vt = target_video_track.max(1);
                        self.move_v1_items_to_extra(&v1, vt);
                    }
                }
                self.video.status = format!("Moved {} clip(s)", d.uids.len());
            }
            TlDragMode::TrimIn | TlDragMode::TrimOut => {
                let dt = (d.target_start - first).max(-100.0).min(100.0);
                for uid in &d.uids {
                    if self.is_uid_locked(*uid) { continue; }
                    if let Some(&r) = refs.get(uid) { self.trim_item(r, d.mode, dt); }
                }
            }
            TlDragMode::Slip => {
                for uid in &d.uids {
                    if self.is_uid_locked(*uid) { continue; }
                    if let Some(&r) = refs.get(uid) { self.slip_item(r, delta); }
                }
            }
        }
        self.video.dirty = true;
        self.project_dirty = true;
        let _ = zoom;
    }

    /// Reorder contiguous V1 clips using their desired start times.
    fn move_v1_items(&mut self, moved: &[(u64, f32)]) {
        let uids: Vec<u64> = moved.iter().map(|(u, _)| *u).collect();
        let block: Vec<(VideoClip, f32)> = self.video.seq.iter()
            .filter(|c| uids.contains(&c.uid) && !self.is_uid_locked(c.uid))
            .map(|c| {
                let start = moved.iter().find(|(u, _)| *u == c.uid).map(|(_, s)| *s).unwrap_or(0.0);
                (c.clone(), start)
            })
            .collect();
        if block.is_empty() { return; }
        let mut block = block;
        block.sort_by(|a, b| a.1.total_cmp(&b.1));
        self.video.seq.retain(|c| !uids.contains(&c.uid));
        let mut ins = self.video.seq.len();
        let mut acc = 0.0_f32;
        for (i, c) in self.video.seq.iter().enumerate() {
            let len = c.len();
            if block[0].1 < acc + len * 0.5 { ins = i; break; }
            acc += len;
        }
        let new_uids: Vec<u64> = block.iter().map(|(c, _)| c.uid).collect();
        self.video.seq.splice(ins..ins, block.into_iter().map(|(c, _)| c));
        self.timeline_ui.set_selection(new_uids);
        self.video.sel = (ins..ins + uids.len()).collect();
        self.video.video_stream = None;
    }

    fn move_v1_items_to_extra(&mut self, moved: &[(u64, f32)], vt: usize) {
        if vt == 0 { return; }
        self.ensure_video_lane(vt);
        let uids: Vec<u64> = moved.iter().map(|(u, _)| *u).collect();
        let mut block: Vec<VideoClip> = self.video.seq.iter()
            .filter(|c| uids.contains(&c.uid) && !self.is_uid_locked(c.uid)).cloned().collect();
        block.sort_by(|a, b| {
            let sa = moved.iter().find(|(u, _)| *u == a.uid).map(|(_, s)| *s).unwrap_or(0.0);
            let sb = moved.iter().find(|(u, _)| *u == b.uid).map(|(_, s)| *s).unwrap_or(0.0);
            sa.total_cmp(&sb)
        });
        self.video.seq.retain(|c| !uids.contains(&c.uid));
        for mut c in block {
            let start = moved.iter().find(|(u, _)| *u == c.uid).map(|(_, s)| *s).unwrap_or(0.0);
            c.track = vt;
            c.start = start.max(0.0);
            self.video.extra_tracks[vt - 1].push(c);
        }
        self.video.video_stream = None;
    }

    fn ensure_video_lane(&mut self, lane: usize) {
        // `lane` is a video track index; V1 is lane 0. Create any number of
        // extra tracks so a clip can always be dropped onto a new lane.
        while self.video.extra_tracks.len() < lane {
            self.video.add_video_track();
        }
        self.timeline_ui.ensure_tracks(5 + self.video.extra_tracks.len() + self.video.extra_audio_mute.len());
    }

    fn move_item_to(
        &mut self,
        r: ItemRef,
        start: f32,
        lane: usize,
        kind: TrackKind,
        first_audio_lane: usize,
        ripple: bool,
    ) {
        let old = match self.item_bounds(r) { Some(v) => v, None => return };
        let uid = match r {
            ItemRef::V1(i) => self.video.seq.get(i).map(|c| c.uid),
            ItemRef::Video { track, index } => self.video.extra_tracks.get(track.wrapping_sub(1)).and_then(|t| t.get(index)).map(|c| c.uid),
            ItemRef::Music(i) => self.video.music.get(i).map(|m| m.uid),
            _ => None,
        }.unwrap_or(0);
        if self.is_uid_locked(uid) { return; }
        match r {
            ItemRef::V1(_) => {} // handled by move_v1_items
            ItemRef::Video { track, index } => {
                if kind != TrackKind::Video { return; }
                if lane == 0 {
                    if let Some(mut c) = self.video.extra_tracks.get(track.wrapping_sub(1)).and_then(|t| t.get(index)).cloned() {
                        self.video.extra_tracks[track - 1].remove(index);
                        c.track = 0;
                        c.start = start;
                        let mut pos = self.video.seq.len();
                        let mut acc = 0.0_f32;
                        for (i, seq_c) in self.video.seq.iter().enumerate() {
                            if start < acc + seq_c.len() * 0.5 { pos = i; break; }
                            acc += seq_c.len();
                        }
                        self.video.seq.insert(pos, c);
                    }
                } else if lane >= 1 {
                    self.ensure_video_lane(lane);
                    if let Some(mut c) = self.video.extra_tracks.get(track.wrapping_sub(1)).and_then(|t| t.get(index)).cloned() {
                        if let Some(t) = self.video.extra_tracks.get_mut(track - 1) { t.remove(index); }
                        c.start = start.max(0.0);
                        c.track = lane;
                        self.video.extra_tracks[lane - 1].push(c);
                    }
                }
                if ripple && lane == track { self.ripple_video(track, old.0, old.1, start - old.0, uid); }
            }
            ItemRef::Overlay(i) => {
                if kind != TrackKind::Overlay { return; }
                if let Some(o) = self.video.overlays.get_mut(i) { o.start = start; }
            }
            ItemRef::Music(i) => {
                if kind != TrackKind::Audio || lane < first_audio_lane { return; }
                let t = lane - first_audio_lane;
                if let Some(m) = self.video.music.get_mut(i) { m.start = start; m.track = t; }
                if ripple { self.ripple_music(t, old.0, old.1, start - old.0, uid); }
            }
            ItemRef::Text(i) => {
                if kind != TrackKind::Text { return; }
                if let Some(t) = self.video.texts.get_mut(i) {
                    let dur = old.1;
                    t.a = start;
                    if t.b <= 1e8 { t.b = start + dur; }
                }
            }
        }
    }

    fn ripple_video(&mut self, track: usize, old_start: f32, dur: f32, delta: f32, skip_uid: u64) {
        if delta.abs() < 0.001 { return; }
        if let Some(t) = self.video.extra_tracks.get_mut(track.wrapping_sub(1)) {
            for c in t.iter_mut() {
                if c.uid == skip_uid || c.start <= old_start + 0.001 || c.start >= old_start + dur - 0.001 { continue; }
                c.start = (c.start + delta).max(0.0);
            }
        }
    }

    fn ripple_music(&mut self, track: usize, old_start: f32, dur: f32, delta: f32, skip_uid: u64) {
        if delta.abs() < 0.001 { return; }
        for m in self.video.music.iter_mut() {
            if m.uid == skip_uid || m.track != track || m.start <= old_start + 0.001 || m.start >= old_start + dur - 0.001 { continue; }
            m.start = (m.start + delta).max(0.0);
        }
    }

    fn trim_item(&mut self, r: ItemRef, mode: TlDragMode, dt: f32) {
        let min_span = timeline::MIN_CLIP;
        let uid = match r {
            ItemRef::V1(i) => self.video.seq.get(i).map(|c| c.uid),
            ItemRef::Video { track, index } => self.video.extra_tracks.get(track.wrapping_sub(1)).and_then(|t| t.get(index)).map(|c| c.uid),
            ItemRef::Overlay(i) => self.video.overlays.get(i).map(|o| o.uid),
            ItemRef::Music(i) => self.video.music.get(i).map(|m| m.uid),
            ItemRef::Text(i) => self.video.texts.get(i).map(|t| t.uid),
        }.unwrap_or(0);
        if self.is_uid_locked(uid) { return; }
        match r {
            ItemRef::V1(i) => {
                if let Some(c) = self.video.seq.get_mut(i) { Self::trim_video_clip(c, mode, dt, min_span); }
            }
            ItemRef::Video { track, index } => {
                if let Some(c) = self.video.extra_tracks.get_mut(track.wrapping_sub(1)).and_then(|t| t.get_mut(index)) {
                    let right = c.start + c.len();
                    Self::trim_video_clip(c, mode, dt, min_span);
                    if mode == TlDragMode::TrimIn {
                        // Trim-in keeps the right edge fixed; only `a` changes
                        // source in-point, so explicit `start` must move too.
                        c.start = (right - c.len()).max(0.0);
                    }
                }
            }
            ItemRef::Overlay(i) => {
                if let Some(o) = self.video.overlays.get_mut(i) {
                    match mode {
                        TlDragMode::TrimIn => { let d = dt.max(-o.start).min(o.dur - min_span); o.start += d; o.dur -= d; }
                        TlDragMode::TrimOut => { o.dur = (o.dur + dt).max(min_span); }
                        _ => {}
                    }
                }
            }
            ItemRef::Music(i) => {
                if let Some(m) = self.video.music.get_mut(i) {
                    match mode {
                        TlDragMode::TrimIn => { let d = dt.max(-m.start).min(m.dur - min_span); m.start += d; m.dur -= d; }
                        TlDragMode::TrimOut => { m.dur = (m.dur + dt).max(min_span); }
                        _ => {}
                    }
                }
            }
            ItemRef::Text(i) => {
                let timeline_end = self.timeline_end();
                if let Some(t) = self.video.texts.get_mut(i) {
                    if t.b > 1e8 { t.b = timeline_end; }
                    match mode {
                        TlDragMode::TrimIn => { let d = dt.max(-t.a).min(t.b - t.a - min_span); t.a += d; }
                        TlDragMode::TrimOut => { t.b = (t.b + dt).max(t.a + min_span); }
                        _ => {}
                    }
                }
            }
        }
        self.video.dirty = true;
    }

    fn trim_video_clip(c: &mut VideoClip, mode: TlDragMode, dt: f32, min_span: f32) {
        let src_sec = c.dur / c.speed.max(0.01);
        let df = dt / src_sec.max(0.001);
        let min_frac = min_span / src_sec.max(0.001);
        match mode {
            TlDragMode::TrimIn => {
                let max_a = (c.b - min_frac).max(0.0);
                c.a = (c.a + df).clamp(0.0, max_a);
            }
            TlDragMode::TrimOut => {
                let min_b = (c.a + min_frac).min(1.0);
                c.b = (c.b + df).clamp(min_b, 1.0);
            }
            _ => {}
        }
    }

    fn slip_item(&mut self, r: ItemRef, dt: f32) {
        let uid = match r {
            ItemRef::V1(i) => self.video.seq.get(i).map(|c| c.uid),
            ItemRef::Video { track, index } => self.video.extra_tracks.get(track.wrapping_sub(1)).and_then(|t| t.get(index)).map(|c| c.uid),
            _ => None,
        }.unwrap_or(0);
        if self.is_uid_locked(uid) { return; }
        let edit = match r {
            ItemRef::V1(i) => self.video.seq.get(i).map(|c| (c.a, c.b, c.dur / c.speed.max(0.01))),
            ItemRef::Video { track, index } => self.video.extra_tracks.get(track.wrapping_sub(1))
                .and_then(|t| t.get(index)).map(|c| (c.a, c.b, c.dur / c.speed.max(0.01))),
            _ => None,
        };
        if let Some((a, b, src)) = edit {
            let df = dt / src.max(0.001);
            let span = b - a;
            let na = (a + df).clamp(0.0, 1.0 - span);
            let nb = na + span;
            match r {
                ItemRef::V1(i) => { if let Some(c) = self.video.seq.get_mut(i) { c.a = na; c.b = nb; } }
                ItemRef::Video { track, index } => {
                    if let Some(c) = self.video.extra_tracks.get_mut(track.wrapping_sub(1)).and_then(|t| t.get_mut(index)) { c.a = na; c.b = nb; }
                }
                _ => {}
            }
            self.video.status = format!("Slipped source in/out by {:.2}s", dt);
            self.video.dirty = true;
        }
    }

    fn split_at_playhead(&mut self) {
        let t = self.video.playhead;
        let uids = self.timeline_ui.selection.clone();
        if uids.is_empty() {
            self.video.status = "Select a clip to split, or use Razor + click.".into();
            return;
        }
        for uid in uids { if !self.is_uid_locked(uid) { self.split_item_at(uid, t); } }
    }

    fn split_item_at(&mut self, uid: u64, t: f32) {
        if self.is_uid_locked(uid) { return; }
        let (_, _, refs) = self.timeline_snapshot();
        let r = match refs.get(&uid) { Some(r) => *r, None => return };
        let (start, dur) = match self.item_bounds(r) { Some(v) => v, None => return };
        if t <= start + timeline::MIN_CLIP || t >= start + dur - timeline::MIN_CLIP { return; }
        let new_uid = fresh_clip_uid();
        match r {
            ItemRef::V1(i) => {
                let Some(c) = self.video.seq.get(i).cloned() else { return };
                if c.compound.is_some() { return; }
                let local = t - start;
                let src = c.a * c.dur + local * c.speed;
                let f = (src / c.dur.max(0.01)).clamp(c.a + 0.01, c.b - 0.01);
                let mut a = c.clone();
                a.b = f;
                let mut b = c;
                b.uid = new_uid;
                b.a = f;
                self.video.seq.splice(i..=i, vec![a, b]);
                self.timeline_ui.set_selection(vec![new_uid]);
                self.video.sel = vec![i + 1];
                self.video.video_stream = None;
            }
            ItemRef::Video { track, index } => {
                let Some(c) = self.video.extra_tracks.get(track.wrapping_sub(1)).and_then(|x| x.get(index)).cloned() else { return };
                let local = t - c.start;
                let src = c.a * c.dur + local * c.speed;
                let f = (src / c.dur.max(0.01)).clamp(c.a + 0.01, c.b - 0.01);
                let mut a = c.clone(); a.b = f;
                let mut b = c; b.uid = new_uid; b.a = f;
                if let Some(tr) = self.video.extra_tracks.get_mut(track - 1) {
                    tr.remove(index);
                    tr.splice(index..index, vec![a, b]);
                }
                self.timeline_ui.set_selection(vec![new_uid]);
                self.video.extra_sel = Some(new_uid);
            }
            ItemRef::Overlay(i) => {
                if let Some(o) = self.video.overlays.get(i).cloned() {
                    let mut a = o.clone(); a.dur = (t - o.start).max(timeline::MIN_CLIP);
                    let mut b = o; b.uid = new_uid; b.start = t;
                    b.dur = (start + dur - t).max(timeline::MIN_CLIP);
                    self.video.overlays.remove(i);
                    self.video.overlays.splice(i..i, vec![a, b]);
                    self.timeline_ui.set_selection(vec![new_uid]);
                }
            }
            ItemRef::Music(i) => {
                if let Some(m) = self.video.music.get(i).cloned() {
                    let mut a = m.clone(); a.dur = t - m.start;
                    let mut b = m; b.uid = new_uid; b.start = t; b.dur = (start + dur - t).max(timeline::MIN_CLIP);
                    self.video.music.remove(i);
                    self.video.music.splice(i..i, vec![a, b]);
                    self.timeline_ui.set_selection(vec![new_uid]);
                }
            }
            ItemRef::Text(i) => {
                if let Some(x) = self.video.texts.get(i).cloned() {
                    let mut a = x.clone(); a.b = t;
                    let mut b = x; b.uid = new_uid; b.a = t;
                    self.video.texts.remove(i);
                    self.video.texts.splice(i..i, vec![a, b]);
                    self.timeline_ui.set_selection(vec![new_uid]);
                }
            }
        }
        self.video.dirty = true;
        self.project_dirty = true;
        self.video.status = format!("Split at {}", timeline::format_tc(t, self.video.fps));
    }

    /// Delete selected items. `ripple` pulls later items on the same track left
    /// by the removed duration (Premiere's Ripple Delete).
    fn delete_selected(&mut self, ripple: bool) {
        let (_, _, refs) = self.timeline_snapshot();
        let rs = self.selected_refs(&refs);
        if rs.is_empty() { return; }
        let mut v1 = Vec::new();
        let mut extra: Vec<(usize, usize, f32, f32)> = Vec::new();
        let mut music: Vec<(usize, usize, f32, f32)> = Vec::new();
        let mut ov = Vec::new();
        let mut txt = Vec::new();
        for r in rs {
            match r {
                ItemRef::V1(i) => v1.push(i),
                ItemRef::Video { track, index } => {
                    if let Some((s, d)) = self.item_bounds(r) { extra.push((track, index, s, d)); }
                }
                ItemRef::Music(i) => {
                    let tr = self.video.music.get(i).map(|m| m.track).unwrap_or(0);
                    if let Some((s, d)) = self.item_bounds(r) { music.push((i, tr, s, d)); }
                }
                ItemRef::Overlay(i) => ov.push(i),
                ItemRef::Text(i) => txt.push(i),
            }
        }
        v1.sort_unstable(); v1.dedup();
        extra.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        music.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        ov.sort_unstable(); ov.dedup();
        txt.sort_unstable(); txt.dedup();
        for &i in v1.iter().rev() { if i < self.video.seq.len() { self.video.seq.remove(i); } }
        for (track, index, s, d) in extra {
            if let Some(t) = self.video.extra_tracks.get_mut(track.wrapping_sub(1)) {
                if index < t.len() { t.remove(index); }
                if ripple {
                    for c in t.iter_mut() { if c.start > s + 0.001 { c.start = (c.start - d).max(0.0); } }
                }
            }
        }
        for (i, tr, s, d) in music {
            if i < self.video.music.len() { self.video.music.remove(i); }
            if ripple {
                for m in self.video.music.iter_mut() {
                    if m.track == tr && m.start > s + 0.001 { m.start = (m.start - d).max(0.0); }
                }
            }
        }
        for i in ov.into_iter().rev() { if i < self.video.overlays.len() { self.video.overlays.remove(i); } }
        for i in txt.into_iter().rev() { if i < self.video.texts.len() { self.video.texts.remove(i); } }
        self.timeline_ui.clear_selection();
        self.video.sel.clear();
        self.video.extra_sel = None;
        self.video.video_stream = None;
        self.video.dirty = true;
        self.project_dirty = true;
        self.video.status = if ripple { "Ripple deleted selected clip(s)".into() } else { "Lifted selected clip(s)".into() };
    }

    fn duplicate_selected(&mut self) {
        let (_, _, refs) = self.timeline_snapshot();
        let rs = self.selected_refs(&refs);
        let mut new_sel = Vec::new();
        let mut rs = rs;
        rs.sort_unstable_by(|a, b| {
            let rank = |r: &ItemRef| match r {
                ItemRef::V1(i) => (*i as isize, 0),
                ItemRef::Video { index, .. } => (*index as isize, 1),
                ItemRef::Overlay(i) => (*i as isize, 2),
                ItemRef::Music(i) => (*i as isize, 3),
                ItemRef::Text(i) => (*i as isize, 4),
            };
            rank(b).cmp(&rank(a))
        });
        for r in rs {
            match r {
                ItemRef::V1(i) => {
                    if let Some(mut c) = self.video.seq.get(i).cloned() {
                        c.uid = fresh_clip_uid();
                        new_sel.push(c.uid);
                        self.video.seq.insert(i + 1, c);
                    }
                }
                ItemRef::Video { track, index } => {
                    if let Some(mut c) = self.video.extra_tracks.get(track.wrapping_sub(1)).and_then(|t| t.get(index)).cloned() {
                        c.uid = fresh_clip_uid();
                        c.start += c.len();
                        new_sel.push(c.uid);
                        if let Some(t) = self.video.extra_tracks.get_mut(track - 1) { t.push(c); }
                    }
                }
                ItemRef::Overlay(i) => {
                    if let Some(mut o) = self.video.overlays.get(i).cloned() {
                        o.uid = fresh_clip_uid(); o.start += o.dur;
                        new_sel.push(o.uid);
                        self.video.overlays.push(o);
                    }
                }
                ItemRef::Music(i) => {
                    if let Some(mut m) = self.video.music.get(i).cloned() {
                        m.uid = fresh_clip_uid(); m.start += m.dur;
                        new_sel.push(m.uid);
                        self.video.music.push(m);
                    }
                }
                ItemRef::Text(i) => {
                    if let Some(mut t) = self.video.texts.get(i).cloned() {
                        t.uid = fresh_clip_uid();
                        let d = if t.b > 1e8 { 4.0 } else { t.b - t.a };
                        t.a += d; t.b = t.a + d;
                        new_sel.push(t.uid);
                        self.video.texts.push(t);
                    }
                }
            }
        }
        self.video.sel = self.video.seq.iter().enumerate()
            .filter(|(_, c)| new_sel.contains(&c.uid))
            .map(|(i, _)| i).collect();
        self.timeline_ui.set_selection(new_sel);
        self.video.dirty = true;
        self.video.status = "Duplicated selected clip(s)".into();
    }

    fn nudge_selected(&mut self, dt: f32) {
        let (_, items, refs) = self.timeline_snapshot();
        let rs = self.selected_refs(&refs);
        for r in rs {
            if let Some((s, _)) = self.item_bounds(r) {
                let lane = items.iter().find(|it| refs.get(&it.uid) == Some(&r)).map(|it| it.track).unwrap_or(0);
                let kind = items.iter().find(|it| refs.get(&it.uid) == Some(&r)).map(|it| it.kind).unwrap_or(TrackKind::Video);
                let first_audio = 2 + self.video.video_track_count();
                self.move_item_to(r, (s + dt).max(0.0), lane, kind, first_audio, false);
            }
        }
        self.video.dirty = true;
    }

    fn group_selected(&mut self) {
        let (_, _, refs) = self.timeline_snapshot();
        let mut idxs: Vec<usize> = self.timeline_ui.selection.iter()
            .filter(|u| !self.is_uid_locked(**u))
            .filter_map(|u| match refs.get(u) { Some(ItemRef::V1(i)) => Some(*i), _ => None })
            .collect();
        idxs.sort_unstable(); idxs.dedup();
        if idxs.len() < 2 { return; }
        let mut dur = 0.0_f32;
        let mut nested = Vec::new();
        for &i in &idxs {
            if let Some(c) = self.video.seq.get(i) { dur += c.len(); nested.push(c.clone()); }
        }
        let at = idxs[0];
        for &i in idxs.iter().rev() { if i < self.video.seq.len() { self.video.seq.remove(i); } }
        self.video.cmp_n += 1;
        let mut c = VideoClip::new(format!("Compound {} ({} clips)", self.video.cmp_n, nested.len()), dur, None);
        c.compound = Some(nested);
        self.video.seq.insert(at, c.clone());
        self.timeline_ui.set_selection(vec![c.uid]);
        self.video.sel = vec![at];
        self.video.dirty = true;
        self.video.status = "Grouped selected clips (compound)".into();
    }

    fn ungroup_selected(&mut self) {
        let (_, _, refs) = self.timeline_snapshot();
        let target = self.timeline_ui.selection.iter()
            .filter(|u| !self.is_uid_locked(**u))
            .find_map(|u| match refs.get(u) { Some(ItemRef::V1(i)) if self.video.seq.get(*i).and_then(|c| c.compound.as_ref()).is_some() => Some(*i), _ => None });
        let Some(i) = target else { return };
        let clips = self.video.seq[i].compound.take().unwrap_or_default();
        if clips.is_empty() { return; }
        let ids: Vec<u64> = clips.iter().map(|c| c.uid).collect();
        self.video.seq.splice(i..=i, clips);
        self.timeline_ui.set_selection(ids.clone());
        self.video.sel = (i..i + ids.len()).collect();
        self.video.dirty = true;
        self.video.status = format!("Ungrouped {} clip(s)", ids.len());
    }

    /// Type-aware addition at a timeline lane/time (used by media-bin drops).
    fn add_bin_item_to_timeline_at(&mut self, name: &str, path: &std::path::Path, dur: f32, lane: usize, time: f32) {
        let (tracks, _, _) = self.timeline_snapshot();
        let kind = tracks.get(lane).map(|t| t.header.kind).unwrap_or(TrackKind::Video);
        let nv = self.video.video_track_count();
        let first_audio = nv + 2;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let is_audio = matches!(ext.as_str(), "wav" | "mp3" | "flac" | "ogg" | "oga" | "m4a" | "aac");
        if is_audio {
            let track = if kind == TrackKind::Audio && lane >= first_audio { lane - first_audio } else { 0 };
            self.video.music.push(MusicClip {
                uid: fresh_clip_uid(), nm: name.to_string(), dur: dur.max(0.5),
                start: time.max(0.0), vol: 80.0, track,
                path: Some(path.to_path_buf()),
            });
            if let Ok((l, r, sr)) = media_engine::decode_audio_stereo(path) {
                let n = l.len().min(r.len());
                let mut inter = Vec::with_capacity(n * 2);
                for i in 0..n { inter.push(l[i]); inter.push(r[i]); }
                self.audio.samples = inter;
                self.audio.sample_rate = sr;
                self.audio.channels = 2;
                self.audio.duration = n as f32 / sr.max(1) as f32;
                self.audio.filename = name.to_string();
                self.audio.loaded = true;
                self.audio.is_demo = false;
                self.plugins.set_sample_rate(sr);
                self.reload_playback_buffer();
            }
            self.video.status = format!("Added {} to A{} at {:.1}s", name, track + 2, time);
            return;
        }

        if kind == TrackKind::Overlay {
            self.video.overlays.push(OverlayClip {
                uid: fresh_clip_uid(),
                nm: name.to_string(), dur: dur.max(0.5), start: time.max(0.0),
                x: 75.0, y: 75.0, s: 32.0, o: 100.0, path: Some(path.to_path_buf()),
            });
            self.video.status = format!("Added {} to Overlay at {:.1}s", name, time);
            return;
        }

        let mut clip = VideoClip::new(name.to_string(), dur.max(0.5), Some(path.to_path_buf()));
        if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "webp" | "gif") { clip.dur = 5.0; }
        let vt = if kind == TrackKind::Video && lane > 0 { lane } else { 0 };
        if vt == 0 {
            clip.start = self.video.seq.iter().map(|c| c.len()).sum();
            let uid = clip.uid;
            let mut pos = self.video.seq.len();
            let mut acc = 0.0_f32;
            for (i, c) in self.video.seq.iter().enumerate() {
                if time < acc + c.len() * 0.5 { pos = i; break; }
                acc += c.len();
            }
            self.video.seq.insert(pos, clip);
            self.timeline_ui.set_selection(vec![uid]);
        } else {
            clip.track = vt;
            clip.start = time.max(0.0);
            let uid = clip.uid;
            self.ensure_video_lane(vt);
            self.video.extra_tracks[vt - 1].push(clip);
            self.timeline_ui.set_selection(vec![uid]);
        }
        self.video.media_path = Some(path.to_path_buf());
        self.video.video_stream = None;
        self.video.status = format!("Added {} to V{} at {:.1}s", name, vt + 1, time);
    }
}

// ===========================================================================
// Audio mode
// ===========================================================================
impl AudioState {
    /// Snap a time to the musical grid (1/8 note by default at 4/4).
    fn snap_time(&self, t: f32) -> f32 {
        if !self.snap_enabled { return t; }
        let beat = 60.0 / self.tempo_bpm.max(1) as f32;
        let step = (beat * 4.0 / self.time_sig_den.max(1) as f32).max(0.001);
        (t / step).round() * step
    }

    fn tick(&mut self, dt: f32, engine: &realtime_av::RealtimeAudioEngine) {
        if self.is_playing {
            let engine_time = engine.current_time_seconds() as f32;
            self.playhead = if engine.is_playing() || engine_time > self.playhead {
                engine_time.min(self.duration)
            } else {
                (self.playhead + dt).min(self.duration)
            };
            if self.playhead >= self.duration {
                self.playhead = self.duration;
                self.is_playing = false;
            }
        }
        // `processing`/`done` are set by real render calls, not fake progress.
    }


    fn start_audio_fx_load(&mut self, path: std::path::PathBuf) {
        if self.fx_loading { return; }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("audio").to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        self.fx_rx = Some(rx);
        self.fx_loading = true;
        self.fx_paused = false;
        self.fx_cancel_requested = false;
        self.fx_loading_status = format!("Loading {}…", name);
        std::thread::spawn(move || {
            let result = match media_engine::decode_audio_stereo(&path) {
                Ok((l, r, sr)) => {
                    let n = l.len().min(r.len());
                    let mut samples = Vec::with_capacity(n * 2);
                    for i in 0..n { samples.push(l[i]); samples.push(r[i]); }
                    let duration = (n as f32 / sr.max(1) as f32).max(0.1);
                    Ok(AudioFxLoad { name, path, samples, sample_rate: sr, duration })
                }
                Err(e) => Err(e),
            };
            let _ = tx.send(result);
        });
    }

    fn cancel_audio_fx_loading(&mut self) {
        self.fx_cancel_requested = true;
        self.fx_loading = false;
        self.fx_rx = None;
        self.fx_loading_status = "Audio FX loading cancelled.".into();
    }

    fn pump_audio_fx(&mut self, music: &mut Vec<MusicClip>, engine: &realtime_av::RealtimeAudioEngine, plugins: &mut PluginChain) {
        if !self.fx_loading || self.fx_paused { return; }
        let Some(rx) = &self.fx_rx else { return };
        match rx.try_recv() {
            Ok(result) => {
                self.fx_loading = false;
                self.fx_rx = None;
                if self.fx_cancel_requested {
                    self.fx_cancel_requested = false;
                    self.fx_loading_status = "Audio FX loading cancelled.".into();
                    return;
                }
                match result {
                    Ok(load) => {
                        let n = load.samples.len() / 2;
                        let mut left = Vec::with_capacity(n);
                        let mut right = Vec::with_capacity(n);
                        for i in 0..n { left.push(load.samples[i * 2]); right.push(load.samples[i * 2 + 1]); }
                        self.samples = load.samples;
                        self.sample_rate = load.sample_rate;
                        self.channels = 2;
                        self.duration = load.duration;
                        self.sel_start = 0.0;
                        self.sel_end = load.duration;
                        self.filename = load.name.clone();
                        self.loaded = true;
                        self.is_demo = false;
                        self.playhead = 0.0;
                        plugins.set_sample_rate(load.sample_rate);
                        engine.load_stereo_buffer(left, right, load.sample_rate);
                        engine.seek_seconds(0.0);
                        if !music.iter().any(|m| m.path.as_deref() == Some(load.path.as_path())) {
                            music.push(MusicClip {
                                uid: fresh_clip_uid(), nm: load.name.clone(), dur: load.duration,
                                start: 0.0, vol: 80.0, track: 0, path: Some(load.path.clone()),
                            });
                        }
                        self.fx_loading_status = format!("Loaded {} ({:.2}s, {} Hz)", load.name, load.duration, load.sample_rate);
                        self.status = self.fx_loading_status.clone();
                    }
                    Err(e) => { self.fx_loading_status = format!("Audio FX load failed: {e}"); }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.fx_loading = false; self.fx_rx = None; self.fx_loading_status = "Audio FX loading stopped.".into();
            }
        }
    }

    fn render_rack(&mut self, ui: &mut Ui, ctx: &egui::Context, plugins: &mut PluginChain, a1_mute: &mut bool, music: &mut Vec<MusicClip>, engine: &realtime_av::RealtimeAudioEngine, seek_out: &mut Option<f32>) {
        self.pump_audio_fx(music, engine, plugins);
        if self.fx_loading { ctx.request_repaint(); }
        // Same dark Grain palette as every other dock. The Audio FX rack must
        // never restyle the workspace into a separate light "audio editor".
        let aud_bar_bg = PANEL2;
        let aud_panel_bg = PANEL;
        let aud_btn_bg = TBBG;
        let aud_btn_border = LINE;
        let aud_dark_txt = INK;
        let aud_sub_txt = SUB;
        let aud_canvas_bg = TLBG;
        let aud_clip_bg = CLIP_A;
        let aud_wave_pk = Color32::from_rgba_unmultiplied(255, 255, 255, 125);
        let aud_wave_rms = ACCENT2;
        let aud_lcd_bg = TLBG;

        ui.vertical(|ui| {
            // 1. TOP AUDACITY TRANSPORT & TOOL DOCK
            egui::Frame::none()
                .fill(aud_bar_bg)
                .stroke(Stroke::new(1.0_f32, aud_btn_border))
                .inner_margin(egui::Margin::symmetric(6.0, 5.0))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                        // Transport buttons: Pause, Play, Stop, Skip Start, Skip End, Record, Loop
                        let pause_btn = egui::Button::new(RichText::new("❚❚").color(aud_dark_txt).size(12.0))
                            .fill(if !self.is_playing && self.playhead > 0.0 { Color32::from_rgb(0xd0, 0xd8, 0xe8) } else { aud_btn_bg })
                            .stroke(Stroke::new(1.0_f32, aud_btn_border))
                            .min_size(vec2(26.0, 24.0));
                        if ui.add(pause_btn).on_hover_text("Pause").clicked() {
                            self.is_playing = false;
                            engine.pause();
                        }

                        let play_btn = egui::Button::new(RichText::new("▶").color(Color32::from_rgb(0x1f, 0x7a, 0x3a)).size(13.0))
                            .fill(if self.is_playing { Color32::from_rgb(0xc8, 0xeb, 0xd2) } else { aud_btn_bg })
                            .stroke(Stroke::new(1.0_f32, aud_btn_border))
                            .min_size(vec2(26.0, 24.0));
                        if ui.add(play_btn).on_hover_text("Play (Space)").clicked() {
                            if self.playhead >= self.duration { self.playhead = 0.0; }
                            self.is_playing = true;
                            engine.play_from_seconds(self.playhead as f64);
                        }

                        let stop_btn = egui::Button::new(RichText::new("⏹").color(aud_sub_txt).size(12.0))
                            .fill(aud_btn_bg)
                            .stroke(Stroke::new(1.0_f32, aud_btn_border))
                            .min_size(vec2(26.0, 24.0));
                        if ui.add(stop_btn).on_hover_text("Stop").clicked() {
                            self.is_playing = false;
                            self.playhead = 0.0;
                            engine.pause();
                            engine.seek_seconds(0.0);
                        }

                        if ui.add(egui::Button::new(RichText::new("⏮").color(aud_dark_txt).size(11.0)).fill(aud_btn_bg).stroke(Stroke::new(1.0_f32, aud_btn_border)).min_size(vec2(24.0, 24.0))).on_hover_text("Skip to Start").clicked() {
                            self.playhead = 0.0;
                            engine.seek_seconds(0.0);
                        }
                        if ui.add(egui::Button::new(RichText::new("⏭").color(aud_dark_txt).size(11.0)).fill(aud_btn_bg).stroke(Stroke::new(1.0_f32, aud_btn_border)).min_size(vec2(24.0, 24.0))).on_hover_text("Skip to End").clicked() {
                            self.playhead = self.duration;
                            engine.seek_seconds(self.duration as f64);
                        }

                        let rec_btn = egui::Button::new(RichText::new("⏺").color(Color32::from_rgb(0xd6, 0x28, 0x28)).size(13.0))
                            .fill(aud_btn_bg)
                            .stroke(Stroke::new(1.0_f32, aud_btn_border))
                            .min_size(vec2(26.0, 24.0));
                        if ui.add(rec_btn).on_hover_text("Record").clicked() {
                            self.status = "Armed audio input — ready to record.".into();
                        }

                        ui.separator();

                        // Audacity Tool Palette (Selection I-beam, Envelope, Draw, Zoom, Multi)
                        for (idx, (glyph, tip)) in [
                            ("I", "Selection Tool (F1)"),
                            ("⬦", "Envelope Tool (F2)"),
                            ("✏", "Draw Tool (F3)"),
                            ("🔍", "Zoom Tool (F4)"),
                            ("✳", "Multi-Tool (F6)"),
                        ].iter().enumerate() {
                            let active = self.active_tool == idx as u8;
                            let b = egui::Button::new(RichText::new(*glyph).color(aud_dark_txt).size(11.0))
                                .fill(if active { Color32::from_rgb(0xc6, 0xd4, 0xf0) } else { aud_btn_bg })
                                .stroke(Stroke::new(1.0_f32, if active { ACCENT2 } else { aud_btn_border }))
                                .min_size(vec2(22.0, 24.0));
                            if ui.add(b).on_hover_text(*tip).clicked() {
                                self.active_tool = idx as u8;
                            }
                        }

                        ui.separator();

                        // Audio Setup / Open Audio / Export WAV buttons
                        if ui.add(egui::Button::new(RichText::new("🔊 Audio Setup").color(aud_dark_txt).size(10.5)).fill(aud_btn_bg).stroke(Stroke::new(1.0_f32, aud_btn_border)).min_size(vec2(82.0, 24.0))).clicked() {
                            if let Some(path) = media_engine::pick_file_dialog("Load Audio File into Audio FX") {
                                self.start_audio_fx_load(path);
                            }
                        }
                        if self.fx_loading {
                            ui.spinner();
                            ui.label(RichText::new(if self.fx_paused { "Paused" } else { &self.fx_loading_status }).color(aud_sub_txt).size(9.5));
                            if ui.small_button(if self.fx_paused { "Resume" } else { "Pause" }).clicked() { self.fx_paused = !self.fx_paused; }
                            if ui.small_button("Cancel").clicked() { self.cancel_audio_fx_loading(); }
                        } else if !self.fx_loading_status.is_empty() {
                            ui.label(RichText::new(&self.fx_loading_status).color(aud_sub_txt).size(9.5));
                        }

                        if ui.add(egui::Button::new(RichText::new("⬆ Share / Export").color(aud_dark_txt).size(10.5)).fill(aud_btn_bg).stroke(Stroke::new(1.0_f32, aud_btn_border)).min_size(vec2(88.0, 24.0))).clicked() {
                            let out_path = media_engine::default_export_path("grain_audio_export.wav");
                            let out_str = out_path.to_string_lossy().to_string();
                            match self.render_real(&out_str, plugins) {
                                Ok(notes) => {
                                    self.done = true;
                                    self.status = format!(
                                        "Exported WAV (non-destructive) → {}{}",
                                        out_str,
                                        if notes.is_empty() { String::new() } else { format!(" · {}", notes.join(" · ")) }
                                    );
                                }
                                Err(e) => { self.done = false; self.status = format!("Export error: {e}"); }
                            }
                        }
                    });

                    // Audacity Playback & Recording Level Meter strip (-54 dB .. 0 dB)
                    ui.add_space(3.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔊 L/R").color(aud_dark_txt).size(9.5));
                        let (mrect, _) = ui.allocate_exact_size(vec2(ui.available_width().max(40.0), 14.0), Sense::hover());
                        let mp = ui.painter();
                        mp.rect_filled(mrect, 2.0, TLBG);
                        mp.rect_stroke(mrect, 2.0, Stroke::new(1.0_f32, aud_btn_border));
                        // Compute current peak level around playhead
                        let peak = if !self.samples.is_empty() && self.is_playing {
                            let idx = ((self.playhead / self.duration.max(0.01)) * self.samples.len() as f32) as usize;
                            let s0 = idx.saturating_sub(256).min(self.samples.len());
                            let s1 = (idx + 256).min(self.samples.len());
                            self.samples[s0..s1].iter().fold(0.0_f32, |acc, &v| acc.max(v.abs())).min(1.0)
                        } else {
                            0.0
                        };
                        if peak > 0.001 {
                            let fill_w = mrect.width() * peak;
                            let mcol = if peak > 0.9 { Color32::from_rgb(0xd9, 0x38, 0x38) } else if peak > 0.7 { Color32::from_rgb(0xe6, 0xb8, 0x00) } else { Color32::from_rgb(0x3b, 0xb5, 0x4a) };
                            mp.rect_filled(Rect::from_min_size(mrect.min + vec2(1.0, 1.0), vec2((fill_w - 2.0).max(0.0), mrect.height() - 2.0)), 1.0, mcol);
                        }
                        for (db_lbl, frac) in [("-54", 0.05), ("-42", 0.22), ("-30", 0.40), ("-18", 0.62), ("-12", 0.75), ("-6", 0.88), ("0", 0.98)] {
                            let tx = mrect.left() + mrect.width() * frac;
                            mp.text(pos2(tx, mrect.center().y), egui::Align2::CENTER_CENTER, db_lbl, FontId::monospace(8.5), aud_sub_txt);
                        }
                    });
                });

            // 2. AUDACITY TIMELINE RULER (1.0, 1.2, 1.3, 1.4, 2.0 ...)
            let left_ctrl_w = 136.0;
            let ruler_h = 18.0;
            let track_h = 128.0;
            let total_track_w = ui.available_width().max(left_ctrl_w + 40.0);
            let wave_w = (total_track_w - left_ctrl_w).max(40.0);

            let (ruler_area, ruler_resp) = ui.allocate_exact_size(vec2(total_track_w, ruler_h), Sense::click_and_drag());
            {
                let rp = ui.painter();
                rp.rect_filled(ruler_area, 0.0, aud_panel_bg);
                rp.rect_stroke(ruler_area, 0.0, Stroke::new(1.0_f32, aud_btn_border));
                rp.text(pos2(ruler_area.left() + 8.0, ruler_area.center().y), egui::Align2::LEFT_CENTER, "⚙", FontId::proportional(11.0), aud_sub_txt);
                let wave_left = ruler_area.left() + left_ctrl_w;
                let ticks = 8;
                for i in 0..=ticks {
                    let frac = i as f32 / ticks as f32;
                    let tx = wave_left + frac * wave_w;
                    let t_val = frac * self.duration.max(1.0);
                    rp.line_segment([pos2(tx, ruler_area.bottom() - 5.0), pos2(tx, ruler_area.bottom())], Stroke::new(1.0_f32, aud_dark_txt));
                    rp.text(pos2(tx + 2.0, ruler_area.center().y), egui::Align2::LEFT_CENTER, format!("{:.1}", t_val), FontId::monospace(8.5), aud_dark_txt);
                }
                if ruler_resp.clicked() || ruler_resp.dragged() || ruler_resp.drag_released() {
                    if let Some(pos) = ruler_resp.interact_pointer_pos() {
                        let t = ((pos.x - wave_left) / wave_w).clamp(0.0, 1.0) * self.duration;
                        self.playhead = t;
                        *seek_out = Some(t);
                    }
                }
            }

            // 3. AUDACITY TRACK CONTROL PANEL (LEFT) + PERIWINKLE BLUE WAVEFORM CLIP (RIGHT)
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let (tbox, _) = ui.allocate_exact_size(vec2(left_ctrl_w, track_h), Sense::hover());
                let tp = ui.painter();
                tp.rect_filled(tbox, 0.0, aud_panel_bg);
                tp.rect_stroke(tbox, 0.0, Stroke::new(1.0_f32, aud_btn_border));
                // Track Title Header: "× Audio 1"
                tp.text(pos2(tbox.left() + 6.0, tbox.top() + 11.0), egui::Align2::LEFT_CENTER, format!("×  {}", self.filename), FontId::proportional(10.5), aud_dark_txt);
                // Vertical amplitude scale on right edge of track header (1.0, 0.5, 0.0, -0.5, -1.0)
                let scale_x = tbox.right() - 4.0;
                for (lbl, fy) in [("1.0", 0.08), ("0.5", 0.28), ("0.0", 0.50), ("-0.5", 0.72), ("-1.0", 0.92)] {
                    tp.text(pos2(scale_x, tbox.top() + track_h * fy), egui::Align2::RIGHT_CENTER, lbl, FontId::monospace(7.5), aud_dark_txt);
                }

                // Interactive Mute / Solo / Effects / Gain / Pan widgets inside left track panel
                let inner_ctrl = Rect::from_min_size(pos2(tbox.left() + 6.0, tbox.top() + 22.0), vec2(left_ctrl_w - 30.0, track_h - 26.0));
                ui.allocate_ui_at_rect(inner_ctrl, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
                        ui.horizontal(|ui| {
                            let m_btn = egui::Button::new(RichText::new("Mute").color(if self.mute { Color32::WHITE } else { aud_dark_txt }).size(9.5))
                                .fill(if self.mute { ACCENT2 } else { aud_btn_bg })
                                .stroke(Stroke::new(1.0_f32, aud_btn_border))
                                .min_size(vec2(44.0, 18.0));
                            if ui.add(m_btn).clicked() {
                                self.mute = !self.mute;
                                *a1_mute = self.mute;
                            }

                            let s_btn = egui::Button::new(RichText::new("Solo").color(if self.solo { Color32::BLACK } else { aud_dark_txt }).size(9.5))
                                .fill(if self.solo { AMBER } else { aud_btn_bg })
                                .stroke(Stroke::new(1.0_f32, aud_btn_border))
                                .min_size(vec2(44.0, 18.0));
                            if ui.add(s_btn).clicked() { self.solo = !self.solo; }
                        });

                        let fx_btn = egui::Button::new(RichText::new("Effects").color(aud_dark_txt).size(9.5))
                            .fill(if self.audio_fx_popup_open { ACCENT2 } else { aud_btn_bg })
                            .stroke(Stroke::new(1.0_f32, aud_btn_border))
                            .min_size(vec2(92.0, 18.0));
                        if ui.add(fx_btn).on_hover_text("Open Audio FX in a new window").clicked() {
                            self.audio_fx_popup_open = !self.audio_fx_popup_open;
                            self.status = format!("Active DSP: {}", self.fx_tab.label());
                        }

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("−").color(aud_sub_txt).size(9.0));
                            ui.add(Slider::new(&mut self.track_gain_db, -24.0..=24.0).show_value(false));
                            ui.label(RichText::new("+").color(aud_sub_txt).size(9.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("L").color(aud_sub_txt).size(8.5));
                            ui.add(Slider::new(&mut self.track_pan, -1.0..=1.0).show_value(false));
                            ui.label(RichText::new("R").color(aud_sub_txt).size(8.5));
                        });
                    });
                });

                // Right Waveform Track Area
                let (wrect, wresp) = ui.allocate_exact_size(vec2(wave_w, track_h), Sense::click_and_drag());
                let wp = ui.painter();
                wp.rect_filled(wrect, 0.0, aud_canvas_bg);

                // Audacity Clip Box with light top title bar ("Audio 1.1") + periwinkle blue body
                let title_h = 16.0;
                let clip_rect = wrect.shrink2(vec2(1.0, 2.0));
                let title_rect = Rect::from_min_size(clip_rect.min, vec2(clip_rect.width(), title_h));
                let body_rect = Rect::from_min_max(pos2(clip_rect.left(), title_rect.bottom()), clip_rect.max);

                wp.rect_filled(title_rect, 0.0, PANEL2);
                wp.text(pos2(title_rect.left() + 5.0, title_rect.center().y), egui::Align2::LEFT_CENTER, format!("{} · 1.1", self.filename), FontId::proportional(9.5), aud_dark_txt);
                wp.rect_filled(body_rect, 0.0, if self.mute { Color32::from_gray(52) } else { aud_clip_bg });

                // Selection highlight overlay
                let dur = self.duration.max(0.01);
                let s0_frac = (self.sel_start / dur).clamp(0.0, 1.0);
                let s1_frac = (self.sel_end / dur).clamp(0.0, 1.0);
                if (s1_frac - s0_frac).abs() > 0.002 {
                    let sx0 = body_rect.left() + s0_frac.min(s1_frac) * body_rect.width();
                    let sx1 = body_rect.left() + s0_frac.max(s1_frac) * body_rect.width();
                    wp.rect_filled(
                        Rect::from_min_max(pos2(sx0, body_rect.top()), pos2(sx1, body_rect.bottom())),
                        0.0,
                        Color32::from_rgba_unmultiplied(255, 255, 255, 34),
                    );
                }

                // 0.0 center line
                let mid = body_rect.center().y;
                wp.line_segment([pos2(body_rect.left(), mid), pos2(body_rect.right(), mid)], Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(20, 24, 50, 120)));

                // Render Audacity dual-tone peak + RMS waveform
                let n = self.samples.len();
                let gain_lin = if self.mute { 0.0 } else { 10.0_f32.powf(self.track_gain_db / 20.0) };
                if n > 0 {
                    let cols = body_rect.width().max(1.0) as usize;
                    let step = (n / cols).max(1);
                    let h_half = body_rect.height() * 0.46;
                    for x in 0..cols {
                        let mut mn = 0.0_f32;
                        let mut mx = 0.0_f32;
                        let mut sq_sum = 0.0_f32;
                        let mut cnt = 0usize;
                        let base = x * step;
                        for j in 0..step {
                            if base + j < n {
                                let s = (self.samples[base + j] * gain_lin).clamp(-1.0, 1.0);
                                if s < mn { mn = s; }
                                if s > mx { mx = s; }
                                sq_sum += s * s;
                                cnt += 1;
                            }
                        }
                        let rms = if cnt > 0 { (sq_sum / cnt as f32).sqrt() } else { 0.0 };
                        let xpix = body_rect.left() + x as f32;
                        let y_top = mid - mx * h_half;
                        let y_bot = mid - mn * h_half;
                        wp.rect_filled(Rect::from_min_max(pos2(xpix, y_top), pos2(xpix + 1.0, y_bot.max(y_top + 1.0))), 0.0, aud_wave_pk);
                        let r_top = mid - rms * h_half;
                        let r_bot = mid + rms * h_half;
                        wp.rect_filled(Rect::from_min_max(pos2(xpix, r_top), pos2(xpix + 1.0, r_bot.max(r_top + 1.0))), 0.0, aud_wave_rms);
                    }
                }

                // Playhead cursor
                let phx = body_rect.left() + (self.playhead / dur).clamp(0.0, 1.0) * body_rect.width();
                wp.line_segment([pos2(phx, wrect.top()), pos2(phx, wrect.bottom())], Stroke::new(1.5_f32, Color32::WHITE));

                if wresp.clicked() {
                    if let Some(pos) = wresp.interact_pointer_pos() {
                        let t = self.snap_time(((pos.x - body_rect.left()) / body_rect.width()).clamp(0.0, 1.0) * dur);
                        self.playhead = t;
                        *seek_out = Some(t);
                        match self.active_tool {
                            0 | 4 => { self.sel_start = t; self.sel_end = t; }
                            1 => self.status = format!("Envelope point at {:.3}s — use the DSP tabs for automation.", t),
                            2 => self.status = format!("Draw tool at {:.3}s — sample painting is available in the Audio FX rack.", t),
                            3 => self.status = format!("Zoom tool anchored at {:.3}s — timeline zoom lives below.", t),
                            _ => {}
                        }
                    }
                }
                if wresp.dragged() {
                    if let Some(pos) = wresp.interact_pointer_pos() {
                        let t = self.snap_time(((pos.x - body_rect.left()) / body_rect.width()).clamp(0.0, 1.0) * dur);
                        self.playhead = t;
                        *seek_out = Some(t);
                        if matches!(self.active_tool, 0 | 4) {
                            self.sel_end = t;
                        }
                    }
                }
            });

            // 4. BOTTOM AUDACITY TRANSPORT & SELECTION COUNTER BAR
            egui::Frame::none()
                .fill(aud_bar_bg)
                .stroke(Stroke::new(1.0_f32, aud_btn_border))
                .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 3.0);
                        ui.label(RichText::new(format!("Tempo: {} BPM", self.tempo_bpm)).color(aud_dark_txt).size(9.5));
                        ui.label(RichText::new(format!("Sig: {}/{}", self.time_sig_num, self.time_sig_den)).color(aud_dark_txt).size(9.5));
                        ui.checkbox(&mut self.snap_enabled, RichText::new("Snap").color(aud_dark_txt).size(9.5));

                        // Digital LCD Audio Position Readout
                        let lcd_txt = format!("{} / {}", fmt_tc(self.playhead), fmt_tc(self.duration));
                        egui::Frame::none()
                            .fill(aud_lcd_bg)
                            .rounding(Rounding::same(3.0))
                            .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new(lcd_txt).color(Color32::from_rgb(0x88, 0xe0, 0xff)).font(FontId::monospace(10.0)));
                            });

                        let s_smp0 = (self.sel_start.min(self.sel_end) * self.sample_rate as f32) as u64;
                        let s_smp1 = (self.sel_start.max(self.sel_end) * self.sample_rate as f32) as u64;
                        ui.label(RichText::new(format!("Sel: {:09} – {:09} smp", s_smp0, s_smp1)).color(aud_sub_txt).font(FontId::monospace(9.0)));
                    });
                    ui.label(
                        RichText::new(if self.is_playing {
                            "Playing. Click and drag waveform to select audio."
                        } else {
                            "Stopped. Click and drag waveform to select audio."
                        })
                        .color(aud_sub_txt)
                        .size(9.0),
                    );
                });

            ui.add_space(6.0);

            // 5. GRAIN + BANANAV DSP FX RACK CONTROLS
            ui.horizontal_wrapped(|ui| {
                for &t in FxTab::ALL {
                    let active = self.fx_tab == t;
                    let btn = egui::Button::new(RichText::new(t.label()).color(if active { CLIP_TXT } else { SUB }).size(11.0))
                        .fill(if active { ACCENT } else { Color32::TRANSPARENT })
                        .stroke(if active { Stroke::NONE } else { Stroke::new(1.0_f32, LINE) })
                        .rounding(Rounding::same(12.0));
                    if ui.add(btn).clicked() { self.fx_tab = t; }
                }
            });
            ui.add_space(4.0);
            ui.label(RichText::new(self.fx_tab.note()).color(SUB).size(10.0));
            ui.label(RichText::new(engine.status_message()).color(if engine.status_message().starts_with("CPAL audio ready") { ACCENT } else { SUB }).size(9.0));
            ui.add_space(4.0);

            let tech_opts = [
                (PitchTech::ElastiquePro, PitchTech::ElastiquePro.label()),
                (PitchTech::ElastiqueA17, PitchTech::ElastiqueA17.label()),
                (PitchTech::Fft, PitchTech::Fft.label()),
                (PitchTech::Wsola, PitchTech::Wsola.label()),
                (PitchTech::Rb, PitchTech::Rb.label()),
                (PitchTech::Bungee, PitchTech::Bungee.label()),
            ];
            match self.fx_tab {
                FxTab::Pitch => {
                    self.p_tech = select_row(ui, "Technique", self.p_tech, &tech_opts);
                    slider_row_f(ui, "Semitones", &mut self.p_amt, -24.0, 24.0, 0.01, |v| format!("{:+.2}", v));
                    self.fft_size = select_row_u32(ui, "FFT size", self.fft_size, &[512, 1024, 2048, 4096, 8192]);
                    slider_row_f(ui, "Formant / timbre", &mut self.fm_amt, -12.0, 12.0, 0.01, |v| format!("{:+.2}", v));
                    ui.checkbox(&mut self.fm_keep, "Preserve original formants");
                }
                FxTab::Time => {
                    self.t_tech = select_row(ui, "Technique", self.t_tech, &tech_opts);
                    slider_row_f(ui, "Duration factor", &mut self.t_amt, 0.4, 2.5, 0.01, |v| format!("{:.2}×", v));
                }
                FxTab::Vibrato => {
                    self.v_mode = select_row(ui, "Mode", self.v_mode, &[(VibMode::A, "a — pitch only"), (VibMode::B, "b — pitch + speed")]);
                    if self.v_mode == VibMode::A { self.v_tech = select_row(ui, "Technique", self.v_tech, &tech_opts); }
                    slider_row_f(ui, "Rate", &mut self.v_rate, 1.0, 12.0, 0.5, |v| format!("{:.1} Hz", v));
                    slider_row_f(ui, "Depth", &mut self.v_depth, 0.1, 4.0, 0.01, |v| format!("{:.2}", v));
                }
                FxTab::SpeedPitch => {
                    slider_row_f(ui, "Rate", &mut self.s_amt, 0.4, 2.5, 0.01, |v| format!("{:.2}×", v));
                    ui.checkbox(&mut self.s_sliding, "Sliding speed / pitch (Audacity)");
                    if self.s_sliding {
                        slider_row_f(ui, "Start rate", &mut self.s_start, 0.02, 4.0, 0.01, |v| format!("{:.2}×", v));
                        slider_row_f(ui, "End rate", &mut self.s_end, 0.02, 4.0, 0.01, |v| format!("{:.2}×", v));
                    }
                }
                FxTab::Bitcrush => {
                    slider_row_i(ui, "Bit depth", &mut self.b_bits, 1, 16, |v| format!("{}", v));
                    slider_row_i(ui, "Sample-rate divisor", &mut self.b_sr, 1, 40, |v| format!("{}", v));
                }
                FxTab::Vocoder => {
                    self.vc_wave = select_row(ui, "Carrier waveform", self.vc_wave, &[(VocWave::Saw, "Sawtooth"), (VocWave::Square, "Square"), (VocWave::Pulse, "Pulse"), (VocWave::Sine, "Sine")]);
                    slider_row_f(ui, "Carrier pitch", &mut self.vc_freq, 40.0, 440.0, 1.0, |v| format!("{:.0} Hz", v));
                    slider_row_i(ui, "Bands", &mut self.vc_bands, 4, 64, |v| format!("{}", v));
                }
                FxTab::Tremolo => {
                    self.tr_wave = select_row(ui, "Waveform", self.tr_wave, &[(TremWave::Sine, "Sine"), (TremWave::Triangle, "Triangle"), (TremWave::Square, "Square"), (TremWave::Saw, "Sawtooth"), (TremWave::Isaw, "Inverse saw"), (TremWave::Noise, "Noise"), (TremWave::Snoise, "Smooth noise")]);
                    slider_row_f(ui, "Rate", &mut self.tr_rate, 0.5, 20.0, 0.5, |v| format!("{:.1} Hz", v));
                    slider_row_f(ui, "Depth", &mut self.tr_depth, 0.0, 1.0, 0.05, |v| format!("{:.2}", v));
                }
                FxTab::Invert => {}
                FxTab::MultiPitch => {
                    self.mp_tech = select_row(ui, "Technique", self.mp_tech, &tech_opts);
                    let semis = [-12, -7, -5, -3, 0, 3, 5, 7, 12];
                    ui.horizontal_wrapped(|ui| {
                        for (i, s) in semis.iter().enumerate() { ui.checkbox(&mut self.mp_intervals[i], format!("{:+}", s)); }
                    });
                }
                FxTab::More => {
                    self.mf_type = select_row(ui, "Effect", self.mf_type, &[(MoreFx::Reverse, "Reverse"), (MoreFx::Echo, "Echo"), (MoreFx::Distortion, "Distortion")]);
                    if self.mf_type == MoreFx::Echo {
                        slider_row_f(ui, "Delay", &mut self.mf_delay, 10.0, 800.0, 1.0, |v| format!("{:.0} ms", v));
                        slider_row_f(ui, "Feedback", &mut self.mf_fb, 0.0, 0.95, 0.01, |v| format!("{:.2}", v));
                    }
                    if self.mf_type == MoreFx::Distortion {
                        slider_row_f(ui, "Drive", &mut self.mf_drive, 1.0, 20.0, 0.1, |v| format!("{:.1}", v));
                    }
                }
                FxTab::Studio => {
                    let opts: Vec<(StudioFx, String)> = [StudioFx::Wah, StudioFx::Flanger, StudioFx::Phaser, StudioFx::Chorus, StudioFx::Reverb, StudioFx::Fade, StudioFx::Normalize, StudioFx::Lowpass, StudioFx::Highpass, StudioFx::Ringmod, StudioFx::Autopan, StudioFx::Compressor, StudioFx::Gain, StudioFx::Bassboost, StudioFx::Telephone, StudioFx::Gate, StudioFx::Stutter, StudioFx::Vinyl, StudioFx::Octaver].iter().map(|&s| (s, s.label().to_string())).collect();
                    self.sf_type = select_row_owned(ui, "Effect", self.sf_type, &opts);
                    let f = self.sf_type.fields();
                    if f.modw { slider_row_f(ui, "Rate", &mut self.sf_rate, 0.1, 10.0, 0.1, |v| format!("{:.1} Hz", v)); slider_row_f(ui, "Depth", &mut self.sf_depth, 0.0, 1.0, 0.01, |v| format!("{:.2}", v)); }
                    if f.rev { slider_row_f(ui, "Decay", &mut self.sf_decay, 0.3, 6.0, 0.1, |v| format!("{:.1} s", v)); }
                    if f.fade { slider_row_f(ui, "Fade in", &mut self.sf_in, 0.0, 10.0, 0.1, |v| format!("{:.1} s", v)); slider_row_f(ui, "Fade out", &mut self.sf_out, 0.0, 10.0, 0.1, |v| format!("{:.1} s", v)); }
                    if f.mixw { slider_row_f(ui, "Mix", &mut self.sf_mix, 0.0, 1.0, 0.01, |v| format!("{:.2}", v)); }
                    if f.fcw { slider_row_f(ui, "Frequency", &mut self.sf_fc, 50.0, 12_000.0, 10.0, |v| format!("{:.0} Hz", v)); }
                    if f.gainw { slider_row_f(ui, "Gain", &mut self.sf_gain, -24.0, 24.0, 0.5, |v| format!("{:+.1} dB", v)); }
                }
            }

            ui.add_space(6.0);
            let run = ui.add(egui::Button::new(RichText::new(if self.processing { "Rendering…" } else { "Apply Effect & Render WAV" }).color(Color32::WHITE)).fill(ACCENT2).rounding(Rounding::same(4.0)).min_size(vec2(ui.available_width(), 30.0)));
            if run.clicked() && !self.processing {
                self.processing = true;
                self.progress = 0.0;
                let out_path = media_engine::default_export_path("grain_audio_export.wav");
                let out_str = out_path.to_string_lossy().to_string();
                match self.render_real(&out_str, plugins) {
                    Ok(notes) => {
                        self.processing = false; self.done = true; self.progress = 1.0;
                        self.status = format!(
                            "Rendered DSP{} (non-destructive) → {}{}",
                            if plugins.active_count() > 0 { " + plug-ins" } else { "" },
                            out_str,
                            if notes.is_empty() { String::new() } else { format!(" · {}", notes.join(" · ")) }
                        );
                    }
                    Err(e) => {
                        self.processing = false; self.done = false;
                        self.status = format!("Render error: {e}");
                    }
                }
            }
            if !self.status.is_empty() {
                ui.add_space(3.0);
                ui.label(RichText::new(&self.status).color(ACCENT).size(10.0));
            }
            if plugins.active_count() > 0 {
                ui.add_space(2.0);
                ui.label(RichText::new(format!(
                    "{} plug-in slot(s) in the FX chain — Render/Export applies them in slot order.",
                    plugins.active_count()
                )).color(SUB).size(9.0));
            }
        });
        self.render_audio_fx_popup(ctx, plugins);
    }

    fn render_audio_fx_popup(&mut self, ctx: &egui::Context, plugins: &mut PluginChain) {
        if !self.audio_fx_popup_open { return; }
        let mut open = true;
        egui::Window::new("Audio FX")
            .collapsible(true)
            .resizable(false)
            .open(&mut open)
            .default_width(360.0)
            .default_height(420.0)
            .min_width(360.0)
            .max_width(360.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("Audio FX · new window").color(INK).size(13.0));
                ui.label(RichText::new("Non-destructive DSP + VST2 / VST3 / AU chain").color(SUB).size(10.0));
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    for &t in FxTab::ALL {
                        let active = self.fx_tab == t;
                        if ui.add(egui::Button::new(RichText::new(t.label()).color(if active { CLIP_TXT } else { SUB }).size(10.0))
                            .fill(if active { ACCENT } else { Color32::TRANSPARENT })
                            .stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(3.0))).clicked() { self.fx_tab = t; }
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new(self.fx_tab.note()).color(SUB).size(10.0));
                ui.separator();
                ui.label(RichText::new(format!("{} plug-in slot(s) active", plugins.active_count())).color(SUB).size(10.0));
                ui.add_space(4.0);
                if ui.add(egui::Button::new(RichText::new("Apply Effect & Render WAV").color(Color32::WHITE))
                    .fill(ACCENT2).rounding(Rounding::same(4.0)).min_size(vec2(ui.available_width(), 28.0))).clicked()
                {
                    let out_path = media_engine::default_export_path("grain_audio_export.wav");
                    let out_str = out_path.to_string_lossy().to_string();
                    match self.render_real(&out_str, plugins) {
                        Ok(notes) => { self.done = true; self.status = format!("Rendered → {}{}", out_str, if notes.is_empty() { String::new() } else { format!(" · {}", notes.join(" · ")) }); }
                        Err(e) => { self.done = false; self.status = format!("Render error: {e}"); }
                    }
                }
                if !self.status.is_empty() { ui.label(RichText::new(&self.status).color(ACCENT).size(10.0)); }
            });
        if !open { self.audio_fx_popup_open = false; }
    }

    fn to_lab(&self) -> AudioLabState {
        let mut st = AudioLabState::default();
        st.active_tab = match self.fx_tab {
            FxTab::Pitch => LabTab::PitchShift, FxTab::Time => LabTab::TimeStretch,
            FxTab::Vibrato => LabTab::Vibrato, FxTab::SpeedPitch => LabTab::SpeedPitch,
            FxTab::Bitcrush => LabTab::Bitcrush, FxTab::Vocoder => LabTab::Vocoder,
            FxTab::Tremolo => LabTab::Tremolo, FxTab::Invert => LabTab::Invert,
            FxTab::MultiPitch => LabTab::MultiPitch, FxTab::More => LabTab::MoreFx,
            FxTab::Studio => LabTab::StudioFx,
        };
        let tech = |p: PitchTech| match p {
            PitchTech::ElastiquePro => LabTech::ElastiquePro,
            PitchTech::ElastiqueA17 => LabTech::ElastiqueA17,
            PitchTech::Fft => LabTech::Fft,
            PitchTech::Wsola => LabTech::Wsola,
            PitchTech::Rb => LabTech::RubberbandRefined,
            PitchTech::Bungee => LabTech::Bungee,
        };
        st.pitch_tech = tech(self.p_tech);
        st.pitch_semitones = self.p_amt;
        st.fft_size = self.fft_size as usize;
        st.formant_semitones = self.fm_amt;
        st.formant_keep = self.fm_keep;
        st.time_tech = tech(self.t_tech);
        st.time_factor = self.t_amt;
        st.vibrato_pitch_only = self.v_mode == VibMode::A;
        st.vibrato_tech = tech(self.v_tech);
        st.vibrato_rate = self.v_rate;
        st.vibrato_depth = self.v_depth;
        st.speed_pitch_rate = self.s_amt;
        st.speed_pitch_start = self.s_start;
        st.speed_pitch_end = self.s_end;
        st.speed_pitch_sliding = self.s_sliding;
        st.bit_depth = self.b_bits.clamp(1, 16) as u8;
        st.sr_divisor = self.b_sr.max(1) as u32;
        st.vocoder_wave = match self.vc_wave { VocWave::Saw => CarrierWave::Saw, VocWave::Square => CarrierWave::Square, VocWave::Pulse => CarrierWave::Pulse, VocWave::Sine => CarrierWave::Sine };
        st.vocoder_freq = self.vc_freq;
        st.vocoder_bands = self.vc_bands.max(1) as usize;
        st.tremolo_wave = match self.tr_wave { TremWave::Sine => LabTrem::Sine, TremWave::Triangle => LabTrem::Triangle, TremWave::Square => LabTrem::Square, TremWave::Saw => LabTrem::Saw, TremWave::Isaw => LabTrem::InvSaw, TremWave::Noise => LabTrem::Noise, TremWave::Snoise => LabTrem::SmoothNoise };
        st.tremolo_rate = self.tr_rate;
        st.tremolo_depth = self.tr_depth;
        st.mp_tech = tech(self.mp_tech);
        st.mp_intervals = self.mp_intervals;
        st.more_type = match self.mf_type { MoreFx::Reverse => LabMore::Reverse, MoreFx::Echo => LabMore::Echo, MoreFx::Distortion => LabMore::Distortion };
        st.echo_delay_ms = self.mf_delay;
        st.echo_feedback = self.mf_fb;
        st.dist_drive = self.mf_drive;
        st.studio_type = match self.sf_type {
            StudioFx::Wah => LabStudio::Wah, StudioFx::Flanger => LabStudio::Flanger, StudioFx::Phaser => LabStudio::Phaser,
            StudioFx::Chorus => LabStudio::Chorus, StudioFx::Reverb => LabStudio::Reverb, StudioFx::Fade => LabStudio::Fade,
            StudioFx::Normalize => LabStudio::Normalize, StudioFx::Lowpass => LabStudio::Lowpass, StudioFx::Highpass => LabStudio::Highpass,
            StudioFx::Ringmod => LabStudio::RingMod, StudioFx::Autopan => LabStudio::AutoPan, StudioFx::Compressor => LabStudio::Compressor,
            StudioFx::Gain => LabStudio::Gain, StudioFx::Bassboost => LabStudio::BassBoost, StudioFx::Telephone => LabStudio::Telephone,
            StudioFx::Gate => LabStudio::Gate, StudioFx::Stutter => LabStudio::Stutter, StudioFx::Vinyl => LabStudio::Vinyl,
            StudioFx::Octaver => LabStudio::Octaver,
        };
        st.sf_rate = self.sf_rate; st.sf_depth = self.sf_depth; st.sf_decay = self.sf_decay;
        st.sf_mix = self.sf_mix; st.sf_fc = self.sf_fc; st.sf_gain_db = self.sf_gain;
        st.sf_fade_in = self.sf_in; st.sf_fade_out = self.sf_out;
        st.export_sample_rate = self.sample_rate;
        st
    }

    /// Apply the Audacity-style track strip (mute / gain / pan) and then the
    /// VST2 / VST3 / AU insertion chain. Returns one note per plug-in failure.
    fn apply_track_and_plugins(&self, l: &mut [f32], r: &mut [f32], plugins: &mut PluginChain) -> Vec<String> {
        if self.mute {
            l.fill(0.0);
            r.fill(0.0);
            return Vec::new();
        }
        let gain = 10.0_f32.powf(self.track_gain_db / 20.0);
        let pan = self.track_pan.clamp(-1.0, 1.0);
        // Constant-power pan law, shared with CPAL live playback.
        let angle = std::f32::consts::FRAC_PI_4 * (pan + 1.0);
        let lg = (gain * angle.cos()).clamp(-4.0, 4.0);
        let rg = (gain * angle.sin()).clamp(-4.0, 4.0);
        for i in 0..l.len().min(r.len()) {
            l[i] *= lg;
            r[i] *= rg;
        }
        plugins.render(l, r, self.sample_rate)
    }

    /// Run the real `rustfft` DSP chain + plug-in chain on the loaded source and write a WAV.
    fn render_real(&self, out: &str, plugins: &mut PluginChain) -> Result<Vec<String>, String> {
        let lab = self.to_lab();
        let ch = self.channels.max(1) as usize;
        let n = self.samples.len() / ch;
        let mut l = Vec::with_capacity(n);
        let mut r = Vec::with_capacity(n);
        for i in 0..n {
            l.push(self.samples[i * ch]);
            r.push(if ch > 1 { self.samples[i * ch + 1] } else { self.samples[i * ch] });
        }
        if l.is_empty() { return Err("no source samples".into()); }
        let (mut ol, mut or_) = audio_dsp::process_stereo(&l, &r, self.sample_rate, &lab);
        let notes = self.apply_track_and_plugins(&mut ol, &mut or_, plugins);
        media_engine::write_wav_f32(std::path::Path::new(out), &ol, &or_, self.sample_rate)?;
        Ok(notes)
    }


}

// >>> VIDEO_IMPL_HERE <<<

// ---- media helpers ----
/// Probe a media file's duration in seconds via `ffprobe` (best-effort; returns
/// None if ffprobe is missing or the file can't be probed).
fn ffprobe_duration(path: &std::path::Path) -> Option<f32> {
    let out = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1"])
        .arg(path)
        .output().ok()?;
    if !out.status.success() { return None; }
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim().parse::<f32>().ok().filter(|d| d.is_finite() && *d > 0.0)
}

// ---- color helpers (h in degrees, s/v/r/g/b in 0..1) ----
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {    let c = v * s;
    let h60 = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (h60.rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h60 as i32 {
        0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x),
        3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x),
    };
    (r + m, g + m, b + m)
}
fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    let h = if d == 0.0 { 0.0 } else if mx == r { 60.0 * (((g - b) / d).rem_euclid(6.0)) } else if mx == g { 60.0 * ((b - r) / d + 2.0) } else { 60.0 * ((r - g) / d + 4.0) };
    let s = if mx == 0.0 { 0.0 } else { d / mx };
    (h, s, mx)
}

// bezier curve LUT (replicates index.html lutOf): 256-entry map x->y in 0..255
fn lut_of(pts: CurvePts) -> [u8; 256] {
    let mut lut = [0u8; 256];
    let [p0, p1] = [pts.p0, pts.p1];
    let mut last = 0usize;
    for i in 0..=300 {
        let t = i as f32 / 300.0;
        let u = 1.0 - t;
        let x = 3.0 * u * u * t * p0[0] + 3.0 * u * t * t * p1[0] + t * t * t;
        let y = 3.0 * u * u * t * p0[1] + 3.0 * u * t * t * p1[1] + t * t * t;
        let xi = (x * 255.0).round().clamp(0.0, 255.0) as usize;
        let yv = (y * 255.0).round().clamp(0.0, 255.0) as u8;
        for k in last..=xi.min(255) { lut[k] = yv; }
        last = (xi + 1).min(256);
    }
    for k in last..256 { lut[k] = 255; }
    lut
}

impl VideoState {
    fn total(&self) -> f32 {
        let mut s = 0.0_f32;
        for c in &self.seq { s += c.len(); }
        for track in &self.extra_tracks {
            for c in track { s = s.max(c.start + c.len()); }
        }
        s.max(1.0)
    }

    /// V1 plus every extra video track.
    fn video_track_count(&self) -> usize { self.extra_tracks.len() + 1 }

    /// Insert a clip on a video track. V1 stays contiguous; V2+ use `start`.
    fn insert_clip_on_track(&mut self, clip: VideoClip, track: usize) {
        if track == 0 {
            let start: f32 = self.seq.iter().map(|c| c.len()).sum();
            let mut c = clip; c.start = start; c.track = 0;
            self.seq.push(c);
        } else if track <= self.extra_tracks.len() {
            let mut c = clip; c.track = track;
            self.extra_tracks[track - 1].push(c);
        }
        self.dirty = true;
    }

    fn add_video_track(&mut self) {
        self.extra_tracks.push(Vec::new());
        self.extra_track_mute.push(false);
        self.extra_track_solo.push(false);
        self.track_visible.push(true);
        self.status = format!("Added V{}", self.video_track_count());
    }

    fn audio_track_count(&self) -> usize { 1 + self.extra_audio_mute.len() }

    fn add_audio_track(&mut self) {
        self.extra_audio_mute.push(false);
        self.extra_audio_solo.push(false);
        self.status = format!("Added A{}", self.audio_track_count() + 1);
    }

    fn delete_audio_track(&mut self) {
        if self.extra_audio_mute.is_empty() {
            self.status = "A2 cannot be deleted — it is the primary audio track.".into();
            return;
        }
        let a = self.extra_audio_mute.len(); // A3+ index before removal
        self.extra_audio_mute.pop();
        self.extra_audio_solo.pop();
        // Keep clips instead of discarding them: move them down to A2.
        for m in &mut self.music { if m.track == a { m.track = 0; } }
        self.status = format!("Removed A{}; its clips moved to A2.", a + 2);
    }

    /// Delete the last video track, preserving its clips by appending them to
    /// V1 in timeline order instead of discarding user work.
    fn delete_video_track(&mut self) {
        if self.extra_tracks.is_empty() {
            self.status = "V1 cannot be deleted — it is the primary track.".into();
            return;
        }
        let mut moved = self.extra_tracks.pop().unwrap_or_default();
        self.extra_track_mute.pop();
        self.extra_track_solo.pop();
        self.track_visible.truncate(5 + self.extra_tracks.len());
        moved.sort_by(|a, b| a.start.total_cmp(&b.start));
        let mut start: f32 = self.seq.iter().map(|c| c.len()).sum();
        for mut c in moved {
            c.track = 0; c.start = start; start += c.len();
            self.seq.push(c);
        }
        self.dirty = true;
        self.status = format!("Removed V{}; its clips moved to V1.", self.video_track_count() + 1);
    }

    /// True when at least one track is soloed. Solo overrides mute.
    fn any_solo(&self) -> bool {
        self.track_solo.iter().any(|&s| s)
            || self.extra_track_solo.iter().any(|&s| s)
            || self.extra_audio_solo.iter().any(|&s| s)
    }

    /// Whether a timeline track contributes to the preview/export. Indices
    /// 0..5 are V1 / Overlay / A1 / A2 / Text; 5.. are V2..Vn; 6.. are A3...
    fn track_active(&self, idx: usize) -> bool {
        let (mute, solo) = if idx < self.track_mute.len() {
            (self.track_mute[idx], self.track_solo[idx])
        } else if idx >= 5 && idx < 5 + self.extra_track_mute.len() {
            let e = idx - 5;
            (self.extra_track_mute[e], self.extra_track_solo[e])
        } else if idx >= 6 + self.extra_track_mute.len()
            && idx < 6 + self.extra_track_mute.len() + self.extra_audio_mute.len()
        {
            let e = idx - (6 + self.extra_track_mute.len());
            (self.extra_audio_mute[e], self.extra_audio_solo[e])
        } else {
            return false;
        };
        let is_audio = idx == 2 || idx == 3 || idx >= 6 + self.extra_track_mute.len();
        let visible = if is_audio { true } else { self.track_visible.get(idx).copied().unwrap_or(true) };
        !mute && visible && (!self.any_solo() || solo)
    }

    fn audio_track_active(&self, a: usize) -> bool {
        if a == 0 { self.track_active(3) }
        else if a - 1 < self.extra_audio_mute.len() {
            let e = a - 1;
            !self.extra_audio_mute[e] && (!self.any_solo() || self.extra_audio_solo[e])
        } else { false }
    }

    fn add_marker(&mut self, t: f32) {
        let t = t.max(0.0);
        if !self.markers.iter().any(|m| (m - t).abs() < 0.05) {
            self.markers.push(t);
            self.markers.sort_by(|a, b| a.total_cmp(b));
        }
        self.status = format!("Marker at {}", timeline::format_tc(t, self.fps));
    }

    // ---- keyframes ----
    fn kf_pack(&self) -> [f32; 20] {
        [self.vb, self.vc, self.vs, self.vh, self.vbl, self.vv, self.vwh, self.vwv, self.vtilt, self.vyaw, self.vcube, self.vrgb, self.vpx, self.vgr, self.vsc, self.vgl, self.vpo, self.vkl, self.vsk, self.vtl]
    }
    fn kf_apply(&mut self, v: [f32; 20]) {
        let [vb, vc, vs, vh, vbl, vv, vwh, vwv, vtilt, vyaw, vcube, vrgb, vpx, vgr, vsc, vgl, vpo, vkl, vsk, vtl] = v;
        self.vb = vb; self.vc = vc; self.vs = vs; self.vh = vh; self.vbl = vbl; self.vv = vv;
        self.vwh = vwh; self.vwv = vwv; self.vtilt = vtilt; self.vyaw = vyaw; self.vcube = vcube; self.vrgb = vrgb;
        self.vpx = vpx; self.vgr = vgr; self.vsc = vsc; self.vgl = vgl; self.vpo = vpo; self.vkl = vkl; self.vsk = vsk; self.vtl = vtl;
    }
    fn upsert_kf(&mut self, v: [f32; 20]) {
        let t = self.playhead;
        if let Some(k) = self.keyframes.iter_mut().find(|k| (k.t - t).abs() < 0.05) {
            k.v = v;
        } else {
            self.keyframes.push(Keyframe { t, v });
            self.keyframes.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
        }
    }
    fn apply_kf_at(&mut self, t: f32) {
        let kf = &self.keyframes;
        if kf.is_empty() { return; }
        let v = if t <= kf[0].t {
            kf[0].v
        } else if t >= kf[kf.len() - 1].t {
            kf[kf.len() - 1].v
        } else {
            let mut a = &kf[0];
            let mut b = &kf[kf.len() - 1];
            for i in 0..kf.len() - 1 {
                if t >= kf[i].t && t <= kf[i + 1].t { a = &kf[i]; b = &kf[i + 1]; break; }
            }
            let f = if b.t - a.t == 0.0 { 0.0 } else { (t - a.t) / (b.t - a.t) };
            let mut out = [0.0_f32; 20];
            for i in 0..20 { out[i] = a.v[i] + (b.v[i] - a.v[i]) * f; }
            out
        };
        self.kf_apply(v);
    }

    fn tick(&mut self, dt: f32, _ctx: &egui::Context) {
        let before = self.playhead;
        if !self.keyframes.is_empty() { self.apply_kf_at(self.playhead); self.dirty = true; }
        if self.is_playing {
            // The audio engine remains the master clock and overwrites this
            // value in `GrainApp::update` whenever real audio is loaded.
            self.playhead += dt * self.speed;
            let total = self.total();
            if self.playhead >= total { self.playhead = total; self.is_playing = false; }
        }
        if (self.playhead - before).abs() > 1e-6 { self.dirty = true; }
        // Export sets `done`/`progress` only on real completion; never fake it.
    }

    fn ensure_src(&mut self) {
        if !self.src_pix.is_empty() { return; }
        let w = self.fw; let h = self.fh;
        let mut pix = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let fx = x as f32 / w as f32; let fy = y as f32 / h as f32;
                let (mut r, mut g, mut b) = hsv_to_rgb(fx * 300.0, 0.5, 0.72);
                let dx = fx - 0.5; let dy = fy - 0.5; let d = (dx * dx + dy * dy).sqrt();
                if d < 0.16 { r = 0.98; g = 0.93; b = 0.82; }
                else if d < 0.19 { let f = (0.19 - d) / 0.03; r = r * (1.0 - f) + 0.9 * f; g = g * (1.0 - f) + 0.85 * f; b = b * (1.0 - f) + 0.7 * f; }
                if (fx + fy).rem_euclid(0.25) < 0.05 { r *= 0.7; g *= 0.7; b *= 0.7; }
                pix.push((r * 255.0) as u8); pix.push((g * 255.0) as u8); pix.push((b * 255.0) as u8); pix.push(255);
            }
        }
        self.src_pix = pix;
    }

    fn clip_frame_key(c: &VideoClip) -> String {
        let q = |v: f32| (v * 1000.0).round() as i64;
        let nested = c.compound.as_ref()
            .map(|v| v.iter().map(Self::clip_frame_key).collect::<Vec<_>>().join(">"))
            .unwrap_or_default();
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{:?}|{:?}|{}|{}",
            c.uid, q(c.start), c.track, q(c.a), q(c.b), q(c.dur), q(c.speed), c.gen, c.path,
            c.compound.is_some(), nested
        )
    }

    /// Cheap string signature used to skip expensive per-pixel grading when the
    /// timeline is paused and no grade/source value changed.
    fn frame_key(&self) -> String {
        let q = |v: f32| (v * 1000.0).round() as i64;
        let clips = self.seq.iter().map(Self::clip_frame_key).collect::<Vec<_>>().join(";");
        let extra = self.extra_tracks.iter().map(|t| t.iter().map(Self::clip_frame_key).collect::<Vec<_>>().join(",")).collect::<Vec<_>>().join(";");
        let curves = self.curves.iter()
            .map(|c| format!("{:?}/{:?}", c.p0, c.p1))
            .collect::<Vec<_>>().join(";");
        let ofx = self.ofx_active.as_ref()
            .map(|f| format!("{:?}|{}|{}|{}|{}", f.kind, q(f.mix), q(f.intensity), q(f.p_a), q(f.p_b)))
            .unwrap_or_default();
        let mut parts = Vec::new();
        parts.push(format!("{:?}", (
            self.fw, self.fh, q(self.playhead), self.fps, q(self.speed),
        )));
        parts.push(format!("{:?}", (
            self.generator_kind, self.track_mute, self.track_solo,
            self.extra_track_mute.clone(), self.extra_track_solo.clone(),
            self.extra_audio_mute.clone(), self.extra_audio_solo.clone(), extra,
            self.media_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        )));
        parts.push(format!("{:?}", (
            self.look, self.mirror, self.vtr_type, self.vmk_type, self.vcv_ch,
        )));
        parts.push(format!("{:?}", (
            q(self.vb), q(self.vc), q(self.vs), q(self.vh), q(self.vbl), q(self.vv),
            q(self.vwh), q(self.vwv), q(self.vtilt), q(self.vyaw),
        )));
        parts.push(format!("{:?}", (
            q(self.vcube), q(self.vrgb), q(self.vpx), q(self.vgr), q(self.vsc), q(self.vgl),
            q(self.vpo), q(self.vkl), q(self.vsk), q(self.vtl), q(self.vtr_dur),
        )));
        parts.push(curves);
        parts.push(format!("{:?}", (
            q(self.wheel_sh[0]), q(self.wheel_sh[1]),
            q(self.wheel_mid[0]), q(self.wheel_mid[1]),
            q(self.wheel_hi[0]), q(self.wheel_hi[1]), self.vhsl_b,
        )));
        parts.push(format!("{:?}", (
            q(self.vhsl_h), q(self.vhsl_s), q(self.vhsl_l),
            q(self.vmk_x), q(self.vmk_y), q(self.vmk_s), q(self.vmk_f), self.vmk_i,
        )));
        parts.push(format!("{:?}", (
            self.aces_enabled, self.aces_view,
            q(self.lgg_lift[0]), q(self.lgg_lift[1]), q(self.lgg_lift[2]),
            q(self.lgg_gamma[0]), q(self.lgg_gamma[1]), q(self.lgg_gamma[2]),
        )));
        parts.push(format!("{:?}", (
            q(self.lgg_gain[0]), q(self.lgg_gain[1]), q(self.lgg_gain[2]),
            q(self.lgg_offset[0]), q(self.lgg_offset[1]), q(self.lgg_offset[2]),
        )));
        parts.push(ofx);
        parts.push(clips);
        parts.join("|")
    }

    // Per-pixel grade: brightness/contrast/sat/hue/look/curves/wheels/HSL/
    // vignette/grain/scanlines/rgb-split/pixelate/posterize/blur/mirror/mask.
    fn recompute_frame(&mut self, ctx: &egui::Context) {
        let w = self.fw; let h = self.fh;
        let key = self.frame_key();
        if !self.dirty
            && key == self.last_frame_key
            && self.out_tex.is_some()
            && self.last_rgba.len() == w * h * 4
        {
            return;
        }
        let src = self.source_rgba();
        let mut px: Vec<[f32; 3]> = (0..w * h).map(|i| {
            let s = i * 4;
            [src[s] as f32 / 255.0, src[s + 1] as f32 / 255.0, src[s + 2] as f32 / 255.0]
        }).collect();

        // Motion trail blends the freshly graded frame with the previous one.
        let trail_prev = if self.vtl > 0.001 && self.last_rgba.len() == w * h * 4 {
            Some(self.last_rgba.clone())
        } else {
            None
        };

        // Clip-start / clip-end transition progress (Vegas-style edge fades).
        let mut trans_kind = self.vtr_type;
        let mut trans_t = 1.0_f32;
        if self.vtr_type != TransType::None {
            let mut acc = 0.0_f32;
            for c in &self.seq {
                let clen = c.len();
                if self.playhead >= acc && self.playhead <= acc + clen {
                    let into = self.playhead - acc;
                    let from_end = acc + clen - self.playhead;
                    let d = self.vtr_dur.min(clen * 0.5).max(0.01);
                    let edge = into.min(from_end);
                    if edge < d { trans_t = (edge / d).clamp(0.0, 1.0); }
                    break;
                }
                acc += clen;
            }
        } else {
            trans_kind = TransType::None;
        }

        let bright = self.vb / 100.0;
        let contrast = self.vc / 100.0;
        let sat = self.vs / 100.0;
        let hue = self.vh;
        let look = self.look;
        let luts = [lut_of(self.curves[0]), lut_of(self.curves[1]), lut_of(self.curves[2]), lut_of(self.curves[3])];
        let curves_on = self.curves.iter().any(|p| (p.p0[0] - p.p0[1]).abs() > 0.004 || (p.p1[0] - p.p1[1]).abs() > 0.004);
        let wheel = |wh: [f32; 2]| -> (f32, f32, f32) {
            let mag = (wh[0] * wh[0] + wh[1] * wh[1]).sqrt();
            if mag < 0.001 { (0.0, 0.0, 0.0) } else {
                let th = wh[1].atan2(wh[0]);
                (0.5 + 0.5 * th.cos(), 0.5 + 0.5 * th.sin(), 0.5 - 0.5 * th.cos())
            }
        };
        let (sh_r, sh_g, sh_b) = wheel(self.wheel_sh);
        let (mid_r, mid_g, mid_b) = wheel(self.wheel_mid);
        let (hi_r, hi_g, hi_b) = wheel(self.wheel_hi);
        let hsl_hue = [0.0_f32, 30.0, 60.0, 120.0, 180.0, 240.0, 280.0, 320.0][self.vhsl_b as usize];

        let pblk = self.vpx.round() as usize;
        if pblk >= 2 {
            let mut i = 0;
            while i < h {
                let mut j = 0;
                while j < w {
                    let mut acc = [0.0_f32; 3]; let mut n = 0usize;
                    for yy in i..(i + pblk).min(h) { for xx in j..(j + pblk).min(w) { let p = px[yy * w + xx]; acc[0] += p[0]; acc[1] += p[1]; acc[2] += p[2]; n += 1; } }
                    let n = n as f32; acc[0] /= n; acc[1] /= n; acc[2] /= n;
                    for yy in i..(i + pblk).min(h) { for xx in j..(j + pblk).min(w) { px[yy * w + xx] = acc; } }
                    j += pblk;
                }
                i += pblk;
            }
        }

        for y in 0..h {
            for x in 0..w {
                let p = &mut px[y * w + x];
                for c in 0..3 { p[c] = (p[c] * bright - 0.5) * contrast + 0.5; }
                let (hh, ss, vv) = rgb_to_hsv(p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0), p[2].clamp(0.0, 1.0));
                let (mut rh, mut rs, mut rv) = (hh + hue, ss * sat, vv);
                if ((hh - hsl_hue + 360.0).rem_euclid(360.0)).min((hh - hsl_hue - 360.0).rem_euclid(360.0).abs()) < 22.0 {
                    rh += self.vhsl_h * 1.8;
                    rs *= 1.0 + self.vhsl_s / 100.0;
                    rv *= 1.0 + self.vhsl_l / 100.0;
                }
                let (mut r, mut g, mut b) = hsv_to_rgb(rh, rs.clamp(0.0, 1.0), rv.clamp(0.0, 1.0));
                match look {
                    LookType::Grayscale => { let l = 0.299 * r + 0.587 * g + 0.114 * b; r = l; g = l; b = l; }
                    LookType::Sepia => { let rr = 0.393 * r + 0.769 * g + 0.189 * b; let gg = 0.349 * r + 0.686 * g + 0.168 * b; let bb = 0.272 * r + 0.534 * g + 0.131 * b; r = rr; g = gg; b = bb; }
                    LookType::Invert => { r = 1.0 - r; g = 1.0 - g; b = 1.0 - b; }
                    LookType::None => {}
                }
                let lum = 0.299 * r + 0.587 * g + 0.114 * b;
                let shw = (1.0 - lum).min(1.0);
                let midw = (1.0 - (lum - 0.5).abs() * 2.0).max(0.0);
                let hiw = lum.min(1.0);
                let s_mag = (self.wheel_sh[0].powi(2) + self.wheel_sh[1].powi(2)).sqrt();
                let m_mag = (self.wheel_mid[0].powi(2) + self.wheel_mid[1].powi(2)).sqrt();
                let h_mag = (self.wheel_hi[0].powi(2) + self.wheel_hi[1].powi(2)).sqrt();
                r += sh_r * shw * s_mag * 0.25 + mid_r * midw * m_mag * 0.2 + hi_r * hiw * h_mag * 0.25;
                g += sh_g * shw * s_mag * 0.25 + mid_g * midw * m_mag * 0.2 + hi_g * hiw * h_mag * 0.25;
                b += sh_b * shw * s_mag * 0.25 + mid_b * midw * m_mag * 0.2 + hi_b * hiw * h_mag * 0.25;
                if curves_on {
                    let ri = (r.clamp(0.0, 1.0) * 255.0) as usize;
                    let gi = (g.clamp(0.0, 1.0) * 255.0) as usize;
                    let bi = (b.clamp(0.0, 1.0) * 255.0) as usize;
                    r = luts[1][luts[0][ri] as usize] as f32 / 255.0;
                    g = luts[2][luts[0][gi] as usize] as f32 / 255.0;
                    b = luts[3][luts[0][bi] as usize] as f32 / 255.0;
                }
                if self.vpo > 0.0 {
                    let levels = (2.0 + (1.0 - self.vpo) * 6.0) as f32;
                    r = (r.clamp(0.0, 1.0) * levels).floor() / levels;
                    g = (g.clamp(0.0, 1.0) * levels).floor() / levels;
                    b = (b.clamp(0.0, 1.0) * levels).floor() / levels;
                }
                // Transition mask: fade to black, wipe, slide or iris-zoom.
                let fx = x as f32 / w as f32;
                let fy = y as f32 / h as f32;
                let trans_mask = match trans_kind {
                    TransType::Fade => trans_t,
                    TransType::Wipe => if fx < trans_t { 1.0 } else { 0.0 },
                    TransType::Slide => if fx > 1.0 - trans_t { 1.0 } else { 0.0 },
                    TransType::Zoom => {
                        let d = ((fx - 0.5).powi(2) + (fy - 0.5).powi(2)).sqrt() * 2.0;
                        if d < trans_t.max(0.02) { 1.0 } else { 0.0 }
                    }
                    TransType::None => 1.0,
                };
                p[0] = (r * trans_mask).clamp(0.0, 1.0);
                p[1] = (g * trans_mask).clamp(0.0, 1.0);
                p[2] = (b * trans_mask).clamp(0.0, 1.0);
            }
        }

        let shift = self.vrgb.round() as i32;
        if shift > 0 {
            let orig = px.clone();
            for y in 0..h { for x in 0..w {
                let xr = (x + shift as usize).min(w - 1);
                let xb = x.saturating_sub(shift as usize);
                px[y * w + x][0] = orig[y * w + xr][0];
                px[y * w + x][2] = orig[y * w + xb][2];
            } }
        }
        match self.mirror {
            MirrorType::Left => { for y in 0..h { for x in 0..w / 2 { px.swap(y * w + x, y * w + (w - 1 - x)); } } }
            MirrorType::Right => { for y in 0..h { for x in w / 2..w { let m = w - 1 - x; px.swap(y * w + x, y * w + m); } } }
            MirrorType::Top => { for x in 0..w { for y in 0..h / 2 { px.swap(y * w + x, (h - 1 - y) * w + x); } } }
            MirrorType::Bottom => { for x in 0..w { for y in h / 2..h { let m = h - 1 - y; px.swap(y * w + x, m * w + x); } } }
            MirrorType::None => {}
        }
        if self.vsc > 0.0 {
            for y in 0..h { if y % 2 == 1 { for x in 0..w { for c in 0..3 { px[y * w + x][c] *= 1.0 - self.vsc * 0.5; } } } }
        }
        if self.vgr > 0.0 {
            let mut seed: u64 = (self.playhead * 60.0).round() as u64 | 0x9e37_79b9;
            let mut rng = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed as f32) / (u32::MAX as f32) * 2.0 - 1.0 };
            for p in px.iter_mut() { let n = rng() * self.vgr * 0.25; for c in 0..3 { p[c] = (p[c] + n).clamp(0.0, 1.0); } }
        }
        let vig = self.vv;
        let mask_t = self.vmk_type;
        let (mx, my, ms, mf) = (self.vmk_x / 100.0, self.vmk_y / 100.0, self.vmk_s / 100.0, self.vmk_f / 100.0);
        let maski = self.vmk_i;
        for y in 0..h {
            for x in 0..w {
                let fx = x as f32 / w as f32; let fy = y as f32 / h as f32;
                let p = &mut px[y * w + x];
                if vig > 0.0 {
                    let d = ((fx - 0.5).powi(2) + (fy - 0.5).powi(2)).sqrt() * 1.4;
                    let f = (1.0 - (d - 0.6).max(0.0) * vig * 2.0).clamp(0.0, 1.0);
                    for c in 0..3 { p[c] *= f; }
                }
                if mask_t != MaskType::None {
                    let mut inside = match mask_t {
                        MaskType::Ellipse => ((fx - mx).powi(2) + (fy - my).powi(2)).sqrt() < ms,
                        MaskType::Rect => (fx - mx).abs() < ms && (fy - my).abs() < ms,
                        MaskType::Linear => (fx - mx).abs() < ms,
                        MaskType::None => true,
                    };
                    if mf > 0.0 && mask_t == MaskType::Ellipse {
                        let d = ((fx - mx).powi(2) + (fy - my).powi(2)).sqrt();
                        if d > ms - mf && d < ms + mf { inside = d < ms; }
                    }
                    if maski { inside = !inside; }
                    if !inside { for c in 0..3 { p[c] = 0.0; } }
                }
            }
        }
        if self.vbl > 0.0 {
            let r = (self.vbl / 2.0).round() as usize;
            let tmp = px.clone();
            for y in 0..h { for x in 0..w {
                let mut acc = [0.0_f32; 3]; let mut n = 0usize;
                for k in -(r as i32)..=(r as i32) {
                    let xx = (x as i32 + k).clamp(0, w as i32 - 1) as usize;
                    let p = tmp[y * w + xx]; acc[0] += p[0]; acc[1] += p[1]; acc[2] += p[2]; n += 1;
                }
                let n = n as f32; px[y * w + x] = [acc[0] / n, acc[1] / n, acc[2] / n];
            } }
        }

        // Geometric & Stylize passes: Wave (vwh / vwv), 3D Tilt/Yaw/Cube, Glitch (vgl), Kaleidoscope (vkl), Camera Shake (vsk)
        if self.vwh > 0.01 || self.vwv > 0.01 || self.vtilt.abs() > 0.01 || self.vyaw.abs() > 0.01 || self.vcube > 0.1 || self.vsk > 0.1 {
            let orig = px.clone();
            let ph_t = self.playhead * 4.0;
            let shake_dx = if self.vsk > 0.1 { ((ph_t * 17.3).sin() * self.vsk * 0.4) as i32 } else { 0 };
            let shake_dy = if self.vsk > 0.1 { ((ph_t * 23.7).cos() * self.vsk * 0.4) as i32 } else { 0 };
            let cube_scale = (1.0 - (self.vcube / 180.0)).clamp(0.35, 1.0);
            for y in 0..h {
                let fy = y as f32 / h as f32 - 0.5;
                let tilt_scale = (1.0 + fy * self.vtilt * 0.8).clamp(0.3, 2.2);
                let wave_dx = if self.vwh > 0.01 { ((fy * 18.0 + ph_t).sin() * self.vwh * 0.35) as i32 } else { 0 };
                for x in 0..w {
                    let fx = x as f32 / w as f32 - 0.5;
                    let yaw_scale = (1.0 + fx * self.vyaw * 0.8).clamp(0.3, 2.2);
                    let wave_dy = if self.vwv > 0.01 { ((fx * 18.0 + ph_t).cos() * self.vwv * 0.35) as i32 } else { 0 };
                    let sx = ((0.5 + (fx / (tilt_scale * cube_scale))) * w as f32) as i32 + wave_dx + shake_dx;
                    let sy = ((0.5 + (fy / (yaw_scale * cube_scale))) * h as f32) as i32 + wave_dy + shake_dy;
                    if sx >= 0 && sx < w as i32 && sy >= 0 && sy < h as i32 {
                        px[y * w + x] = orig[sy as usize * w + sx as usize];
                    } else {
                        px[y * w + x] = [0.0, 0.0, 0.0];
                    }
                }
            }
        }
        if self.vgl > 0.01 {
            let orig = px.clone();
            let slices = (self.vgl * 14.0).round() as usize;
            let seed_base = (self.playhead * 30.0).round() as usize;
            for s in 0..slices {
                let y0 = ((s * 73 + seed_base * 19) % h).min(h - 1);
                let band_h = ((s * 13 + 6) % 14).max(2);
                let shift = (((s * 41 + seed_base * 7) % 32) as i32 - 16) * (self.vgl * 2.0) as i32;
                for yy in y0..(y0 + band_h).min(h) {
                    for x in 0..w {
                        let sx = (x as i32 + shift).clamp(0, w as i32 - 1) as usize;
                        px[yy * w + x] = orig[yy * w + sx];
                    }
                }
            }
        }
        if self.vkl >= 2.0 {
            let orig = px.clone();
            let segs = self.vkl.round().max(2.0);
            let cx = w as f32 * 0.5;
            let cy = h as f32 * 0.5;
            let wedge = 2.0 * std::f32::consts::PI / segs;
            for y in 0..h {
                for x in 0..w {
                    let dx = x as f32 - cx;
                    let dy = y as f32 - cy;
                    let r = (dx * dx + dy * dy).sqrt();
                    let mut ang = dy.atan2(dx).rem_euclid(wedge);
                    if ang > wedge * 0.5 {
                        ang = wedge - ang;
                    }
                    let sx = (cx + r * ang.cos()).clamp(0.0, w as f32 - 1.0) as usize;
                    let sy = (cy + r * ang.sin()).clamp(0.0, h as f32 - 1.0) as usize;
                    px[y * w + x] = orig[sy * w + sx];
                }
            }
        }

        // BananaV ACES 1.3 (32-bit float) RRT/ODT + ASC-CDL Lift/Gamma/Gain/Offset pass
        bananav_suite::apply_aces_and_lgg(
            &mut px,
            self.aces_enabled,
            self.aces_view,
            self.lgg_lift,
            self.lgg_gamma,
            self.lgg_gain,
            self.lgg_offset,
        );

        // Staged OpenFX effect (real per-pixel pass driven by the OFX tab params).
        if let Some(fx) = &self.ofx_active { self.apply_ofx(&mut px, w, h, fx); }

        if let Some(prev) = trail_prev {
            let a = 1.0 - self.vtl;
            for (i, p) in px.iter_mut().enumerate() {
                let s = i * 4;
                for c in 0..3 {
                    p[c] = (p[c] * a + prev[s + c] as f32 / 255.0 * self.vtl).clamp(0.0, 1.0);
                }
            }
        }

        let mut out = Vec::with_capacity(w * h * 4);
        for p in &px { out.push((p[0].clamp(0.0, 1.0) * 255.0) as u8); out.push((p[1].clamp(0.0, 1.0) * 255.0) as u8); out.push((p[2].clamp(0.0, 1.0) * 255.0) as u8); out.push(255); }
        self.last_rgba = out.clone();
        let img = ColorImage::from_rgba_unmultiplied([w, h], &self.last_rgba);
        let opts = egui::TextureOptions::default();
        if let Some(t) = self.out_tex.as_mut() { t.set(img, opts); }
        else { self.out_tex = Some(ctx.load_texture("grain_preview", img, opts)); }
        self.last_frame_key = key;
        self.dirty = false;
    }

    /// Resolve timeline time to a source clip, expanding compounds recursively.
    fn resolve_in_clips(clips: &[VideoClip], t: f32) -> SourceRef {
        let mut acc = 0.0_f32;
        for c in clips {
            let clen = c.len();
            if t >= acc && t <= acc + clen + 0.001 {
                let local = t - acc;
                if let Some(gen) = c.gen {
                    return SourceRef { path: None, gen: Some(gen), time: local };
                }
                if let Some(nested) = &c.compound {
                    return Self::resolve_in_clips(nested, local);
                }
                return SourceRef {
                    path: c.path.clone(),
                    gen: None,
                    time: (c.a * c.dur + local * c.speed).clamp(0.0, c.dur.max(0.1)),
                };
            }
            acc += clen;
        }
        SourceRef { path: None, gen: None, time: t.max(0.0) }
    }

    /// Resolve one clip at an explicit timeline start (V2+ tracks).
    fn resolve_clip_at(c: &VideoClip, t: f32, start: f32) -> Option<SourceRef> {
        let clen = c.len();
        if t < start || t > start + clen + 0.001 { return None; }
        let local = (t - start).clamp(0.0, clen.max(0.0));
        if let Some(gen) = c.gen {
            return Some(SourceRef { path: None, gen: Some(gen), time: local });
        }
        if let Some(nested) = &c.compound {
            return Some(Self::resolve_in_clips(nested, local));
        }
        Some(SourceRef {
            path: c.path.clone(), gen: None,
            time: (c.a * c.dur + local * c.speed).clamp(0.0, c.dur.max(0.1)),
        })
    }

    fn resolve_source_at(&self, t: f32) -> SourceRef {
        let t = t.max(0.0);
        // Topmost active video track wins, so a clip placed on V2/V3 occludes V1.
        for ti in (0..self.extra_tracks.len()).rev() {
            if !self.track_active(5 + ti) { continue; }
            for c in &self.extra_tracks[ti] {
                if let Some(r) = Self::resolve_clip_at(c, t, c.start) { return r; }
            }
        }
        if !self.track_active(0) {
            return SourceRef { path: None, gen: None, time: t };
        }
        Self::resolve_in_clips(&self.seq, t)
    }

    /// Source pixels for the current frame: imported image, decoded video frame
    /// (ffmpeg subprocess via `ContinuousVideoStream`), generator, or test pattern.
    fn source_rgba(&mut self) -> Vec<u8> {
        let w = self.fw; let h = self.fh;
        let any_video = self.track_active(0)
            || (0..self.extra_tracks.len()).any(|i| self.track_active(5 + i))
            || self.generator_kind != bananav_suite::GeneratorKind::None;
        if !any_video {
            return vec![0; w * h * 4];
        }
        let src = self.resolve_source_at(self.playhead);
        if let Some(gen) = src.gen {
            return bananav_suite::generate_procedural_frame(gen, w, h, src.time);
        }
        if src.path.is_none() && self.generator_kind != bananav_suite::GeneratorKind::None {
            return bananav_suite::generate_procedural_frame(self.generator_kind, w, h, self.playhead);
        }
        if let Some(path) = src.path {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let is_img = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "webp" | "gif");
            let is_vid = matches!(ext.as_str(), "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" | "flv" | "ts" | "mpg" | "mpeg");
            if is_img {
                if let Ok(img) = image::open(&path) {
                    let rgba = img.resize_exact(w as u32, h as u32, image::imageops::FilterType::Triangle).to_rgba8();
                    return rgba.into_raw();
                }
            } else if is_vid {
                if self.video_stream.is_none() {
                    self.video_stream = Some(realtime_av::ContinuousVideoStream::new(w as u32, h as u32, self.fps as f64));
                }
                let vf = format!("scale={}:{}", w, h);
                if let Some(vs) = self.video_stream.as_mut() {
                    if let Ok(frame) = vs.read_frame_at(&path, src.time as f64, &vf) {
                        return frame;
                    }
                    // decode failed (e.g. ffmpeg missing / bad path) -> fall through to test pattern
                    self.video_stream = None;
                }
            }
        }
        if let Some(path) = self.media_path.clone() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "webp" | "gif") {
                if let Ok(img) = image::open(&path) {
                    let rgba = img.resize_exact(w as u32, h as u32, image::imageops::FilterType::Triangle).to_rgba8();
                    return rgba.into_raw();
                }
            }
        }
        self.ensure_src();
        self.src_pix.clone()
    }

    /// Real per-pixel OpenFX-style effect, classified from the plugin identifier.
    fn apply_ofx(&self, px: &mut Vec<[f32; 3]>, w: usize, h: usize, fx: &OfxFx) {
        let mix = (fx.mix / 100.0).clamp(0.0, 1.0);
        let inten = fx.intensity;
        match fx.kind {
            OfxKind::Generic => {
                let g = 1.0 + inten * 0.3;
                for p in px.iter_mut() { for c in 0..3 {
                    let orig = p[c];
                    p[c] = (orig + (orig * g - orig) * mix).clamp(0.0, 1.0);
                } }
            }
            OfxKind::Vignette => {
                let strength = inten * 0.9;
                for y in 0..h { for x in 0..w {
                    let fxp = x as f32 / w as f32; let fyp = y as f32 / h as f32;
                    let d = ((fxp - 0.5).powi(2) + (fyp - 0.5).powi(2)).sqrt() * 1.4;
                    let f = (1.0 - (d - 0.55).max(0.0) * strength * 2.5).clamp(0.0, 1.0);
                    let p = &mut px[y * w + x];
                    for c in 0..3 { p[c] = p[c] * (1.0 - mix) + p[c] * f * mix; }
                }}
            }
            OfxKind::Blur => {
                let r = (fx.p_a * 0.5).round() as usize;
                if r >= 1 { let bl = Self::box_blur(px, w, h, r); Self::blend(px, &bl, mix); }
            }
            OfxKind::Glow => {
                // bright-pass -> blur -> screen-add
                let thr = fx.p_a;
                let bright: Vec<[f32;3]> = px.iter().map(|p| {
                    let lum = 0.299*p[0]+0.587*p[1]+0.114*p[2];
                    let f = ((lum - thr).max(0.0) * inten * 1.5).min(1.5);
                    [p[0]*f, p[1]*f, p[2]*f]
                }).collect();
                let r = (fx.p_b * 0.15).round().max(1.0) as usize;
                let bl = Self::box_blur(&bright, w, h, r);
                for i in 0..px.len() {
                    let p = &mut px[i];
                    for c in 0..3 { let s = p[c] + bl[i][c] * mix; p[c] = s.min(1.0); }
                }
            }
            OfxKind::Sharpen => {
                let r = 1usize;
                let bl = Self::box_blur(px, w, h, r);
                for i in 0..px.len() {
                    let p = &mut px[i];
                    for c in 0..3 {
                        let s = p[c] + inten * 0.6 * (p[c] - bl[i][c]);
                        p[c] = p[c] * (1.0 - mix) + s.clamp(0.0,1.0) * mix;
                    }
                }
            }
            OfxKind::Warp => {
                let amp = fx.p_a * 0.02;
                let freq = fx.p_b;
                let orig = px.clone();
                for y in 0..h { for x in 0..w {
                    let dx = ((y as f32 * freq * 0.1).sin() * amp * w as f32) as i32;
                    let dy = ((x as f32 * freq * 0.1).sin() * amp * h as f32) as i32;
                    let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                    let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                    let p = &mut px[y * w + x]; let o = orig[sy * w + sx];
                    for c in 0..3 { p[c] = p[c] * (1.0 - mix) + o[c] * mix; }
                }}
            }
        }
    }

    fn box_blur(src: &[[f32; 3]], w: usize, h: usize, r: usize) -> Vec<[f32; 3]> {
        let mut tmp = vec![[0.0_f32; 3]; src.len()];
        // horizontal
        for y in 0..h { for x in 0..w {
            let mut acc = [0.0_f32; 3]; let mut n = 0usize;
            for k in -(r as i32)..=(r as i32) { let xx = (x as i32 + k).clamp(0, w as i32 - 1) as usize; let p = src[y * w + xx]; acc[0]+=p[0]; acc[1]+=p[1]; acc[2]+=p[2]; n+=1; }
            let n = n as f32; tmp[y * w + x] = [acc[0]/n, acc[1]/n, acc[2]/n];
        }}
        // vertical
        let mut out = vec![[0.0_f32; 3]; src.len()];
        for y in 0..h { for x in 0..w {
            let mut acc = [0.0_f32; 3]; let mut n = 0usize;
            for k in -(r as i32)..=(r as i32) { let yy = (y as i32 + k).clamp(0, h as i32 - 1) as usize; let p = tmp[yy * w + x]; acc[0]+=p[0]; acc[1]+=p[1]; acc[2]+=p[2]; n+=1; }
            let n = n as f32; out[y * w + x] = [acc[0]/n, acc[1]/n, acc[2]/n];
        }}
        out
    }

    fn blend(dst: &mut Vec<[f32; 3]>, src: &[[f32; 3]], mix: f32) {
        for i in 0..dst.len() {
            let d = &mut dst[i];
            for c in 0..3 { d[c] = d[c] * (1.0 - mix) + src[i][c] * mix; }
        }
    }


    /// Export the current graded frame (post-OFX) as a real PNG file.
    fn export_png(&self, path: &str) -> Result<(), String> {
        let w = self.fw; let h = self.fh;
        if self.last_rgba.len() != w * h * 4 {
            return Err("no frame rendered yet — move a slider first".into());
        }
        image::save_buffer(path, &self.last_rgba, w as u32, h as u32, image::ColorType::Rgba8)
            .map_err(|e| format!("png save: {e}"))
    }


    fn render_preview(&mut self, ui: &mut Ui, ctx: &egui::Context) -> Rect {
        // Only recompute pixels when the timeline or controls actually changed.
        // This keeps paused scrubbing/interaction responsive and avoids restarting
        // the ffmpeg pipe on every UI frame.
        self.recompute_frame(ctx);
        let pw = ui.available_width();
        let ph = (pw * self.fh as f32 / self.fw as f32).min(ui.available_height() - 28.0).max(120.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(pw, ph), Sense::click_and_drag());
        let p = ui.painter();
        p.rect_filled(rect, 2.0, Color32::BLACK);
        if let Some(tex) = &self.out_tex {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            p.image(tex.id(), rect, uv, Color32::WHITE);
        }
        // safe frame + center crosshair (Vegas-style)
        let safe = rect.shrink2(vec2(rect.width() * 0.05, rect.height() * 0.05));
        p.rect_stroke(safe, 0.0, Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 38)));
        p.line_segment([pos2(rect.center().x - 8.0, rect.center().y), pos2(rect.center().x + 8.0, rect.center().y)], Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 55)));
        p.line_segment([pos2(rect.center().x, rect.center().y - 8.0), pos2(rect.center().x, rect.center().y + 8.0)], Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 55)));
        if self.preview_transform_mode {
            let box_rect = rect.shrink2(vec2(rect.width() * 0.09, rect.height() * 0.09));
            p.rect_stroke(box_rect, 0.0, Stroke::new(1.5_f32, ACCENT));
            let s = 4.0_f32;
            for cp in [box_rect.left_top(), box_rect.right_top(), box_rect.left_bottom(), box_rect.right_bottom()] {
                p.rect_filled(Rect::from_center_size(cp, vec2(s * 2.0, s * 2.0)), 1.0, BG);
                p.rect_stroke(Rect::from_center_size(cp, vec2(s * 2.0, s * 2.0)), 1.0, Stroke::new(1.5_f32, ACCENT));
            }
            p.text(box_rect.left_top() + vec2(6.0, -6.0), egui::Align2::LEFT_BOTTOM, "Transform", FontId::proportional(9.0), ACCENT);
        }
        // text overlays (with shadow) — Text track M/S is respected
        if self.track_active(4) {
            for t in &self.texts {
                if self.playhead < t.a || (t.b < 1e8 && self.playhead > t.b) { continue; }
                let px = t.size / 100.0 * ph;
                let pos = pos2(rect.left() + t.x / 100.0 * pw, rect.top() + t.y / 100.0 * ph);
                p.text(pos + vec2(1.0, 1.0), egui::Align2::CENTER_CENTER, &t.t, FontId::proportional(px), Color32::from_rgba_unmultiplied(0, 0, 0, 180));
                p.text(pos, egui::Align2::CENTER_CENTER, &t.t, FontId::proportional(px), t.col);
            }
        }
        // overlay picture-in-picture — V2 track M/S is respected. Imported
        // image/video overlays draw their real poster frame; placeholders
        // remain for named-but-unlinked overlays.
        if self.track_active(1) {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            for o in &self.overlays {
                if self.playhead < o.start || self.playhead > o.start + o.dur { continue; }
                let w = rect.width() * o.s / 100.0; let h = w * 9.0 / 16.0;
                let cx = rect.left() + o.x / 100.0 * pw; let cy = rect.top() + o.y / 100.0 * ph;
                let r = Rect::from_center_size(pos2(cx, cy), vec2(w, h));
                let alpha = (255.0 * (o.o / 100.0).clamp(0.0, 1.0)) as u8;
                let tex_opts = egui::TextureOptions::default();
                let is_vid = o.path.as_ref()
                    .and_then(|p| p.extension().and_then(|e| e.to_str()))
                    .map(|e| matches!(e.to_lowercase().as_str(), "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v"))
                    .unwrap_or(false);
                let tex_id: Option<egui::TextureId> = o.path.as_ref().and_then(|path| {
                    if is_vid {
                        if self.overlay_stream.is_none() {
                            self.overlay_stream = Some(realtime_av::ContinuousVideoStream::new(
                                self.fw as u32, self.fh as u32, self.fps as f64,
                            ));
                        }
                        let vf = format!("scale={}:{}", self.fw, self.fh);
                        let local = (self.playhead - o.start).max(0.0);
                        let frame = self.overlay_stream.as_mut()
                            .and_then(|vs| vs.read_frame_at(path, local as f64, &vf).ok())?;
                        let cimg = ColorImage::from_rgba_unmultiplied([self.fw, self.fh], &frame);
                        if let Some(tex) = self.overlay_tex.get_mut(path) {
                            tex.set(cimg, tex_opts);
                            Some(tex.id())
                        } else {
                            let tex = ctx.load_texture(
                                format!("overlay_{}", path.to_string_lossy()), cimg, tex_opts,
                            );
                            let id = tex.id();
                            self.overlay_tex.insert(path.clone(), tex);
                            Some(id)
                        }
                    } else if let Some(tex) = self.overlay_tex.get(path) {
                        Some(tex.id())
                    } else {
                        let img = image::open(path).ok()?.to_rgba8();
                        let (iw, ih) = img.dimensions();
                        let cimg = ColorImage::from_rgba_unmultiplied([iw as usize, ih as usize], img.as_raw());
                        let tex = ctx.load_texture(
                            format!("overlay_{}", path.to_string_lossy()), cimg, tex_opts,
                        );
                        let id = tex.id();
                        self.overlay_tex.insert(path.clone(), tex);
                        Some(id)
                    }
                });
                if let Some(id) = tex_id {
                    p.image(id, r, uv, Color32::from_rgba_unmultiplied(255, 255, 255, alpha));
                } else {
                    p.rect_filled(r, 2.0, Color32::from_rgba_unmultiplied(40, 40, 40, alpha));
                    p.text(r.center(), egui::Align2::CENTER_CENTER, &o.nm, FontId::proportional(11.0), SUB);
                }
                p.rect_stroke(r, 2.0, Stroke::new(1.0_f32, ACCENT));
            }
        }
        if resp.dragged() || resp.clicked() || resp.drag_released() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) * self.total();
                self.playhead = t;
                self.seek_request = Some(t);
                self.dirty = true;
            }
        }
        // slim preview toolbar
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(fmt_tc_frames(self.playhead)).color(INK).font(FontId::monospace(11.0)));
            ui.label(RichText::new(format!("frame {}", (self.playhead * self.fps.max(1.0)).round() as i64)).color(SUB).size(10.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(format!(
                    "eng·{} · {}×{} · {:.0}p · {:.2}×",
                    media_engine::engine_label(), self.source_w, self.source_h, self.fps, self.speed
                )).color(SUB).size(10.0));
            });
        });
        rect
    }

    fn render_inspector(&mut self, ui: &mut Ui) {
        let before = self.kf_pack();
        ui.add_space(6.0);
        ui.label(RichText::new("Properties").color(INK).size(13.0));
        ui.add_space(4.0);
        let groups: [(&str, usize); 6] = [("Basic", 0), ("Color", 1), ("Effects", 2), ("Distort", 3), ("Mask & text", 4), ("Tracks", 5)];
        for (title, gi) in groups {
            CollapsingHeader::new(RichText::new(title).color(INK).size(12.0)).default_open(self.grp_open[gi]).show(ui, |ui| {
                match gi {
                    0 => {
                        slider_row_f(ui, "Speed", &mut self.speed, 0.25, 3.0, 0.01, |v| format!("{:.2}×", v));
                        self.vtr_type = select_row(ui, "Transition (clip start & end)", self.vtr_type, &[(TransType::None, "None"), (TransType::Fade, "Fade through black"), (TransType::Wipe, "Wipe"), (TransType::Slide, "Slide"), (TransType::Zoom, "Zoom")]);
                        slider_row_f(ui, "Duration", &mut self.vtr_dur, 0.1, 3.0, 0.1, |v| format!("{:.1} s", v));
                        self.look = select_row(ui, "Look", self.look, &[(LookType::None, "None"), (LookType::Grayscale, "Grayscale"), (LookType::Sepia, "Sepia"), (LookType::Invert, "Invert")]);
                        self.mirror = select_row(ui, "Mirror", self.mirror, &[(MirrorType::None, "None"), (MirrorType::Left, "Left → right"), (MirrorType::Right, "Right → left"), (MirrorType::Top, "Top → bottom"), (MirrorType::Bottom, "Bottom → top")]);
                        ui.checkbox(&mut self.preview_transform_mode, "Show transform handles in preview");
                    }
                    1 => {
                        slider_row_f(ui, "Brightness", &mut self.vb, 0.0, 200.0, 1.0, |v| format!("{}%", v as i32));
                        slider_row_f(ui, "Contrast", &mut self.vc, 0.0, 200.0, 1.0, |v| format!("{}%", v as i32));
                        slider_row_f(ui, "Saturation", &mut self.vs, 0.0, 300.0, 1.0, |v| format!("{}%", v as i32));
                        slider_row_f(ui, "Hue", &mut self.vh, 0.0, 360.0, 1.0, |v| format!("{}°", v as i32));
                        ui.label(RichText::new("Color curves").color(SUB).size(11.0));
                        self.vcv_ch = select_row(ui, "Channel", self.vcv_ch, &[(CurveCh::Rgb, "RGB"), (CurveCh::R, "Red"), (CurveCh::G, "Green"), (CurveCh::B, "Blue")]);
                        self.render_curves(ui);
                        ui.label(RichText::new("Color wheels — shadows · midtones · highlights").color(SUB).size(11.0));
                        ui.horizontal(|ui| { self.render_wheel(ui, "sh", 0); self.render_wheel(ui, "mid", 1); self.render_wheel(ui, "hi", 2); });
                        ui.label(RichText::new("HSL by color").color(SUB).size(11.0));
                        self.vhsl_b = select_row_u8(ui, "Color", self.vhsl_b, &["Red", "Orange", "Yellow", "Green", "Cyan", "Blue", "Purple", "Magenta"]);
                        slider_row_f(ui, "Hue", &mut self.vhsl_h, -100.0, 100.0, 1.0, |v| format!("{:+}", v as i32));
                        slider_row_f(ui, "Saturation", &mut self.vhsl_s, -100.0, 100.0, 1.0, |v| format!("{:+}", v as i32));
                        slider_row_f(ui, "Luminance", &mut self.vhsl_l, -100.0, 100.0, 1.0, |v| format!("{:+}", v as i32));
                    }
                    2 => {
                        slider_row_f(ui, "Blur", &mut self.vbl, 0.0, 12.0, 0.5, |v| format!("{}px", v as i32));
                        slider_row_f(ui, "Vignette", &mut self.vv, 0.0, 1.0, 0.05, |v| format!("{:.2}", v));
                        slider_row_f(ui, "Film grain", &mut self.vgr, 0.0, 1.0, 0.05, |v| format!("{:.2}", v));
                        slider_row_f(ui, "Scanlines", &mut self.vsc, 0.0, 1.0, 0.05, |v| format!("{:.2}", v));
                        slider_row_f(ui, "Pixelate (px)", &mut self.vpx, 0.0, 60.0, 1.0, |v| format!("{:.0}", v));
                        slider_row_f(ui, "Glitch", &mut self.vgl, 0.0, 1.0, 0.05, |v| format!("{:.2}", v));
                        slider_row_f(ui, "Posterize", &mut self.vpo, 0.0, 1.0, 0.05, |v| format!("{:.2}", v));
                        slider_row_f(ui, "RGB split (px)", &mut self.vrgb, 0.0, 40.0, 1.0, |v| format!("{:.0}", v));
                        slider_row_f(ui, "Kaleidoscope", &mut self.vkl, 0.0, 12.0, 1.0, |v| format!("{:.0}", v));
                        slider_row_f(ui, "Camera shake", &mut self.vsk, 0.0, 40.0, 1.0, |v| format!("{:.0}", v));
                        slider_row_f(ui, "Motion trail", &mut self.vtl, 0.0, 0.9, 0.05, |v| format!("{:.2}", v));
                    }
                    3 => {
                        slider_row_f(ui, "Wave — horizontal", &mut self.vwh, 0.0, 60.0, 0.5, |v| format!("{:.2}", v));
                        slider_row_f(ui, "Wave — vertical", &mut self.vwv, 0.0, 60.0, 0.5, |v| format!("{:.2}", v));
                        slider_row_f(ui, "3D tilt", &mut self.vtilt, -1.0, 1.0, 0.01, |v| format!("{:.2}", v));
                        slider_row_f(ui, "3D yaw", &mut self.vyaw, -1.0, 1.0, 0.01, |v| format!("{:.2}", v));
                        slider_row_f(ui, "Cube rotation (°)", &mut self.vcube, 0.0, 90.0, 0.5, |v| format!("{:.2}", v));
                    }
                    4 => {
                        self.vmk_type = select_row(ui, "Mask", self.vmk_type, &[(MaskType::None, "None"), (MaskType::Ellipse, "Ellipse"), (MaskType::Rect, "Rectangle"), (MaskType::Linear, "Linear gradient")]);
                        if self.vmk_type != MaskType::None {
                            slider_row_f(ui, "Center X %", &mut self.vmk_x, 0.0, 100.0, 1.0, |v| format!("{}", v as i32));
                            slider_row_f(ui, "Center Y %", &mut self.vmk_y, 0.0, 100.0, 1.0, |v| format!("{}", v as i32));
                            slider_row_f(ui, "Size %", &mut self.vmk_s, 5.0, 100.0, 1.0, |v| format!("{}", v as i32));
                            slider_row_f(ui, "Feather (px)", &mut self.vmk_f, 0.0, 80.0, 1.0, |v| format!("{}", v as i32));
                            ui.checkbox(&mut self.vmk_i, "Invert mask");
                        }
                        ui.label(RichText::new("Text overlay").color(SUB).size(11.0));
                        ui.text_edit_singleline(&mut self.vtx_text);
                        slider_row_f(ui, "Size (% of height)", &mut self.vtx_size, 3.0, 25.0, 1.0, |v| format!("{}", v as i32));
                        slider_row_f(ui, "X %", &mut self.vtx_x, 0.0, 100.0, 1.0, |v| format!("{}", v as i32));
                        slider_row_f(ui, "Y %", &mut self.vtx_y, 0.0, 100.0, 1.0, |v| format!("{}", v as i32));
                        slider_row_f(ui, "Start (s)", &mut self.vtx_a, 0.0, 60.0, 0.1, |v| format!("{:.1}", v));
                        slider_row_f(ui, "End (s, 0 = to end)", &mut self.vtx_b, 0.0, 60.0, 0.1, |v| format!("{:.1}", v));
                        ui.horizontal(|ui| { ui.label(RichText::new("Color").color(SUB).size(11.0)); ui.color_edit_button_srgba(&mut self.vtx_col); });
                        if ui.add(egui::Button::new("+ Add text").fill(TBBG).rounding(Rounding::same(20.0))).clicked() {
                            self.texts.push(TextOverlay { uid: fresh_clip_uid(), t: if self.vtx_text.is_empty() { "Text".into() } else { self.vtx_text.clone() }, size: self.vtx_size, x: self.vtx_x, y: self.vtx_y, a: self.vtx_a, b: if self.vtx_b == 0.0 { 1e9 } else { self.vtx_b }, col: self.vtx_col });
                        }
                        for i in (0..self.texts.len()).rev() {
                            ui.horizontal(|ui| {
                                let t = &self.texts[i];
                                ui.label(RichText::new(format!("“{}” {:.1}s–{}", t.t, t.a, if t.b > 1e8 { "end".to_string() } else { format!("{:.1}s", t.b) })).color(SUB).size(11.0));
                                if ui.small_button("✕").clicked() { self.texts.remove(i); }
                            });
                        }
                    }
                    5 => {
                        ui.label(RichText::new("Overlay track (V2) — picture-in-picture").color(SUB).size(11.0));
                        slider_row_f(ui, "Starts at (s)", &mut self.vov_t, 0.0, 60.0, 0.1, |v| format!("{:.1}", v));
                        slider_row_f(ui, "X %", &mut self.vov_x, 0.0, 100.0, 1.0, |v| format!("{}", v as i32));
                        slider_row_f(ui, "Y %", &mut self.vov_y, 0.0, 100.0, 1.0, |v| format!("{}", v as i32));
                        slider_row_f(ui, "Size %", &mut self.vov_s, 5.0, 100.0, 1.0, |v| format!("{}", v as i32));
                        slider_row_f(ui, "Opacity %", &mut self.vov_o, 0.0, 100.0, 1.0, |v| format!("{}", v as i32));
                        if ui.small_button("+ Add overlay").clicked() {
                            self.overlays.push(OverlayClip { uid: fresh_clip_uid(), nm: format!("overlay_{}", self.overlays.len() + 1), dur: 5.0, start: self.vov_t, x: self.vov_x, y: self.vov_y, s: self.vov_s, o: self.vov_o, path: self.media_path.clone() });
                        }
                        ui.label(RichText::new("Music track (A2)").color(SUB).size(11.0));
                        slider_row_f(ui, "Starts at (s)", &mut self.vmu_t, 0.0, 60.0, 0.1, |v| format!("{:.1}", v));
                        slider_row_f(ui, "Volume %", &mut self.vmu_v, 0.0, 100.0, 1.0, |v| format!("{}", v as i32));
                        if ui.small_button("+ Add music").clicked() {
                            self.music.push(MusicClip {
                                uid: fresh_clip_uid(),
                                nm: format!("music_{}", self.music.len() + 1), dur: 8.0,
                                track: 0, start: self.vmu_t, vol: self.vmu_v, path: self.media_path.clone(),
                            });
                        }
                    }
                    _ => {}
                }
            });
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new("◆ Add keyframe").fill(TBBG).rounding(Rounding::same(4.0))).clicked() { self.upsert_kf(self.kf_pack()); }
            if ui.add(egui::Button::new("Clear keyframes").fill(TBBG).rounding(Rounding::same(4.0))).clicked() { self.keyframes.clear(); }
        });
        ui.label(RichText::new(if self.keyframes.is_empty() { "No keyframes — sliders are constant.".into() } else { format!("{} keyframe(s): {}", self.keyframes.len(), self.keyframes.iter().map(|k| format!("{:.1}s", k.t)).collect::<Vec<_>>().join(", ")) }).color(SUB).size(10.0));
        ui.add_space(8.0);
        CollapsingHeader::new(RichText::new("BananaV ACES 1.3 · LGG · Scopes · Generators").color(ACCENT).size(12.0))
            .default_open(true)
            .show(ui, |ui| {
                ui.checkbox(&mut self.aces_enabled, "Enable ACES 1.3 (32-bit float RRT + ODT)");
                self.aces_view = select_row(
                    ui,
                    "ACES ODT View",
                    self.aces_view,
                    &[
                        (bananav_suite::AcesViewTransform::SrgbRec709, "ACES 1.3 sRGB / Rec.709"),
                        (bananav_suite::AcesViewTransform::Rec2020Pq, "ACES 1.3 Rec.2020 PQ (HDR10)"),
                        (bananav_suite::AcesViewTransform::DciP3, "ACES 1.3 DCI-P3 D65"),
                        (bananav_suite::AcesViewTransform::AcesCctLog, "ACEScct Working Log"),
                        (bananav_suite::AcesViewTransform::LinearAces2065, "ACES2065-1 Scene Linear"),
                    ],
                );
                ui.add_space(4.0);
                ui.label(RichText::new("ASC-CDL Lift / Gamma / Gain / Offset (Master)").color(SUB).size(11.0));
                let mut m_lift = self.lgg_lift[0];
                let mut m_gamma = self.lgg_gamma[0];
                let mut m_gain = self.lgg_gain[0];
                let mut m_off = self.lgg_offset[0];
                slider_row_f(ui, "Lift (Shadows)", &mut m_lift, -0.5, 0.5, 0.01, |v| format!("{:+.2}", v));
                slider_row_f(ui, "Gamma (Midtones)", &mut m_gamma, 0.2, 2.5, 0.01, |v| format!("{:.2}", v));
                slider_row_f(ui, "Gain (Highlights)", &mut m_gain, 0.2, 2.5, 0.01, |v| format!("{:.2}", v));
                slider_row_f(ui, "Offset", &mut m_off, -0.5, 0.5, 0.01, |v| format!("{:+.2}", v));
                self.lgg_lift = [m_lift; 3];
                self.lgg_gamma = [m_gamma; 3];
                self.lgg_gain = [m_gain; 3];
                self.lgg_offset = [m_off; 3];

                ui.add_space(4.0);
                self.generator_kind = select_row(
                    ui,
                    "Generator Source",
                    self.generator_kind,
                    &[
                        (bananav_suite::GeneratorKind::None, "Source Media (Off)"),
                        (bananav_suite::GeneratorKind::SmpteBars, "SMPTE HD Color Bars"),
                        (bananav_suite::GeneratorKind::Checkerboard, "Checkerboard"),
                        (bananav_suite::GeneratorKind::ColorGradient, "Color Gradient"),
                        (bananav_suite::GeneratorKind::SolidColor, "Solid Color Matte"),
                        (bananav_suite::GeneratorKind::PerlinNoiseTexture, "Perlin Noise Clouds"),
                        (bananav_suite::GeneratorKind::TestGrid, "Calibration Grid"),
                    ],
                );

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.show_scopes, "Live Video Scope");
                    self.scope_kind = select_row(
                        ui,
                        "",
                        self.scope_kind,
                        &[
                            (bananav_suite::VideoScopeKind::Histogram, "Histogram"),
                            (bananav_suite::VideoScopeKind::WaveformLuma, "Waveform"),
                            (bananav_suite::VideoScopeKind::RgbParade, "RGB Parade"),
                            (bananav_suite::VideoScopeKind::Vectorscope, "Vectorscope"),
                        ],
                    );
                });
                if self.show_scopes && !self.last_rgba.is_empty() {
                    let (srect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 82.0), Sense::hover());
                    let sp = ui.painter();
                    sp.rect_filled(srect, 3.0, BG);
                    sp.rect_stroke(srect, 3.0, Stroke::new(1.0_f32, LINE));
                    let fw = self.fw.max(1);
                    let fh = self.fh.max(1);
                    let step_x = (fw / 180).max(1);
                    let step_y = (fh / 80).max(1);
                    let inner = srect.shrink(3.0);
                    match self.scope_kind {
                        bananav_suite::VideoScopeKind::Histogram => {
                            let mut bins = [0u32; 64];
                            for px in self.last_rgba.chunks_exact(4) {
                                let luma = ((px[0] as u32 * 299 + px[1] as u32 * 587 + px[2] as u32 * 114) / 1000).min(255);
                                bins[(luma as usize * 63) / 255] += 1;
                            }
                            let max_b = (*bins.iter().max().unwrap_or(&1)).max(1) as f32;
                            let bw = inner.width() / 64.0;
                            for (bi, &cnt) in bins.iter().enumerate() {
                                let h = (cnt as f32 / max_b) * (inner.height() - 2.0);
                                let x = inner.left() + bi as f32 * bw;
                                sp.rect_filled(
                                    Rect::from_min_max(pos2(x, inner.bottom() - h), pos2(x + bw * 0.85, inner.bottom())),
                                    0.0,
                                    ACCENT,
                                );
                            }
                        }
                        bananav_suite::VideoScopeKind::WaveformLuma => {
                            for y in (0..fh).step_by(step_y) {
                                for x in (0..fw).step_by(step_x) {
                                    let i = ((y * fw + x) * 4) as usize;
                                    if i + 2 >= self.last_rgba.len() { continue; }
                                    let p = &self.last_rgba[i..i + 3];
                                    let luma = (p[0] as f32 * 0.299 + p[1] as f32 * 0.587 + p[2] as f32 * 0.114) / 255.0;
                                    let at = pos2(
                                        inner.left() + x as f32 / fw as f32 * inner.width(),
                                        inner.bottom() - luma * inner.height(),
                                    );
                                    sp.circle_filled(at, 0.8, Color32::from_rgba_unmultiplied(120, 235, 200, 55));
                                }
                            }
                        }
                        bananav_suite::VideoScopeKind::RgbParade => {
                            for (ci, col) in [
                                Color32::from_rgba_unmultiplied(255, 70, 70, 60),
                                Color32::from_rgba_unmultiplied(70, 235, 90, 60),
                                Color32::from_rgba_unmultiplied(80, 140, 255, 60),
                            ].into_iter().enumerate() {
                                let third = inner.width() / 3.0;
                                sp.line_segment(
                                    [pos2(inner.left() + (ci + 1) as f32 * third, inner.top()), pos2(inner.left() + (ci + 1) as f32 * third, inner.bottom())],
                                    Stroke::new(1.0_f32, LINE),
                                );
                                for y in (0..fh).step_by(step_y) {
                                    for x in (0..fw).step_by(step_x) {
                                        let i = ((y * fw + x) * 4) as usize;
                                        if i + ci >= self.last_rgba.len() { continue; }
                                        let v = self.last_rgba[i + ci] as f32 / 255.0;
                                        let at = pos2(
                                            inner.left() + ci as f32 * third + x as f32 / fw as f32 * third,
                                            inner.bottom() - v * inner.height(),
                                        );
                                        sp.circle_filled(at, 0.7, col);
                                    }
                                }
                            }
                        }
                        bananav_suite::VideoScopeKind::Vectorscope => {
                            let c = inner.center();
                            let rad = inner.height() * 0.46;
                            for r in [0.33_f32, 0.66, 1.0] {
                                sp.circle_stroke(c, rad * r, Stroke::new(1.0_f32, Color32::from_rgb(52, 62, 58)));
                            }
                            sp.line_segment([pos2(c.x - rad, c.y), pos2(c.x + rad, c.y)], Stroke::new(1.0_f32, Color32::from_rgb(52, 62, 58)));
                            sp.line_segment([pos2(c.x, c.y - rad), pos2(c.x, c.y + rad)], Stroke::new(1.0_f32, Color32::from_rgb(52, 62, 58)));
                            for y in (0..fh).step_by(step_y) {
                                for x in (0..fw).step_by(step_x) {
                                    let i = ((y * fw + x) * 4) as usize;
                                    if i + 2 >= self.last_rgba.len() { continue; }
                                    let r = self.last_rgba[i] as f32 / 255.0 - 0.5;
                                    let g = self.last_rgba[i + 1] as f32 / 255.0 - 0.5;
                                    let b = self.last_rgba[i + 2] as f32 / 255.0 - 0.5;
                                    let cb = (b - r) * 1.4;
                                    let cr = (r - g) * 1.4;
                                    let at = c + vec2(cb * rad, -cr * rad);
                                    if inner.contains(at) {
                                        sp.circle_filled(at, 0.8, Color32::from_rgba_unmultiplied(90, 240, 170, 55));
                                    }
                                }
                            }
                        }
                    }
                }
            });
        let after = self.kf_pack();
        if before != after {
            self.dirty = true;
            if !self.keyframes.is_empty() { self.upsert_kf(after); }
        }
    }

    fn render_curves(&mut self, ui: &mut Ui) {
        let s = 170.0;
        let (rect, _r) = ui.allocate_exact_size(vec2(s, s), Sense::click_and_drag());
        let p = ui.painter();
        p.rect_filled(rect, 2.0, BG);
        let g = Stroke::new(1.0_f32, Color32::from_rgb(70, 70, 70));
        for i in 1..4 {
            let t = i as f32 / 4.0 * s;
            p.line_segment([pos2(rect.left() + t, rect.top()), pos2(rect.left() + t, rect.bottom())], g);
            p.line_segment([pos2(rect.left(), rect.top() + t), pos2(rect.right(), rect.top() + t)], g);
        }
        let ch = self.vcv_ch as usize;
        let pts = self.curves[ch];
        let col = match self.vcv_ch { CurveCh::Rgb => Color32::from_rgb(140, 140, 140), CurveCh::R => Color32::from_rgb(228, 68, 68), CurveCh::G => Color32::from_rgb(75, 200, 75), CurveCh::B => Color32::from_rgb(68, 136, 255) };
        let [p0, p1] = [pts.p0, pts.p1];
        let mut last = pos2(rect.left(), rect.bottom());
        for i in 1..=40 {
            let t = i as f32 / 40.0; let u = 1.0 - t;
            let x = 3.0 * u * u * t * p0[0] + 3.0 * u * t * t * p1[0] + t * t * t;
            let y = 3.0 * u * u * t * p0[1] + 3.0 * u * t * t * p1[1] + t * t * t;
            let cur = pos2(rect.left() + x * s, rect.bottom() - y * s);
            p.line_segment([last, cur], Stroke::new(2.0_f32, col));
            last = cur;
        }
        for (i, q) in [p0, p1].iter().enumerate() {
            let cp = pos2(rect.left() + q[0] * s, rect.bottom() - q[1] * s);
            p.circle_filled(cp, 5.0, ACCENT2);
            let hr = Rect::from_center_size(cp, vec2(16.0, 16.0));
            let r = ui.interact(hr, Id::new(("curve", ch, i)), Sense::drag());
            if r.dragged() {
                if let Some(pos) = r.interact_pointer_pos() {
                    let nx = ((pos.x - rect.left()) / s).clamp(0.0, 1.0);
                    let ny = (1.0 - (pos.y - rect.top()) / s).clamp(0.0, 1.0);
                    if i == 0 { self.curves[ch].p0 = [nx, ny]; } else { self.curves[ch].p1 = [nx, ny]; }
                }
            }
        }
        if ui.small_button("Reset channel").clicked() { self.curves[ch] = CurvePts::default(); }
    }

    fn render_wheel(&mut self, ui: &mut Ui, id: &str, idx: usize) {
        let s = 84.0;
        let (rect, _r) = ui.allocate_exact_size(vec2(s, s), Sense::drag());
        let val = match idx { 0 => &mut self.wheel_sh, 1 => &mut self.wheel_mid, _ => &mut self.wheel_hi };
        let p = ui.painter();
        let cx = rect.center().x; let cy = rect.center().y;
        let r = s / 2.0 - 2.0;
        p.circle_filled(rect.center(), r, Color32::from_rgb(40, 40, 40));
        p.circle_stroke(rect.center(), r, Stroke::new(1.0_f32, LINE));
        p.line_segment([pos2(rect.left() + 2.0, cy), pos2(rect.right() - 2.0, cy)], Stroke::new(1.0_f32, Color32::from_rgb(60, 60, 60)));
        p.line_segment([pos2(cx, rect.top() + 2.0), pos2(cx, rect.bottom() - 2.0)], Stroke::new(1.0_f32, Color32::from_rgb(60, 60, 60)));
        let px = cx + val[0] * r; let py = cy + val[1] * r;
        p.circle_filled(pos2(px, py), 5.0, ACCENT2);
        let hr = Rect::from_center_size(pos2(px, py), vec2(20.0, 20.0));
        let resp = ui.interact(hr, Id::new(("wheel", id)), Sense::drag());
        if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let dx = (pos.x - cx) / r; let dy = (pos.y - cy) / r;
                let m = (dx * dx + dy * dy).sqrt();
                if m > 1.0 { val[0] = dx / m; val[1] = dy / m; } else { val[0] = dx; val[1] = dy; }
            }
        }
        if resp.double_clicked() { val[0] = 0.0; val[1] = 0.0; }
    }
}

fn select_row_u8(ui: &mut Ui, label: &str, current: u8, opts: &[&str]) -> u8 {
    let mut sel = current;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(SUB).size(12.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let txt = opts.get(sel as usize).copied().unwrap_or("");
            ComboBox::from_id_source(label).selected_text(txt).show_ui(ui, |ui| {
                for (i, o) in opts.iter().enumerate() { ui.selectable_value(&mut sel, i as u8, *o); }
            });
        });
    });
    sel
}

// ===========================================================================
// Shared UI helpers + timecode + main
// ===========================================================================
fn parse_timecode(input: &str, fps: f32) -> Option<f32> {
    let input = input.trim();
    if input.is_empty() { return None; }
    if let Ok(seconds) = input.parse::<f32>() { return seconds.is_finite().then(|| seconds.max(0.0)); }
    let parts: Vec<&str> = input.split(':').map(str::trim).collect();
    if parts.iter().any(|p| p.is_empty()) { return None; }
    let nums: Vec<f64> = parts.iter().filter_map(|p| p.parse::<f64>().ok()).collect();
    if nums.len() != parts.len() { return None; }
    let fps = fps.max(1.0) as f64;
    let t = match nums.len() {
        1 => nums[0],
        2 => nums[0] * 60.0 + nums[1],
        3 => nums[0] * 3600.0 + nums[1] * 60.0 + nums[2],
        4 => nums[0] * 3600.0 + nums[1] * 60.0 + nums[2] + nums[3] / fps,
        _ => return None,
    };
    (t.is_finite() && t >= 0.0).then_some(t as f32)
}

fn fmt_tc(sec: f32) -> String {
    let m = (sec / 60.0) as u32;
    let s = sec - (m as f32) * 60.0;
    format!("{:02}:{:05.2}", m, s)
}

fn fmt_tc_frames(sec: f32) -> String {
    let fps = 30.0_f32;
    let total = sec.max(0.0);
    let h = (total / 3600.0) as u32;
    let m = ((total - h as f32 * 3600.0) / 60.0) as u32;
    let s = (total - h as f32 * 3600.0 - m as f32 * 60.0) as u32;
    let ff = ((total - total.floor()) * fps) as u32;
    format!("{:02}:{:02}:{:02}:{:02}", h, m, s, ff)
}

fn slider_row_f(ui: &mut Ui, label: &str, v: &mut f32, min: f32, max: f32, step: f64, fmt: impl Fn(f32) -> String) {
    let val = fmt(*v);
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(SUB).size(12.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| { ui.label(RichText::new(val).color(INK).font(FontId::monospace(11.0))); });
    });
    ui.add(Slider::new(v, min..=max).step_by(step).show_value(false));
}

fn slider_row_i(ui: &mut Ui, label: &str, v: &mut i32, min: i32, max: i32, fmt: impl Fn(i32) -> String) {
    let val = fmt(*v);
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(SUB).size(12.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| { ui.label(RichText::new(val).color(INK).font(FontId::monospace(11.0))); });
    });
    ui.add(Slider::new(v, min..=max).show_value(false));
}

fn select_row<T: PartialEq + Copy>(ui: &mut Ui, label: &str, current: T, opts: &[(T, &'static str)]) -> T {
    let mut sel = current;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(SUB).size(12.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let txt = opts.iter().find(|(t, _)| *t == sel).map(|(_, s)| *s).unwrap_or("");
            ComboBox::from_id_source(label).selected_text(txt).show_ui(ui, |ui| { for (t, s) in opts { ui.selectable_value(&mut sel, *t, *s); } });
        });
    });
    sel
}

fn select_row_owned<T: PartialEq + Copy>(ui: &mut Ui, label: &str, current: T, opts: &[(T, String)]) -> T {
    let mut sel = current;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(SUB).size(12.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let txt = opts.iter().find(|(t, _)| *t == sel).map(|(_, s)| s.as_str()).unwrap_or("");
            ComboBox::from_id_source(label).selected_text(txt).show_ui(ui, |ui| { for (t, s) in opts { ui.selectable_value(&mut sel, *t, s.as_str()); } });
        });
    });
    sel
}

fn select_row_u32(ui: &mut Ui, label: &str, current: u32, opts: &[u32]) -> u32 {
    let mut sel = current;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(SUB).size(12.0));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ComboBox::from_id_source(label).selected_text(format!("{}", sel)).show_ui(ui, |ui| { for &o in opts { ui.selectable_value(&mut sel, o, format!("{}", o)); } });
        });
    });
    sel
}

fn main() -> eframe::Result<()> {
    // `grain elastique <shifts...> INPUT [-o OUT]` → headless pitch/time render
    // using the real audio_dsp engine + media_engine decode/export. Otherwise GUI.
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "elastique" {
        let code = elastique_cli::run_elastique_cli(&args[1..]);
        std::process::exit(code);
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1180.0, 760.0]).with_min_inner_size([640.0, 480.0]),
        // 120 FPS target. egui still uses hardware acceleration; no forced vsync cap.
        vsync: false,
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        ..Default::default()
    };
    eframe::run_native("Grain — audio · video lab", opts, Box::new(|_cc| Ok(Box::new(GrainApp::default()))))
}
