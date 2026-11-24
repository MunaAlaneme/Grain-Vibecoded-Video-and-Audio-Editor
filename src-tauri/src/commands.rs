// Grain Video Editor — Rust core API
// Desktop: native FFmpeg + macOS VideoToolbox HW accel + Rust DSP
// Browser: WebCodecs fallback

use crate::audio_dsp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct AudioProcessRequest {
    pub input_path: String,
    pub output_path: Option<String>,
    pub effect: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportRequest {
    pub input_path: String,
    pub output_path: String,
    pub container: String,
    pub video_codec: String,
    pub audio_codec: String,
    pub video_bitrate_kbps: Option<u32>,
    pub audio_bitrate_kbps: Option<u32>,
    pub sample_rate: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub hardware_encode: bool,
    pub hardware_decode: bool,
}

#[tauri::command]
pub async fn process_audio(req: AudioProcessRequest) -> Result<String, String> {
    let semitones = req.params.get("semitones").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let factor = req.params.get("factor").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    let rate = req.params.get("rate").and_then(|v| v.as_f64()).unwrap_or(5.0) as f32;
    let depth = req.params.get("depth").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;

    if !std::path::Path::new(&req.input_path).exists() {
        return Ok(format!(
            "Rust DSP ready: {} (semitones={:.2}, factor={:.2}) [pass local file path for disk render]",
            req.effect, semitones, factor
        ));
    }

    let mut reader = hound::WavReader::open(&req.input_path)
        .map_err(|e| format!("Failed to open WAV '{}': {}", req.input_path, e))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().filter_map(Result::ok).map(|s| s as f32 / max).collect()
        }
    };

    let processed = match req.effect.as_str() {
        "pitch" => audio_dsp::pitch_shift_phase_vocoder(&samples, semitones, 2048),
        "time" => audio_dsp::time_stretch_wsola(&samples, factor),
        "vibrato" => audio_dsp::vibrato(&samples, rate, depth, spec.sample_rate as f32),
        "bitcrush" => audio_dsp::bitcrush(&samples, 8, 4),
        "invert" => samples.iter().map(|s| -s).collect(),
        _ => samples,
    };

    let out_path = req.output_path.unwrap_or_else(|| format!("{}.processed.wav", req.input_path));
    let out_spec = hound::WavSpec {
        channels: spec.channels,
        sample_rate: spec.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&out_path, out_spec)
        .map_err(|e| format!("Failed to create output WAV: {}", e))?;
    for s in processed {
        writer.write_sample(s).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;

    Ok(out_path)
}

#[tauri::command]
pub async fn export_video(req: ExportRequest) -> Result<String, String> {
    if req.container == "frames" {
        return Err("Frames ZIP export uses the built-in frame renderer".into());
    }

    if !std::path::Path::new(&req.input_path).exists() {
        return Err("NO_LOCAL_PATH".into());
    }

    let out = if req.output_path.is_empty() {
        let mut p = std::env::temp_dir();
        p.push(format!("grain_export.{}", req.container));
        p.to_string_lossy().to_string()
    } else {
        req.output_path.clone()
    };

    let vcodec = match req.video_codec.as_str() {
        "h264" => if req.hardware_encode && cfg!(target_os = "macos") { "h264_videotoolbox" } else { "libx264" },
        "hevc" => if req.hardware_encode && cfg!(target_os = "macos") { "hevc_videotoolbox" } else { "libx265" },
        "av1" => "libsvtav1",
        "vp9" => "libvpx-vp9",
        "mjpeg" => "mjpeg",
        other => other,
    };

    let acodec = match req.audio_codec.as_str() {
        "aac" => "aac",
        "opus" => "libopus",
        "flac" => "flac",
        "pcm" => "pcm_s24le",
        "none" => "none",
        other => other,
    };

    let mut args: Vec<String> = vec!["-y".into()];
    if req.hardware_decode && cfg!(target_os = "macos") {
        args.extend(["-hwaccel".into(), "videotoolbox".into()]);
    }
    args.extend(["-i".into(), req.input_path.clone()]);
    args.extend(["-c:v".into(), vcodec.into()]);

    if let Some(vbr) = req.video_bitrate_kbps {
        args.extend(["-b:v".into(), format!("{}k", vbr)]);
    }
    if let (Some(w), Some(h)) = (req.width, req.height) {
        if w > 0 && h > 0 {
            args.extend(["-vf".into(), format!("scale={}:{}", w, h)]);
        }
    }
    if let Some(fps) = req.fps {
        args.extend(["-r".into(), fps.to_string()]);
    }

    if acodec == "none" {
        args.push("-an".into());
    } else {
        args.extend(["-c:a".into(), acodec.into()]);
        if let Some(abr) = req.audio_bitrate_kbps {
            args.extend(["-b:a".into(), format!("{}k", abr)]);
        }
        if let Some(sr) = req.sample_rate {
            args.extend(["-ar".into(), sr.to_string()]);
        }
    }

    args.push(out.clone());

    let output = Command::new("ffmpeg")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to invoke ffmpeg (install via `brew install ffmpeg`): {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg export error: {}", stderr));
    }

    Ok(format!("Exported via Rust + FFmpeg ({}) to {}", vcodec, PathBuf::from(&out).display()))
}

#[tauri::command]
pub fn list_hardware_encoders() -> Vec<String> {
    if cfg!(target_os = "macos") {
        vec![
            "h264_videotoolbox".into(),
            "hevc_videotoolbox".into(),
            "prores_videotoolbox".into(),
            "software".into(),
        ]
    } else {
        vec![
            "h264_nvenc".into(),
            "hevc_nvenc".into(),
            "av1_nvenc".into(),
            "h264_vaapi".into(),
            "h264_qsv".into(),
            "software".into(),
        ]
    }
}

#[tauri::command]
pub fn get_engine_info() -> String {
    format!(
        "Grain Rust Engine v0.3 — {} ({})",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}
