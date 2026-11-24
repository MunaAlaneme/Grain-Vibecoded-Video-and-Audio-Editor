use crate::audio_dsp::{process_channel, AudioDspParams};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerFormat {
    Mp4,
    Mov,
    Mkv,
    Webm,
    Avi,
}

impl ContainerFormat {
    pub const ALL: &'static [ContainerFormat] = &[
        ContainerFormat::Mp4,
        ContainerFormat::Mov,
        ContainerFormat::Mkv,
        ContainerFormat::Webm,
        ContainerFormat::Avi,
    ];

    pub fn ext(&self) -> &'static str {
        match self {
            ContainerFormat::Mp4 => "mp4",
            ContainerFormat::Mov => "mov",
            ContainerFormat::Mkv => "mkv",
            ContainerFormat::Webm => "webm",
            ContainerFormat::Avi => "avi",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ContainerFormat::Mp4 => "MP4 (.mp4)",
            ContainerFormat::Mov => "QuickTime MOV (.mov)",
            ContainerFormat::Mkv => "Matroska (.mkv)",
            ContainerFormat::Webm => "WebM (.webm)",
            ContainerFormat::Avi => "AVI (.avi)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
            VideoCodec::Vp9 => "VP9",
            VideoCodec::Mjpeg => "MJPEG",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCodec {
    Aac,
    Opus,
    Flac,
    Pcm24,
    None,
}

impl AudioCodec {
    pub const ALL: &'static [AudioCodec] = &[
        AudioCodec::Aac,
        AudioCodec::Opus,
        AudioCodec::Flac,
        AudioCodec::Pcm24,
        AudioCodec::None,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            AudioCodec::Aac => "AAC-LC",
            AudioCodec::Opus => "Opus",
            AudioCodec::Flac => "FLAC (Lossless)",
            AudioCodec::Pcm24 => "PCM 24-bit",
            AudioCodec::None => "No Audio",
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
}

impl ResolutionPreset {
    pub const ALL: &'static [ResolutionPreset] = &[
        ResolutionPreset::Source,
        ResolutionPreset::P480,
        ResolutionPreset::P720,
        ResolutionPreset::P1080,
        ResolutionPreset::P1440,
        ResolutionPreset::P2160,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ResolutionPreset::Source => "Source (Original)",
            ResolutionPreset::P480 => "480p SD",
            ResolutionPreset::P720 => "720p HD",
            ResolutionPreset::P1080 => "1080p Full HD",
            ResolutionPreset::P1440 => "1440p 2K",
            ResolutionPreset::P2160 => "2160p 4K UHD",
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
    pub brightness: f32,   // -1.0 to 1.0 (0.0 default)
    pub contrast: f32,     // 0.0 to 2.0 (1.0 default)
    pub saturation: f32,   // 0.0 to 3.0 (1.0 default)
    pub hue_deg: f32,      // -180.0 to 180.0
    pub blur: f32,         // 0.0 to 10.0
    pub grayscale: bool,
    pub sepia: bool,
    pub invert: bool,
    pub mirror_h: bool,
    pub mirror_v: bool,
    pub overlay_text: String,
}

impl TimelineClip {
    pub fn effective_duration(&self) -> f64 {
        let raw = (self.trim_end_sec - self.trim_start_sec).max(0.05);
        raw / (self.speed.max(0.1) as f64)
    }
}

#[derive(Debug, Clone)]
pub struct ExportSettings {
    pub container: ContainerFormat,
    pub video_codec: VideoCodec,
    pub audio_codec: AudioCodec,
    pub resolution: ResolutionPreset,
    pub fps: u32,
    pub video_bitrate_kbps: u32,
    pub audio_bitrate_kbps: u32,
    pub sample_rate: u32,
    pub hardware_encode: bool,
    pub hardware_decode: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            container: ContainerFormat::Mp4,
            video_codec: VideoCodec::H264,
            audio_codec: AudioCodec::Aac,
            resolution: ResolutionPreset::P1080,
            fps: 30,
            video_bitrate_kbps: 10_000,
            audio_bitrate_kbps: 192,
            sample_rate: 48_000,
            hardware_encode: true,
            hardware_decode: true,
        }
    }
}

pub fn check_ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn probe_duration(path: &Path) -> f64 {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output();

    if let Ok(o) = out {
        if o.status.success() {
            if let Ok(s) = String::from_utf8(o.stdout) {
                if let Ok(d) = s.trim().parse::<f64>() {
                    if d.is_finite() && d > 0.0 {
                        return d;
                    }
                }
            }
        }
    }
    10.0
}

pub fn validate_export_combo(settings: &ExportSettings) -> Result<(), String> {
    match settings.container {
        ContainerFormat::Webm => {
            if !matches!(settings.video_codec, VideoCodec::Vp9 | VideoCodec::Av1) {
                return Err("WebM requires VP9 or AV1 video codec.".into());
            }
            if !matches!(settings.audio_codec, AudioCodec::Opus | AudioCodec::None) {
                return Err("WebM requires Opus or No Audio.".into());
            }
        }
        ContainerFormat::Avi => {
            if settings.video_codec != VideoCodec::Mjpeg {
                return Err("AVI requires MJPEG video codec.".into());
            }
            if !matches!(settings.audio_codec, AudioCodec::Pcm24 | AudioCodec::None) {
                return Err("AVI requires PCM or No Audio.".into());
            }
        }
        ContainerFormat::Mp4 | ContainerFormat::Mov => {
            if matches!(settings.video_codec, VideoCodec::Mjpeg | VideoCodec::Vp9) {
                return Err("MP4 / MOV requires H.264, HEVC, or AV1 video.".into());
            }
            if settings.audio_codec == AudioCodec::Pcm24 {
                return Err("Use MKV or AVI for uncompressed PCM audio.".into());
            }
        }
        ContainerFormat::Mkv => {}
    }
    Ok(())
}

pub fn export_timeline(
    clips: &[TimelineClip],
    output_path: &Path,
    settings: &ExportSettings,
) -> Result<String, String> {
    if clips.is_empty() {
        return Err("Add at least one clip to the timeline before exporting.".into());
    }
    validate_export_combo(settings)?;

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
        AudioCodec::Aac => "aac",
        AudioCodec::Opus => "libopus",
        AudioCodec::Flac => "flac",
        AudioCodec::Pcm24 => "pcm_s24le",
        AudioCodec::None => "none",
    };

    // For single-clip fast native export, apply trim, speed, color grading, flip, blur, scale & text directly in FFmpeg
    let clip = &clips[0];
    let mut args: Vec<String> = vec!["-y".into()];

    if settings.hardware_decode && cfg!(target_os = "macos") {
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

    // Build video filter chain
    let mut vf_parts: Vec<String> = Vec::new();
    if (clip.speed - 1.0).abs() > 0.01 {
        vf_parts.push(format!("setpts=PTS/{:.4}", clip.speed));
    }
    if clip.brightness.abs() > 0.01
        || (clip.contrast - 1.0).abs() > 0.01
        || (clip.saturation - 1.0).abs() > 0.01
    {
        vf_parts.push(format!(
            "eq=brightness={:.3}:contrast={:.3}:saturation={:.3}",
            clip.brightness, clip.contrast, clip.saturation
        ));
    }
    if clip.hue_deg.abs() > 0.5 {
        vf_parts.push(format!("hue=h={:.1}", clip.hue_deg));
    }
    if clip.grayscale {
        vf_parts.push("hue=s=0".into());
    }
    if clip.sepia {
        vf_parts.push("colorchannelmixer=.393:.769:.189:0:.349:.686:.168:0:.272:.534:.131".into());
    }
    if clip.invert {
        vf_parts.push("negate".into());
    }
    if clip.blur > 0.1 {
        vf_parts.push(format!("boxblur={:.1}:1", clip.blur));
    }
    if clip.mirror_h {
        vf_parts.push("hflip".into());
    }
    if clip.mirror_v {
        vf_parts.push("vflip".into());
    }
    if let Some(h) = settings.resolution.height() {
        vf_parts.push(format!("scale=-2:{}", h));
    }
    if !clip.overlay_text.trim().is_empty() {
        let escaped = clip
            .overlay_text
            .replace('\', "\\")
            .replace(':', "\:")
            .replace(''', "\'");
        vf_parts.push(format!(
            "drawtext=text='{}':fontcolor=white:fontsize=36:x=(w-text_w)/2:y=h-th-40:box=1:boxcolor=black@0.5:boxborderw=8",
            escaped
        ));
    }

    if !vf_parts.is_empty() {
        args.extend(["-vf".into(), vf_parts.join(",")]);
    }

    args.extend([
        "-c:v".into(),
        vcodec.into(),
        "-b:v".into(),
        format!("{}k", settings.video_bitrate_kbps),
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
            "-ar".into(),
            settings.sample_rate.to_string(),
        ]);
    }

    args.push(output_path.to_string_lossy().to_string());

    let output = Command::new("ffmpeg")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run ffmpeg (install via `brew install ffmpeg`): {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg failed: {}", err));
    }

    Ok(format!(
        "Exported {} ({}) → {}",
        settings.container.ext().to_uppercase(),
        vcodec,
        output_path.display()
    ))
}

pub fn export_audio_file(
    input_path: &Path,
    output_path: &Path,
    params: &AudioDspParams,
) -> Result<String, String> {
    // Decode input audio (any format supported by FFmpeg) into a temporary 32-bit float WAV
    let mut temp_in = std::env::temp_dir();
    temp_in.push("grain_rust_dsp_in.wav");

    let dec = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(input_path)
        .args(["-c:a", "pcm_f32le", "-ar", "48000", "-ac", "2"])
        .arg(&temp_in)
        .output()
        .map_err(|e| format!("Failed to invoke ffmpeg for audio decode: {}", e))?;

    if !dec.status.success() {
        return Err(format!(
            "Audio decode failed: {}",
            String::from_utf8_lossy(&dec.stderr)
        ));
    }

    let mut reader =
        hound::WavReader::open(&temp_in).map_err(|e| format!("WAV read error: {}", e))?;
    let spec = reader.spec();
    let interleaved: Vec<f32> = reader
        .samples::<f32>()
        .filter_map(Result::ok)
        .collect();

    let ch_count = spec.channels.max(1) as usize;
    let frame_count = interleaved.len() / ch_count;
    let mut channels = vec![Vec::with_capacity(frame_count); ch_count];

    for frame in 0..frame_count {
        for c in 0..ch_count {
            channels[c].push(interleaved[frame * ch_count + c]);
        }
    }

    let processed_channels: Vec<Vec<f32>> = channels
        .iter()
        .map(|ch| process_channel(ch, spec.sample_rate, params))
        .collect();

    let out_len = processed_channels
        .iter()
        .map(|c| c.len())
        .min()
        .unwrap_or(0);

    let out_spec = hound::WavSpec {
        channels: spec.channels,
        sample_rate: spec.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(output_path, out_spec)
        .map_err(|e| format!("Failed to create output WAV: {}", e))?;

    for i in 0..out_len {
        for c in 0..ch_count {
            writer
                .write_sample(processed_channels[c][i])
                .map_err(|e| e.to_string())?;
        }
    }
    writer.finalize().map_err(|e| e.to_string())?;

    Ok(format!(
        "Rust DSP ({}) exported → {}",
        params.effect.label(),
        output_path.display()
    ))
}
