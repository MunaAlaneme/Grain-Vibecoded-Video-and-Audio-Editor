// Grain Video Editor — Rust core API
// Commands exposed to the frontend via Tauri invoke().

mod audio_dsp;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AudioProcessRequest {
    pub input_path: String,
    pub effect: String, // "pitch", "time", "vibrato", …
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportRequest {
    pub input_path: String,
    pub output_path: String,
    pub container: String,      // "mp4", "mov", "mkv", "avi", "webm"
    pub video_codec: String,    // "h264", "hevc", "av1", "vp9", "mjpeg"
    pub audio_codec: String,    // "aac", "opus", "flac", "pcm", "none"
    pub video_bitrate_kbps: Option<u32>,
    pub audio_bitrate_kbps: Option<u32>,
    pub sample_rate: Option<u32>, // 48000 | 96000 | 192000
    pub hardware_encode: bool,
    pub hardware_decode: bool,
}

#[tauri::command]
pub async fn process_audio(req: AudioProcessRequest) -> Result<String, String> {
    // Demo: if effect == "pitch" and we had samples, call audio_dsp.
    // Real path will load the file, run the chosen DSP, write a temp WAV.
    let semitones = req
        .params
        .get("semitones")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;

    let _demo = audio_dsp::pitch_shift_simple(&[0.0, 0.1, 0.0, -0.1], semitones);

    Ok(format!(
        "Rust audio DSP: effect={} semitones={:.2} on {}",
        req.effect, semitones, req.input_path
    ))
}

#[tauri::command]
pub async fn export_video(req: ExportRequest) -> Result<String, String> {
    // Outline of the real FFmpeg pipeline (enabled with feature "ffmpeg"):
    //
    // 1. Open input with ffmpeg::format::input(&req.input_path)
    // 2. Select decoder (prefer HW if req.hardware_decode):
    //    h264_v4l2m2m, h264_videotoolbox, hevc_nvdec, …
    // 3. Configure encoder:
    //    video: libx264 / h264_videotoolbox / hevc_nvenc / libsvtav1 / libvpx-vp9 / mjpeg
    //    audio: aac / libopus / flac / pcm_s16le (sample_rate from req)
    // 4. Set bitrates (req.video_bitrate_kbps, req.audio_bitrate_kbps)
    // 5. Mux into container (mp4/mov/mkv/avi/webm) via output format
    // 6. Write to req.output_path
    //
    // Fallback when FFmpeg feature is off or libs missing:
    Ok(format!(
        "Export outline ready — {} → {} | video={} audio={} container={} | vbr={:?} abr={:?} sr={:?} | HW enc={} dec={}",
        req.input_path,
        req.output_path,
        req.video_codec,
        req.audio_codec,
        req.container,
        req.video_bitrate_kbps,
        req.audio_bitrate_kbps,
        req.sample_rate,
        req.hardware_encode,
        req.hardware_decode
    ))
}

#[tauri::command]
pub fn list_hardware_encoders() -> Vec<String> {
    // Will query FFmpeg for available HW encoders once the feature is on.
    vec![
        "h264_videotoolbox".into(),
        "hevc_videotoolbox".into(),
        "h264_nvenc".into(),
        "hevc_nvenc".into(),
        "h264_vaapi".into(),
        "h264_qsv".into(),
        "software (libx264 / libx265 / libsvtav1)".into(),
    ]
}

#[tauri::command]
pub fn get_engine_info() -> String {
    "Grain Rust engine v0.1 — Tauri desktop + shared core (DSP + FFmpeg export outline)".into()
}
