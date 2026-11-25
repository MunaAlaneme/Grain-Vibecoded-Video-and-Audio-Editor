use crate::audio_dsp::{process_stereo, AudioExportFormat, AudioLabState};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerFormat {
    Mp4,
    Mov,
    Mkv,
    Webm,
    Avi,
    FramesZip,
}

impl ContainerFormat {
    pub const ALL: &'static [ContainerFormat] = &[
        ContainerFormat::Mp4,
        ContainerFormat::Mov,
        ContainerFormat::Mkv,
        ContainerFormat::Webm,
        ContainerFormat::Avi,
        ContainerFormat::FramesZip,
    ];

    pub fn ext(&self) -> &'static str {
        match self {
            ContainerFormat::Mp4 => "mp4",
            ContainerFormat::Mov => "mov",
            ContainerFormat::Mkv => "mkv",
            ContainerFormat::Webm => "webm",
            ContainerFormat::Avi => "avi",
            ContainerFormat::FramesZip => "zip",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ContainerFormat::Mp4 => "MP4 (.mp4)",
            ContainerFormat::Mov => "QuickTime MOV (.mov)",
            ContainerFormat::Mkv => "Matroska (.mkv)",
            ContainerFormat::Webm => "WebM (.webm)",
            ContainerFormat::Avi => "AVI (.avi)",
            ContainerFormat::FramesZip => "Image Frames (JPEG sequence)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    Hevc,
    Av1,
    Vp9,
    Mjpeg,
}

impl VideoCodec {
    pub const ALL: &'static [VideoCodec] = &[
        VideoCodec::H264,
        VideoCodec::Hevc,
        VideoCodec::Av1,
        VideoCodec::Vp9,
        VideoCodec::Mjpeg,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "H.264 / AVC",
            VideoCodec::Hevc => "H.265 / HEVC",
            VideoCodec::Av1 => "AV1",
            VideoCodec::Vp9 => "VP9 (WebM, MKV)",
            VideoCodec::Mjpeg => "MJPEG (AVI)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoAudioCodec {
    Aac,
    Opus,
    Pcm,
    None,
}

impl VideoAudioCodec {
    pub const ALL: &'static [VideoAudioCodec] = &[
        VideoAudioCodec::Aac,
        VideoAudioCodec::Opus,
        VideoAudioCodec::Pcm,
        VideoAudioCodec::None,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            VideoAudioCodec::Aac => "AAC-LC",
            VideoAudioCodec::Opus => "Opus",
            VideoAudioCodec::Pcm => "PCM (MKV, AVI)",
            VideoAudioCodec::None => "No audio",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionPreset {
    Source,
    P480,
    P720,
    P1080,
    P1440,
    P2160,
    P4320,
}

impl ResolutionPreset {
    pub const ALL: &'static [ResolutionPreset] = &[
        ResolutionPreset::Source,
        ResolutionPreset::P480,
        ResolutionPreset::P720,
        ResolutionPreset::P1080,
        ResolutionPreset::P1440,
        ResolutionPreset::P2160,
        ResolutionPreset::P4320,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ResolutionPreset::Source => "Source (Adapted)",
            ResolutionPreset::P480 => "480p",
            ResolutionPreset::P720 => "720p",
            ResolutionPreset::P1080 => "1080p Full HD",
            ResolutionPreset::P1440 => "1440p 2K",
            ResolutionPreset::P2160 => "2160p 4K",
            ResolutionPreset::P4320 => "4320p 8K",
        }
    }

    pub fn height(&self) -> Option<u32> {
        match self {
            ResolutionPreset::Source => None,
            ResolutionPreset::P480 => Some(480),
            ResolutionPreset::P720 => Some(720),
            ResolutionPreset::P1080 => Some(1080),
            ResolutionPreset::P1440 => Some(1440),
            ResolutionPreset::P2160 => Some(2160),
            ResolutionPreset::P4320 => Some(4320),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AspectRatioPreset {
    Original,
    R16x9,
    R9x16,
    R1x1,
    R4x5,
    R4x3,
    R21x9,
}

impl AspectRatioPreset {
    pub const ALL: &'static [AspectRatioPreset] = &[
        AspectRatioPreset::Original,
        AspectRatioPreset::R16x9,
        AspectRatioPreset::R9x16,
        AspectRatioPreset::R1x1,
        AspectRatioPreset::R4x5,
        AspectRatioPreset::R4x3,
        AspectRatioPreset::R21x9,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            AspectRatioPreset::Original => "Original",
            AspectRatioPreset::R16x9 => "16:9",
            AspectRatioPreset::R9x16 => "9:16 (vertical)",
            AspectRatioPreset::R1x1 => "1:1 (square)",
            AspectRatioPreset::R4x5 => "4:5",
            AspectRatioPreset::R4x3 => "4:3",
            AspectRatioPreset::R21x9 => "21:9",
        }
    }

    pub fn ratio(&self) -> Option<f32> {
        match self {
            AspectRatioPreset::Original => None,
            AspectRatioPreset::R16x9 => Some(16.0 / 9.0),
            AspectRatioPreset::R9x16 => Some(9.0 / 16.0),
            AspectRatioPreset::R1x1 => Some(1.0),
            AspectRatioPreset::R4x5 => Some(4.0 / 5.0),
            AspectRatioPreset::R4x3 => Some(4.0 / 3.0),
            AspectRatioPreset::R21x9 => Some(21.0 / 9.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramingMode {
    FitLetterbox,
    FillCrop,
}

impl FramingMode {
    pub fn label(&self) -> &'static str {
        match self {
            FramingMode::FitLetterbox => "Fit (letterbox)",
            FramingMode::FillCrop => "Fill (crop)",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectSettings {
    pub name: String,
    pub color_space: String,
    pub aspect: AspectRatioPreset,
    pub framing: FramingMode,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            name: "Untitled".into(),
            color_space: "Rec. 709 SDR".into(),
            aspect: AspectRatioPreset::Original,
            framing: FramingMode::FitLetterbox,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingCurve {
    Linear,
    InQuad,
    OutQuad,
    InOutQuad,
    InCubic,
    OutCubic,
    InOutCubic,
    InSine,
    OutSine,
    InOutSine,
    InExpo,
    OutExpo,
    Hold,
}

impl EasingCurve {
    pub const ALL: &'static [EasingCurve] = &[
        EasingCurve::Linear,
        EasingCurve::InQuad,
        EasingCurve::OutQuad,
        EasingCurve::InOutQuad,
        EasingCurve::InCubic,
        EasingCurve::OutCubic,
        EasingCurve::InOutCubic,
        EasingCurve::InSine,
        EasingCurve::OutSine,
        EasingCurve::InOutSine,
        EasingCurve::InExpo,
        EasingCurve::OutExpo,
        EasingCurve::Hold,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            EasingCurve::Linear => "Linear",
            EasingCurve::InQuad => "Ease in (quad)",
            EasingCurve::OutQuad => "Ease out (quad)",
            EasingCurve::InOutQuad => "Ease in-out (quad)",
            EasingCurve::InCubic => "Ease in (cubic)",
            EasingCurve::OutCubic => "Ease out (cubic)",
            EasingCurve::InOutCubic => "Ease in-out (cubic)",
            EasingCurve::InSine => "Sine in",
            EasingCurve::OutSine => "Sine out",
            EasingCurve::InOutSine => "Sine in-out",
            EasingCurve::InExpo => "Expo in",
            EasingCurve::OutExpo => "Expo out",
            EasingCurve::Hold => "Hold (step)",
        }
    }

    pub fn eval(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            EasingCurve::Linear => t,
            EasingCurve::InQuad => t * t,
            EasingCurve::OutQuad => t * (2.0 - t),
            EasingCurve::InOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            EasingCurve::InCubic => t * t * t,
            EasingCurve::OutCubic => {
                let u = t - 1.0;
                u * u * u + 1.0
            }
            EasingCurve::InOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    (t - 1.0) * (2.0 * t - 2.0) * (2.0 * t - 2.0) + 1.0
                }
            }
            EasingCurve::InSine => 1.0 - (t * std::f32::consts::FRAC_PI_2).cos(),
            EasingCurve::OutSine => (t * std::f32::consts::FRAC_PI_2).sin(),
            EasingCurve::InOutSine => -((std::f32::consts::PI * t).cos() - 1.0) * 0.5,
            EasingCurve::InExpo => {
                if t == 0.0 {
                    0.0
                } else {
                    2.0_f32.powf(10.0 * t - 10.0)
                }
            }
            EasingCurve::OutExpo => {
                if t == 1.0 {
                    1.0
                } else {
                    1.0 - 2.0_f32.powf(-10.0 * t)
                }
            }
            EasingCurve::Hold => 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VideoKeyframe {
    pub time_sec: f64,
    pub easing: EasingCurve,
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub hue: f32,
    pub blur: f32,
    pub zoom: f32,
    pub rotate: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoLook {
    None,
    Grayscale,
    Sepia,
    Invert,
    CrossProcess,
    Cyanotype,
    NightVision,
    Vintage,
}

impl VideoLook {
    pub const ALL: &'static [VideoLook] = &[
        VideoLook::None,
        VideoLook::Grayscale,
        VideoLook::Sepia,
        VideoLook::Invert,
        VideoLook::CrossProcess,
        VideoLook::Cyanotype,
        VideoLook::NightVision,
        VideoLook::Vintage,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            VideoLook::None => "None",
            VideoLook::Grayscale => "Grayscale",
            VideoLook::Sepia => "Sepia",
            VideoLook::Invert => "Invert",
            VideoLook::CrossProcess => "Cross-process",
            VideoLook::Cyanotype => "Cyanotype",
            VideoLook::NightVision => "Night vision",
            VideoLook::Vintage => "Vintage",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionType {
    None,
    FadeBlack,
    Wipe,
    Slide,
    Zoom,
    SDissolveGlow,
    SDissolveLensFlare,
    SDissolveUltraGlow,
    SFilmRoll,
    SSwishPan,
    SHyperPull,
}

impl TransitionType {
    pub const ALL: &'static [TransitionType] = &[
        TransitionType::None,
        TransitionType::FadeBlack,
        TransitionType::Wipe,
        TransitionType::Slide,
        TransitionType::Zoom,
        TransitionType::SDissolveGlow,
        TransitionType::SDissolveLensFlare,
        TransitionType::SDissolveUltraGlow,
        TransitionType::SFilmRoll,
        TransitionType::SSwishPan,
        TransitionType::SHyperPull,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            TransitionType::None => "None",
            TransitionType::FadeBlack => "Fade through black",
            TransitionType::Wipe => "Wipe",
            TransitionType::Slide => "Slide",
            TransitionType::Zoom => "Zoom",
            TransitionType::SDissolveGlow => "SDissolveGlow (Sapphire)",
            TransitionType::SDissolveLensFlare => "SDissolveLensFlare (Sapphire)",
            TransitionType::SDissolveUltraGlow => "SDissolveUltraGlow (Sapphire)",
            TransitionType::SFilmRoll => "SFilmRoll (Sapphire)",
            TransitionType::SSwishPan => "SSwishPan (Sapphire)",
            TransitionType::SHyperPull => "SHyperPull (Sapphire)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskType {
    None,
    Ellipse,
    Rectangle,
    LinearGradient,
}

impl MaskType {
    pub const ALL: &'static [MaskType] = &[
        MaskType::None,
        MaskType::Ellipse,
        MaskType::Rectangle,
        MaskType::LinearGradient,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            MaskType::None => "None",
            MaskType::Ellipse => "Ellipse",
            MaskType::Rectangle => "Rectangle",
            MaskType::LinearGradient => "Linear gradient",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextOverlayItem {
    pub text: String,
    pub size_pct: f32,
    pub x_pct: f32,
    pub y_pct: f32,
    pub start_sec: f64,
    pub end_sec: f64,
    pub color: [u8; 3],
}

#[derive(Debug, Clone)]
pub struct OverlayTrackItem {
    pub path: PathBuf,
    pub name: String,
    pub duration_sec: f64,
    pub start_sec: f64,
    pub x_pct: f32,
    pub y_pct: f32,
    pub size_pct: f32,
    pub opacity_pct: f32,
}

#[derive(Debug, Clone)]
pub struct MusicTrackItem {
    pub path: PathBuf,
    pub name: String,
    pub duration_sec: f64,
    pub in_sec: f64,
    pub start_sec: f64,
    pub volume_pct: f32,
}

#[derive(Debug, Clone)]
pub struct VideoFxParams {
    // Adjust / Basic & Color
    pub brightness: f32,       // 0..200% (100 default)
    pub contrast: f32,         // 0..200% (100 default)
    pub saturation: f32,       // 0..300% (100 default)
    pub hue_deg: f32,          // 0..360°
    pub temperature: f32,      // -100..100
    pub tint: f32,             // -100..100
    pub look: VideoLook,
    pub wheel_shadows: [f32; 2],
    pub wheel_midtones: [f32; 2],
    pub wheel_highlights: [f32; 2],
    pub curves_rgb_mid: [f32; 2],
    pub hsl_hue_shift: f32,
    pub hsl_sat_shift: f32,
    pub hsl_lum_shift: f32,
    // Transitions
    pub transition: TransitionType,
    pub transition_dur: f32,
    // Core Effects
    pub blur_px: f32,
    pub vignette: f32,
    pub film_grain: f32,
    pub scanlines: f32,
    pub pixelate: f32,
    pub glitch: f32,
    pub posterize: f32,
    pub rgb_split: f32,
    pub kaleidoscope: u32,
    pub camera_shake: f32,
    pub motion_trail: f32,
    // Mask
    pub mask_type: MaskType,
    pub mask_x: f32,
    pub mask_y: f32,
    pub mask_size: f32,
    pub mask_feather: f32,
    pub mask_invert: bool,
    // Light / Stylize / Transform
    pub glow: f32,
    pub light_leak: f32,
    pub strobe: f32,
    pub tilt_shift: f32,
    pub sharpen: f32,
    pub duotone: f32,
    pub zoom_blur: f32,
    pub zoom_pct: f32,         // 100..250%
    pub rotate_deg: f32,       // -180..180
    pub flip_h: bool,
    pub flip_v: bool,
    pub mirror_mode: String,
    pub letterbox: f32,
    // Print & Neon
    pub halftone_px: f32,
    pub neon_edges: f32,
    pub ripple_rings: f32,
    // Color FX
    pub solarize: f32,
    pub threshold_bw: f32,
    pub bleach_bypass: f32,
    pub channel_swap: String,
    // Hue Cycle, Tile, Spotlight
    pub hue_cycle_speed: f32,
    pub tile_repeat: u32,
    pub spotlight_dark: f32,
    pub spotlight_radius: f32,
    // BorisFX Sapphire S_ Suite (40+ effects)
    pub wavesenabled: bool,
    pub wave_a_amp: f32,
    pub wave_a_freq: f32,
    pub wave_b_amp: f32,
    pub wave_b_freq: f32,
    pub perspectiveenabled: bool,
    pub persp_lat: f32,
    pub persp_swing: f32,
    pub persp_roll: f32,
    pub cube_rot_deg: f32,
    pub s_fisheye: f32,
    pub s_magnify: f32,
    pub s_polar: bool,
    pub s_bubble2: f32,
    pub s_vortex: f32,
    pub s_warp_chroma: f32,
    pub s_warp_puddle: f32,
    pub s_warp_transform_rot: f32,
    pub s_blur_dir: f32,
    pub s_blur_rad: f32,
    pub s_blur_motion: f32,
    pub s_rack_defocus: f32,
    pub s_z_blur: f32,
    pub s_edge_blur: f32,
    pub s_beauty: f32,
    pub s_glow: f32,
    pub s_glow_noise: f32,
    pub s_glow_rainbow: f32,
    pub s_glow_orthicon: f32,
    pub s_halation: f32,
    pub s_glare: f32,
    pub s_glint: f32,
    pub s_sparkle: f32,
    pub s_flashbulb: f32,
    pub s_rays: f32,
    pub s_streaks: f32,
    pub s_lens_flare: f32,
    pub s_edge_rays: f32,
    pub s_light_leak: f32,
    pub s_quadtone: f32,
    pub s_multitone: f32,
    pub s_tritone: f32,
    pub s_zebrafy: f32,
    pub s_pixel_sort: f32,
    pub s_warp_puff: f32,
    pub s_emboss_distort: f32,
    pub s_emboss_shiny: f32,
    pub s_zap: f32,
    pub s_aurora: f32,
    pub s_caustics: f32,
    pub s_clouds: f32,
    pub s_texture_cells: f32,
    pub s_auto_paint: f32,
    pub s_cartoon: f32,
    pub s_etching: f32,
    pub s_scanlines: f32,
    pub s_film_damage: f32,
    pub s_shake: f32,
    pub s_psyko_stripes: bool,
    pub s_psyko_blobs: bool,
    // Red Giant Universe style
    pub u_chromatic_glow: f32,
    pub u_edge_glow: f32,
    pub u_starglow: f32,
    pub u_holomatrix: f32,
    pub u_rgb_sep: f32,
    pub u_heatwave: f32,
    pub u_glitch: f32,
    pub u_noir: f32,
    pub u_retrograde: f32,
    // Boris Continuum style
    pub bcc_film_grain: f32,
    pub bcc_film_damage: f32,
    pub bcc_vignette: f32,
    pub bcc_glow: f32,
    pub bcc_lightning: f32,
    pub bcc_laser: f32,
    pub bcc_halftone: f32,
    pub bcc_pencil: f32,
    pub bcc_mosaic: f32,
    pub bcc_median: u32,
    pub bcc_falsecolor: f32,
    // TV Simulator (CRT)
    pub crtenabled: bool,
    pub crt_curvature: f32,
    pub crt_scanlines: f32,
    pub crt_phosphor: f32,
    pub crt_bloom: f32,
    pub crt_bleed: f32,
    pub crt_noise: f32,
}

impl Default for VideoFxParams {
    fn default() -> Self {
        Self {
            brightness: 100.0,
            contrast: 100.0,
            saturation: 100.0,
            hue_deg: 0.0,
            temperature: 0.0,
            tint: 0.0,
            look: VideoLook::None,
            wheel_shadows: [0.0, 0.0],
            wheel_midtones: [0.0, 0.0],
            wheel_highlights: [0.0, 0.0],
            curves_rgb_mid: [0.5, 0.5],
            hsl_hue_shift: 0.0,
            hsl_sat_shift: 0.0,
            hsl_lum_shift: 0.0,
            transition: TransitionType::None,
            transition_dur: 0.8,
            blur_px: 0.0,
            vignette: 0.0,
            film_grain: 0.0,
            scanlines: 0.0,
            pixelate: 0.0,
            glitch: 0.0,
            posterize: 0.0,
            rgb_split: 0.0,
            kaleidoscope: 0,
            camera_shake: 0.0,
            motion_trail: 0.0,
            mask_type: MaskType::None,
            mask_x: 50.0,
            mask_y: 50.0,
            mask_size: 60.0,
            mask_feather: 20.0,
            mask_invert: false,
            glow: 0.0,
            light_leak: 0.0,
            strobe: 0.0,
            tilt_shift: 0.0,
            sharpen: 0.0,
            duotone: 0.0,
            zoom_blur: 0.0,
            zoom_pct: 100.0,
            rotate_deg: 0.0,
            flip_h: false,
            flip_v: false,
            mirror_mode: "None".into(),
            letterbox: 0.0,
            halftone_px: 0.0,
            neon_edges: 0.0,
            ripple_rings: 0.0,
            solarize: 0.0,
            threshold_bw: 0.0,
            bleach_bypass: 0.0,
            channel_swap: "None".into(),
            hue_cycle_speed: 0.0,
            tile_repeat: 1,
            spotlight_dark: 0.0,
            spotlight_radius: 40.0,
            wavesenabled: false,
            wave_a_amp: 0.0,
            wave_a_freq: 6.0,
            wave_b_amp: 0.0,
            wave_b_freq: 3.0,
            perspectiveenabled: false,
            persp_lat: 35.0,
            persp_swing: 0.0,
            persp_roll: 0.0,
            cube_rot_deg: 0.0,
            s_fisheye: 0.0,
            s_magnify: 0.0,
            s_polar: false,
            s_bubble2: 0.0,
            s_vortex: 0.0,
            s_warp_chroma: 0.0,
            s_warp_puddle: 0.0,
            s_warp_transform_rot: 0.0,
            s_blur_dir: 0.0,
            s_blur_rad: 0.0,
            s_blur_motion: 0.0,
            s_rack_defocus: 0.0,
            s_z_blur: 0.0,
            s_edge_blur: 0.0,
            s_beauty: 0.0,
            s_glow: 0.0,
            s_glow_noise: 0.0,
            s_glow_rainbow: 0.0,
            s_glow_orthicon: 0.0,
            s_halation: 0.0,
            s_glare: 0.0,
            s_glint: 0.0,
            s_sparkle: 0.0,
            s_flashbulb: 0.0,
            s_rays: 0.0,
            s_streaks: 0.0,
            s_lens_flare: 0.0,
            s_edge_rays: 0.0,
            s_light_leak: 0.0,
            s_quadtone: 0.0,
            s_multitone: 0.0,
            s_tritone: 0.0,
            s_zebrafy: 0.0,
            s_pixel_sort: 0.0,
            s_warp_puff: 0.0,
            s_emboss_distort: 0.0,
            s_emboss_shiny: 0.0,
            s_zap: 0.0,
            s_aurora: 0.0,
            s_caustics: 0.0,
            s_clouds: 0.0,
            s_texture_cells: 0.0,
            s_auto_paint: 0.0,
            s_cartoon: 0.0,
            s_etching: 0.0,
            s_scanlines: 0.0,
            s_film_damage: 0.0,
            s_shake: 0.0,
            s_psyko_stripes: false,
            s_psyko_blobs: false,
            u_chromatic_glow: 0.0,
            u_edge_glow: 0.0,
            u_starglow: 0.0,
            u_holomatrix: 0.0,
            u_rgb_sep: 0.0,
            u_heatwave: 0.0,
            u_glitch: 0.0,
            u_noir: 0.0,
            u_retrograde: 0.0,
            bcc_film_grain: 0.0,
            bcc_film_damage: 0.0,
            bcc_vignette: 0.0,
            bcc_glow: 0.0,
            bcc_lightning: 0.0,
            bcc_laser: 0.0,
            bcc_halftone: 0.0,
            bcc_pencil: 0.0,
            bcc_mosaic: 0.0,
            bcc_median: 0,
            bcc_falsecolor: 0.0,
            crtenabled: false,
            crt_curvature: 0.5,
            crt_scanlines: 0.5,
            crt_phosphor: 0.35,
            crt_bloom: 0.4,
            crt_bleed: 2.0,
            crt_noise: 0.12,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimelineClip {
    pub path: PathBuf,
    pub name: String,
    pub duration_sec: f64,
    pub trim_start_sec: f64,
    pub trim_end_sec: f64,
    pub speed: f32,
    pub is_compound: bool,
    pub fx: VideoFxParams,
}

impl TimelineClip {
    pub fn effective_duration(&self) -> f64 {
        let raw = (self.trim_end_sec - self.trim_start_sec).max(0.05);
        raw / (self.speed.max(0.1) as f64)
    }
}

#[derive(Debug, Clone)]
pub struct VideoExportSettings {
    pub container: ContainerFormat,
    pub video_codec: VideoCodec,
    pub audio_codec: VideoAudioCodec,
    pub resolution: ResolutionPreset,
    pub fps: u32,
    pub auto_bitrate: bool,
    pub video_bitrate_mbps: u32,
    pub audio_bitrate_kbps: u32,
    pub hardware_encode: bool,
    pub also_export_pcm_wav: bool,
    pub audio_only_format: AudioExportFormat,
    pub audio_only_sr: u32,
}

impl Default for VideoExportSettings {
    fn default() -> Self {
        Self {
            container: ContainerFormat::Mp4,
            video_codec: VideoCodec::H264,
            audio_codec: VideoAudioCodec::Aac,
            resolution: ResolutionPreset::P1080,
            fps: 30,
            auto_bitrate: true,
            video_bitrate_mbps: 12,
            audio_bitrate_kbps: 192,
            hardware_encode: true,
            also_export_pcm_wav: false,
            audio_only_format: AudioExportFormat::Wav24,
            audio_only_sr: 48_000,
        }
    }
}

pub fn check_ffmpeg() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn probe_media_info(path: &Path) -> (f64, u32, u32) {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height:format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output();

    let mut dur = 10.0;
    let mut w = 1920u32;
    let mut h = 1080u32;

    if let Ok(o) = out {
        if o.status.success() {
            if let Ok(s) = String::from_utf8(o.stdout) {
                let nums: Vec<&str> = s.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
                for line in nums {
                    if let Ok(iv) = line.parse::<u32>() {
                        if w == 1920 && iv > 64 {
                            w = iv;
                        } else if iv > 64 {
                            h = iv;
                        }
                    } else if let Ok(fv) = line.parse::<f64>() {
                        if fv.is_finite() && fv > 0.0 {
                            dur = fv;
                        }
                    }
                }
            }
        }
    }
    (dur, w, h)
}

pub fn validate_video_export(s: &VideoExportSettings) -> Result<(), String> {
    match s.container {
        ContainerFormat::Avi => {
            if s.video_codec != VideoCodec::Mjpeg {
                return Err("AVI carries MJPEG video: select MJPEG.".into());
            }
            if !matches!(s.audio_codec, VideoAudioCodec::Pcm | VideoAudioCodec::None) {
                return Err("AVI carries PCM audio: pick PCM or No audio.".into());
            }
        }
        ContainerFormat::Mkv => {
            if s.video_codec == VideoCodec::Mjpeg {
                return Err("Matroska needs H.264, HEVC, AV1 or VP9 video.".into());
            }
        }
        ContainerFormat::Webm => {
            if !matches!(s.video_codec, VideoCodec::Vp9 | VideoCodec::Av1) {
                return Err("WebM needs VP9 or AV1 video.".into());
            }
            if !matches!(s.audio_codec, VideoAudioCodec::Opus | VideoAudioCodec::None) {
                return Err("WebM uses Opus audio.".into());
            }
        }
        ContainerFormat::Mp4 | ContainerFormat::Mov => {
            if !matches!(
                s.video_codec,
                VideoCodec::H264 | VideoCodec::Hevc | VideoCodec::Av1
            ) {
                return Err("MP4 and MOV need H.264, HEVC or AV1 video.".into());
            }
            if s.audio_codec == VideoAudioCodec::Pcm {
                return Err("PCM goes in MKV or AVI, not MP4 or MOV.".into());
            }
        }
        ContainerFormat::FramesZip => {}
    }
    Ok(())
}

pub fn export_video_sequence(
    clips: &[TimelineClip],
    texts: &[TextOverlayItem],
    project: &ProjectSettings,
    settings: &VideoExportSettings,
    outputpath: &Path,
) -> Result<String, String> {
    if clips.is_empty() {
        return Err("Add at least one clip first.".into());
    }
    validate_video_export(settings)?;

    if settings.container == ContainerFormat::FramesZip {
        let temp_dir = std::env::temp_dir().join("grain_frames_export");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;
        let clip = &clips[0];
        let frame_pattern = temp_dir.join("frame_%06d.jpg");
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-ss",
                &format!("{:.3}", clip.trim_start_sec),
                "-to",
                &format!("{:.3}", clip.trim_end_sec),
                "-i",
            ])
            .arg(&clip.path)
            .args(["-r", &settings.fps.to_string(), "-q:v", "2"])
            .arg(&frame_pattern)
            .output()
            .map_err(|e| format!("FFmpeg failed: {}", e))?;
        if !status.status.success() {
            return Err(String::from_utf8_lossy(&status.stderr).to_string());
        }
        let zip_res = Command::new("zip")
            .args(["-j", "-q"])
            .arg(outputpath)
            .arg(temp_dir.join("*.jpg"))
            .output();
        if zip_res.is_err() {
            let py = format!(
                "import zipfile, glob, os; z=zipfile.ZipFile(r'{}','w'); [z.write(f, os.path.basename(f)) for f in sorted(glob.glob(r'{}/*.jpg'))]; z.close()",
                outputpath.display(),
                temp_dir.display()
            );
            let _ = Command::new("python3").args(["-c", &py]).output();
        }
        return Ok(format!(
            "Exported JPEG Frame Sequence ZIP → {}",
            outputpath.display()
        ));
    }

    let vcodec = match settings.video_codec {
        VideoCodec::H264 => {
            if settings.hardware_encode && cfg!(target_os = "macos") {
                "h264_videotoolbox"
            } else {
                "libx264"
            }
        }
        VideoCodec::Hevc => {
            if settings.hardware_encode && cfg!(target_os = "macos") {
                "hevc_videotoolbox"
            } else {
                "libx265"
            }
        }
        VideoCodec::Av1 => "libsvtav1",
        VideoCodec::Vp9 => "libvpx-vp9",
        VideoCodec::Mjpeg => "mjpeg",
    };

    let acodec = match settings.audio_codec {
        VideoAudioCodec::Aac => "aac",
        VideoAudioCodec::Opus => "libopus",
        VideoAudioCodec::Pcm => "pcm_s16le",
        VideoAudioCodec::None => "none",
    };

    let clip = &clips[0];
    let fx = &clip.fx;
    let mut args: Vec<String> = vec!["-y".into()];
    if settings.hardware_encode && cfg!(target_os = "macos") {
        args.extend(["-hwaccel".into(), "videotoolbox".into()]);
    }
    args.extend([
        "-ss".into(),
        format!("{:.3}", clip.trim_start_sec),
        "-to".into(),
        format!("{:.3}", clip.trim_end_sec),
        "-i".into(),
        clip.path.to_string_lossy().to_string(),
    ]);

    let mut vf: Vec<String> = Vec::new();
    if (clip.speed - 1.0).abs() > 0.01 {
        vf.push(format!("setpts=PTS/{:.4}", clip.speed));
    }

    let b = (fx.brightness - 100.0) / 100.0;
    let c = fx.contrast / 100.0;
    let s = fx.saturation / 100.0;
    if b.abs() > 0.01 || (c - 1.0).abs() > 0.01 || (s - 1.0).abs() > 0.01 {
        vf.push(format!(
            "eq=brightness={:.3}:contrast={:.3}:saturation={:.3}",
            b, c, s
        ));
    }
    if fx.hue_deg.abs() > 0.5 {
        vf.push(format!("hue=h={:.1}", fx.hue_deg));
    }
    match fx.look {
        VideoLook::Grayscale | VideoLook::NightVision => vf.push("hue=s=0".into()),
        VideoLook::Sepia | VideoLook::Vintage => {
            vf.push("colorchannelmixer=.393:.769:.189:0:.349:.686:.168:0:.272:.534:.131".into())
        }
        VideoLook::Invert => vf.push("negate".into()),
        VideoLook::None | VideoLook::CrossProcess | VideoLook::Cyanotype => {}
    }
    let blur_total = fx.blur_px + fx.s_rack_defocus * 0.1 + fx.s_z_blur * 0.08 + fx.s_edge_blur * 0.08;
    if blur_total > 0.1 {
        vf.push(format!("boxblur={:.1}:1", blur_total.min(25.0)));
    }
    if fx.sharpen > 1.0 || fx.s_beauty > 1.0 {
        let amt = (fx.sharpen / 50.0) - (fx.s_beauty / 120.0);
        vf.push(format!("unsharp=5:5:{:.2}:5:5:0.0", amt));
    }
    if fx.flip_h {
        vf.push("hflip".into());
    }
    if fx.flip_v {
        vf.push("vflip".into());
    }
    if fx.vignette > 0.02 || fx.bcc_vignette > 1.0 {
        vf.push("vignette".into());
    }
    if fx.film_grain > 0.02 || fx.bcc_film_grain > 1.0 || fx.s_film_damage > 1.0 {
        let strength = (fx.film_grain * 25.0 + fx.bcc_film_grain * 0.25 + fx.s_film_damage * 0.25).min(45.0);
        vf.push(format!("noise=alls={:.0}:allf=t+u", strength));
    }
    if fx.s_glow > 1.0 || fx.s_halation > 1.0 {
        let g_boost = (fx.s_glow + fx.s_halation) * 0.002;
        vf.push(format!("eq=brightness={:.3}:contrast={:.3}", g_boost.min(0.25), 1.0 + g_boost));
    }
    if let Some(h) = settings.resolution.height() {
        vf.push(format!("scale=-2:{}", h));
    }
    if let Some(ar) = project.aspect.ratio() {
        match project.framing {
            FramingMode::FillCrop => {
                vf.push(format!("crop=ih*{:.4}:ih", ar));
            }
            FramingMode::FitLetterbox => {
                vf.push(format!(
                    "pad=max(iw\\,ih*{:.4}):max(ih\\,iw/{:.4}):(ow-iw)/2:(oh-ih)/2:black",
                    ar, ar
                ));
            }
        }
    }

    for tx in texts {
        if !tx.text.trim().is_empty() {
            let escaped = tx
                .text
                .replace('\\', "\\\\")
                .replace(':', "\\:")
                .replace('\'', "\\'");
            let hex_col = format!("#{:02x}{:02x}{:02x}", tx.color[0], tx.color[1], tx.color[2]);
            let end_clause = if tx.end_sec > 0.0 {
                format!(":enable='between(t,{:.2},{:.2})'", tx.start_sec, tx.end_sec)
            } else {
                format!(":enable='gte(t,{:.2})'", tx.start_sec)
            };
            vf.push(format!(
                "drawtext=text='{}':fontcolor={}:fontsize=h*{:.3}:x=w*{:.3}-text_w/2:y=h*{:.3}-text_h/2:box=1:boxcolor=black@0.45:boxborderw=6{}",
                escaped,
                hex_col,
                tx.size_pct / 100.0,
                tx.x_pct / 100.0,
                tx.y_pct / 100.0,
                end_clause
            ));
        }
    }

    if !vf.is_empty() {
        args.extend(["-vf".into(), vf.join(",")]);
    }

    let br_kbps = if settings.auto_bitrate {
        12_000
    } else {
        settings.video_bitrate_mbps * 1000
    };
    args.extend([
        "-c:v".into(),
        vcodec.into(),
        "-b:v".into(),
        format!("{}k", br_kbps),
        "-r".into(),
        settings.fps.to_string(),
    ]);

    if acodec == "none" {
        args.push("-an".into());
    } else {
        if (clip.speed - 1.0).abs() > 0.01 {
            let atempo = clip.speed.clamp(0.5, 2.0);
            args.extend(["-af".into(), format!("atempo={:.4}", atempo)]);
        }
        args.extend([
            "-c:a".into(),
            acodec.into(),
            "-b:a".into(),
            format!("{}k", settings.audio_bitrate_kbps),
        ]);
    }

    args.push(outputpath.to_string_lossy().to_string());

    let out = Command::new("ffmpeg")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to invoke ffmpeg (`brew install ffmpeg`): {}", e))?;

    if !out.status.success() {
        return Err(format!(
            "FFmpeg export error: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    Ok(format!(
        "Exported {} ({} @ {} fps) → {}",
        settings.container.ext().to_uppercase(),
        vcodec,
        settings.fps,
        outputpath.display()
    ))
}

pub fn decode_audio_stereo(inputpath: &Path) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    let temp_wav = std::env::temp_dir().join("grain_audio_decode.wav");
    let dec = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(inputpath)
        .args(["-c:a", "pcm_f32le", "-ac", "2"])
        .arg(&temp_wav)
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;
    if !dec.status.success() {
        return Err(format!(
            "Audio decode failed: {}",
            String::from_utf8_lossy(&dec.stderr)
        ));
    }

    let mut reader =
        hound::WavReader::open(&temp_wav).map_err(|e| format!("WAV read error: {}", e))?;
    let spec = reader.spec();
    let samples: Vec<f32> = reader.samples::<f32>().filter_map(Result::ok).collect();
    let frames = samples.len() / 2;
    let mut left = Vec::with_capacity(frames);
    let mut right = Vec::with_capacity(frames);
    for i in 0..frames {
        left.push(samples[i * 2]);
        right.push(samples[i * 2 + 1]);
    }
    Ok((left, right, spec.sample_rate))
}

pub fn process_and_export_audio(
    inputpath: &Path,
    outputpath: &Path,
    st: &AudioLabState,
) -> Result<String, String> {
    let (left, right, src_sr) = decode_audio_stereo(inputpath)?;
    let (proc_l, proc_r) = process_stereo(&left, &right, src_sr, st);

    let temp_processed = std::env::temp_dir().join("grain_audio_processed.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: src_sr,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&temp_processed, spec)
        .map_err(|e| format!("WAV write error: {}", e))?;
    for i in 0..proc_l.len() {
        writer.write_sample(proc_l[i]).map_err(|e| e.to_string())?;
        writer.write_sample(proc_r[i]).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;

    let target_sr = st.export_sample_rate.to_string();
    let bitrate = format!("{}k", st.export_bitrate_kbps);
    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        temp_processed.to_string_lossy().to_string(),
        "-ar".to_string(),
        target_sr,
    ];

    match st.export_format {
        AudioExportFormat::Wav16 => args.extend(["-c:a".into(), "pcm_s16le".into()]),
        AudioExportFormat::Wav24 => args.extend(["-c:a".into(), "pcm_s24le".into()]),
        AudioExportFormat::WavF32 => args.extend(["-c:a".into(), "pcm_f32le".into()]),
        AudioExportFormat::Flac16 => {
            args.extend(["-c:a".into(), "flac".into(), "-sample_fmt".into(), "s16".into()])
        }
        AudioExportFormat::Flac24 => {
            args.extend(["-c:a".into(), "flac".into(), "-sample_fmt".into(), "s32".into()])
        }
        AudioExportFormat::AacM4a => {
            args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), bitrate])
        }
        AudioExportFormat::OggOpus => {
            args.extend(["-c:a".into(), "libopus".into(), "-b:a".into(), bitrate])
        }
    }
    args.push(outputpath.to_string_lossy().to_string());

    let enc = Command::new("ffmpeg")
        .args(&args)
        .output()
        .map_err(|e| format!("FFmpeg encode error: {}", e))?;
    if !enc.status.success() {
        return Err(String::from_utf8_lossy(&enc.stderr).to_string());
    }

    Ok(format!(
        "Exported {} ({} Hz) → {}",
        st.export_format.label(),
        st.export_sample_rate,
        outputpath.display()
    ))
}


#[derive(Debug, Clone)]
pub struct EffectKeyframe {
    pub time_sec: f64,
    pub easing: EasingCurve,
    pub params: VideoFxParams,
}

pub fn evaluate_keyframed_params(keyframes: &[EffectKeyframe], current_time: f64) -> VideoFxParams {
    if keyframes.is_empty() {
        return VideoFxParams::default();
    }
    if current_time <= keyframes[0].time_sec {
        return keyframes[0].params.clone();
    }
    if current_time >= keyframes[keyframes.len() - 1].time_sec {
        return keyframes[keyframes.len() - 1].params.clone();
    }

    let mut idx = 0;
    for i in 0..keyframes.len() - 1 {
        if current_time >= keyframes[i].time_sec && current_time <= keyframes[i + 1].time_sec {
            idx = i;
            break;
        }
    }

    let k0 = &keyframes[idx];
    let k1 = &keyframes[idx + 1];

    let duration = k1.time_sec - k0.time_sec;
    if duration <= 0.0001 {
        return k0.params.clone();
    }

    let raw_t = ((current_time - k0.time_sec) / duration) as f32;
    let t = k0.easing.eval(raw_t);

    let mut interpolated = k0.params.clone();
    interpolated.brightness = k0.params.brightness + (k1.params.brightness - k0.params.brightness) * t;
    interpolated.contrast = k0.params.contrast + (k1.params.contrast - k0.params.contrast) * t;
    interpolated.saturation = k0.params.saturation + (k1.params.saturation - k0.params.saturation) * t;
    interpolated.hue_deg = k0.params.hue_deg + (k1.params.hue_deg - k0.params.hue_deg) * t;
    interpolated.blur_px = k0.params.blur_px + (k1.params.blur_px - k0.params.blur_px) * t;
    interpolated.vignette = k0.params.vignette + (k1.params.vignette - k0.params.vignette) * t;
    interpolated.film_grain = k0.params.film_grain + (k1.params.film_grain - k0.params.film_grain) * t;
    interpolated.glow = k0.params.glow + (k1.params.glow - k0.params.glow) * t;
    interpolated.glitch = k0.params.glitch + (k1.params.glitch - k0.params.glitch) * t;
    interpolated.zoom_pct = k0.params.zoom_pct + (k1.params.zoom_pct - k0.params.zoom_pct) * t;
    interpolated.rotate_deg = k0.params.rotate_deg + (k1.params.rotate_deg - k0.params.rotate_deg) * t;
    interpolated.s_glow = k0.params.s_glow + (k1.params.s_glow - k0.params.s_glow) * t;
    interpolated.s_halation = k0.params.s_halation + (k1.params.s_halation - k0.params.s_halation) * t;
    interpolated.s_rays = k0.params.s_rays + (k1.params.s_rays - k0.params.s_rays) * t;
    interpolated.s_lens_flare = k0.params.s_lens_flare + (k1.params.s_lens_flare - k0.params.s_lens_flare) * t;
    interpolated.s_fisheye = k0.params.s_fisheye + (k1.params.s_fisheye - k0.params.s_fisheye) * t;
    interpolated.s_vortex = k0.params.s_vortex + (k1.params.s_vortex - k0.params.s_vortex) * t;

    interpolated
}

pub fn build_dynamic_ffmpeg_filter(keyframes: &[EffectKeyframe], base_fx: &VideoFxParams) -> String {
    let mut filters = Vec::new();

    if keyframes.len() > 1 {
        let k0 = &keyframes[0];
        let k1 = &keyframes[1];
        let dt = (k1.time_sec - k0.time_sec).max(0.001);

        let b0 = (k0.params.brightness - 100.0) / 100.0;
        let b1 = (k1.params.brightness - 100.0) / 100.0;
        let c0 = k0.params.contrast / 100.0;
        let c1 = k1.params.contrast / 100.0;
        let s0 = k0.params.saturation / 100.0;
        let s1 = k1.params.saturation / 100.0;

        let b_expr = format!("'{:.3}+({:.3}-({:.3}))*clip((t-{:.3})/{:.3},0,1)'", b0, b1, b0, k0.time_sec, dt);
        let c_expr = format!("'{:.3}+({:.3}-({:.3}))*clip((t-{:.3})/{:.3},0,1)'", c0, c1, c0, k0.time_sec, dt);
        let s_expr = format!("'{:.3}+({:.3}-({:.3}))*clip((t-{:.3})/{:.3},0,1)'", s0, s1, s0, k0.time_sec, dt);

        filters.push(format!("eq=brightness={}:contrast={}:saturation={}", b_expr, c_expr, s_expr));
    } else {
        let b = (base_fx.brightness - 100.0) / 100.0;
        let c = base_fx.contrast / 100.0;
        let s = base_fx.saturation / 100.0;
        if b.abs() > 0.01 || (c - 1.0).abs() > 0.01 || (s - 1.0).abs() > 0.01 {
            filters.push(format!("eq=brightness={:.3}:contrast={:.3}:saturation={:.3}", b, c, s));
        }
    }

    if base_fx.blur_px > 0.1 {
        filters.push(format!("boxblur={:.1}:1", base_fx.blur_px.min(25.0)));
    }

    filters.join(",")
}

pub fn play_media_preview(path: &Path) -> Result<(), String> {
    let status = Command::new("ffplay")
        .arg("-autoexit")
        .arg("-window_title")
        .arg("Grain Preview - ffplay")
        .arg(path)
        .status()
        .map_err(|e| format!("Failed to spawn ffplay (`brew install ffmpeg`): {}", e))?;
    if !status.success() {
        return Err("ffplay exited with non-zero status".into());
    }
    Ok(())
}


pub fn build_preview_vf_chain(fx: &VideoFxParams, w: u32, h: u32) -> String {
    let mut vf: Vec<String> = vec![format!("scale={}:{}", w, h)];

    let b = (fx.brightness - 100.0) / 100.0;
    let c = fx.contrast / 100.0;
    let s = fx.saturation / 100.0;
    if b.abs() > 0.01 || (c - 1.0).abs() > 0.01 || (s - 1.0).abs() > 0.01 {
        vf.push(format!(
            "eq=brightness={:.3}:contrast={:.3}:saturation={:.3}",
            b, c, s
        ));
    }
    if fx.hue_deg.abs() > 0.5 || fx.hsl_hue_shift.abs() > 0.5 {
        vf.push(format!("hue=h={:.1}", fx.hue_deg + fx.hsl_hue_shift));
    }
    match fx.look {
        VideoLook::Grayscale | VideoLook::NightVision => vf.push("hue=s=0".into()),
        VideoLook::Sepia | VideoLook::Vintage => {
            vf.push("colorchannelmixer=.393:.769:.189:0:.349:.686:.168:0:.272:.534:.131".into())
        }
        VideoLook::Invert => vf.push("negate".into()),
        VideoLook::CrossProcess => vf.push("curves=r='0/0 0.5/0.65 1/1':b='0/0.1 0.5/0.4 1/0.9'".into()),
        VideoLook::Cyanotype => vf.push("colorchannelmixer=.2:.4:.4:0:.3:.5:.5:0:.4:.6:.8".into()),
        VideoLook::None => {}
    }
    let blur_total = fx.blur_px
        + fx.s_rack_defocus * 0.1
        + fx.s_z_blur * 0.08
        + fx.s_edge_blur * 0.08
        + fx.zoom_blur * 0.06;
    if blur_total > 0.1 {
        vf.push(format!("boxblur={:.1}:1", blur_total.min(20.0)));
    }
    if fx.sharpen > 1.0 || fx.s_beauty > 1.0 {
        let amt = (fx.sharpen / 50.0) - (fx.s_beauty / 120.0);
        vf.push(format!("unsharp=5:5:{:.2}:5:5:0.0", amt));
    }
    if fx.flip_h {
        vf.push("hflip".into());
    }
    if fx.flip_v {
        vf.push("vflip".into());
    }
    if fx.rotate_deg.abs() > 0.5 {
        vf.push(format!("rotate={:.4}*PI/180", fx.rotate_deg));
    }
    if fx.zoom_pct > 101.0 {
        let z = (fx.zoom_pct / 100.0).min(2.5);
        vf.push(format!("crop=iw/{:.3}:ih/{:.3},scale={}:{}", z, z, w, h));
    }
    if fx.pixelate > 1.5 || fx.bcc_mosaic > 1.5 {
        let px = fx.pixelate.max(fx.bcc_mosaic).clamp(2.0, 48.0) as u32;
        let sw = (w / px).max(8);
        let sh = (h / px).max(8);
        vf.push(format!("scale={}:{}:flags=neighbor,scale={}:{}:flags=neighbor", sw, sh, w, h));
    }
    if fx.vignette > 0.02 || fx.bcc_vignette > 1.0 || fx.spotlight_dark > 5.0 {
        vf.push("vignette".into());
    }
    if fx.film_grain > 0.02 || fx.bcc_film_grain > 1.0 || fx.s_film_damage > 1.0 {
        let strength = (fx.film_grain * 25.0 + fx.bcc_film_grain * 0.25 + fx.s_film_damage * 0.25).min(45.0);
        vf.push(format!("noise=alls={:.0}:allf=t+u", strength));
    }
    if fx.s_glow > 1.0 || fx.s_halation > 1.0 || fx.glow > 1.0 || fx.bcc_glow > 1.0 {
        let g_boost = (fx.s_glow + fx.s_halation + fx.glow + fx.bcc_glow) * 0.0018;
        vf.push(format!("eq=brightness={:.3}:contrast={:.3}", g_boost.min(0.28), 1.0 + g_boost));
    }
    if fx.neon_edges > 2.0 || fx.u_edge_glow > 2.0 {
        vf.push("edgedetect=low=0.1:high=0.3".into());
    }
    if fx.solarize > 2.0 {
        vf.push("negate".into());
    }
    if fx.threshold_bw > 2.0 {
        vf.push("hue=s=0,eq=contrast=3.0".into());
    }
    if fx.crtenabled || fx.scanlines > 0.05 || fx.s_scanlines > 2.0 {
        vf.push("drawgrid=w=0:h=4:t=1:c=black@0.35".into());
    }
    if fx.letterbox > 0.5 {
        let bar = ((h as f32) * (fx.letterbox / 100.0) * 0.5) as u32;
        if bar > 0 && bar * 2 < h {
            vf.push(format!("drawbox=x=0:y=0:w={}:h={}:color=black:t=fill,drawbox=x=0:y={}:w={}:h={}:color=black:t=fill", w, bar, h - bar, w, bar));
        }
    }

    vf.join(",")
}

pub fn extract_video_frame_rgba(
    path: &Path,
    time_sec: f64,
    width: u32,
    height: u32,
    fx: &VideoFxParams,
) -> Result<Vec<u8>, String> {
    let w = width.max(16) & !1;
    let h = height.max(16) & !1;
    let vf_chain = build_preview_vf_chain(fx, w, h);
    let out = Command::new("ffmpeg")
        .args([
            "-ss",
            &format!("{:.3}", time_sec.max(0.0)),
            "-i",
        ])
        .arg(path)
        .args([
            "-vframes",
            "1",
            "-vf",
            &vf_chain,
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .output()
        .map_err(|e| format!("Failed to invoke ffmpeg for frame preview: {}", e))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    let expected = (w * h * 4) as usize;
    if out.stdout.len() < expected {
        return Err("Incomplete frame data from ffmpeg".into());
    }
    Ok(out.stdout[..expected].to_vec())
}
