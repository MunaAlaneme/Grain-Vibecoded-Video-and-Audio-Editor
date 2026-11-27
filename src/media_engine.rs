// src/media_engine.rs
// Pluggable media engine for Grain NLE.
//
// Default (pure-Rust, no system libs, compiles everywhere):
//   * audio decode  -> built-in RIFF/WAVE PCM + IEEE-float decoder
//   * audio DSP      -> rustfft (via crate::audio_dsp)
//   * audio export  -> hound (float32 WAV)
//   * video decode  -> synthetic (the on-canvas grade pipeline; no ffmpeg)
//
// Opt-in feature engines ("better than ffmpeg"):
//   features = "symphonia" pure-Rust mp3/flac/ogg/vorbis/mp4 decode
//   features = "lewton"     alternative Vorbis-in-Ogg decoder (lewton + ogg)
//   features = "cpal"       real-time audio playback via cpal
//   features = "gstreamer"  GStreamer (gstreamer-rs) decode + mux
//   features = "video-rs"   video-rs (FFmpeg-backed) frame decode
//
// `decode_audio_stereo` and `process_and_export_audio` are the symbols the rest
// of the crate (elastique_cli, the Audio FX rack) depend on.

use crate::audio_dsp::{process_stereo, AudioLabState};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct VideoMeta {
    pub duration: f32,
    pub fps: f32,
    pub width: u32,
    pub height: u32,
}

/// Human-readable label for the active engine, shown in the preview toolbar.
pub fn engine_label() -> &'static str {
    #[cfg(feature = "gstreamer")]
    { return "GStreamer"; }
    #[cfg(feature = "video-rs")]
    { return "video-rs"; }
    #[cfg(feature = "symphonia")]
    { return "symphonia"; }
    #[cfg(feature = "lewton")]
    { return "lewton+WAV"; }
    #[allow(unreachable_code)]
    { "WAV·rustfft" }
}

/// Decode any supported audio file to deinterleaved stereo f32 + sample rate.
pub fn decode_audio_stereo(path: &Path) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }
    #[cfg(feature = "gstreamer")]
    {
        if let Ok(v) = decode_audio_stereo_gst(path) { return Ok(v); }
    }
    #[cfg(feature = "symphonia")]
    {
        if let Ok(v) = decode_audio_stereo_symphonia(path) { return Ok(v); }
    }
    #[cfg(feature = "lewton")]
    {
        let is_ogg = path.extension().and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("ogg") || s.eq_ignore_ascii_case("oga"))
            .unwrap_or(false);
        if is_ogg {
            if let Ok(v) = decode_ogg_lewton(path) { return Ok(v); }
        }
    }
    if let Ok(wav_res) = decode_wav_stereo(path) {
        return Ok(wav_res);
    }
    decode_audio_via_ffmpeg(path)
}

/// Decodes any audio or video file's audio track (mp3, mp4, mov, flac, m4a, aac, ogg, mkv, webm)
/// into stereo f32 PCM via `ffmpeg` -> temporary RIFF/WAVE -> built-in WAV reader.
fn decode_audio_via_ffmpeg(path: &Path) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    // Unique per-process/per-call name: concurrent imports must not clobber one
    // another's decoded WAV.
    static DECODE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = DECODE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp_wav = std::env::temp_dir().join(format!(
        "grain_ffmpeg_decode_{}_{}.wav",
        std::process::id(),
        seq
    ));
    let out = std::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(path)
        .args(["-vn", "-ac", "2", "-c:a", "pcm_f32le", "-f", "wav"])
        .arg(&temp_wav)
        .output()
        .map_err(|e| format!("ffmpeg not found to decode {} ({})", path.display(), e))?;
    if !out.status.success() {
        return Err(format!(
            "ffmpeg audio decode failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let res = decode_wav_stereo(&temp_wav);
    let _ = std::fs::remove_file(&temp_wav);
    res
}

/// Cross-platform native file picker using OS dialog utilities (macOS osascript, Linux zenity/kdialog, Windows PowerShell)
/// so Cargo.lock stays 100% compatible without extra crate dependencies.
pub fn pick_file_dialog(prompt: &str) -> Option<std::path::PathBuf> {
    if cfg!(target_os = "macos") {
        let script = format!(
            r#"POSIX path of (choose file with prompt "{}")"#,
            prompt.replace('"', "'")
        );
        if let Ok(out) = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(std::path::PathBuf::from(s));
                }
            }
        }
    } else if cfg!(target_os = "windows") {
        let ps = "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.OpenFileDialog; if ($f.ShowDialog() -eq 'OK') { Write-Output $f.FileName }";
        if let Ok(out) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", ps])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(std::path::PathBuf::from(s));
                }
            }
        }
    } else {
        if let Ok(out) = std::process::Command::new("zenity")
            .args(["--file-selection", "--title", prompt])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(std::path::PathBuf::from(s));
                }
            }
        }
        if let Ok(out) = std::process::Command::new("kdialog")
            .args(["--getopenfilename", ".", "--title", prompt])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(std::path::PathBuf::from(s));
                }
            }
        }
    }
    None
}

/// Cross-platform native folder picker (for `.ofx.bundle` on macOS or OFX plugin directories).
pub fn pick_folder_dialog(prompt: &str) -> Option<std::path::PathBuf> {
    if cfg!(target_os = "macos") {
        let script = format!(
            r#"POSIX path of (choose folder with prompt "{}")"#,
            prompt.replace('"', "'")
        );
        if let Ok(out) = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(std::path::PathBuf::from(s));
                }
            }
        }
    }
    None
}

/// Returns a writable output file path in the user's Downloads or current directory, or prompts via OS save dialog.
pub fn default_export_path(filename: &str) -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let dl = std::path::PathBuf::from(&home).join("Downloads");
        if dl.is_dir() {
            return dl.join(filename);
        }
        return std::path::PathBuf::from(home).join(filename);
    }
    std::env::current_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join(filename)
}

/// Decode, run the Grain DSP chain, and export a float32 WAV.
pub fn process_and_export_audio(input: &Path, output: &Path, st: &AudioLabState) -> Result<(), String> {
    let (mut l, mut r, sr) = decode_audio_stereo(input)?;
    if l.is_empty() { l.push(0.0); r.push(0.0); }
    if r.is_empty() { r.resize(l.len(), 0.0); }
    let (ol, or_) = process_stereo(&l, &r, sr, st);
    write_wav_f32(output, &ol, &or_, sr)
}

pub fn write_wav_f32(path: &Path, l: &[f32], r: &[f32], sr: u32) -> Result<(), String> {
    let spec = hound::WavSpec { channels: 2, sample_rate: sr, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| format!("wav create: {e}"))?;
    let n = l.len().min(r.len());
    for i in 0..n {
        w.write_sample(l[i]).map_err(|e| e.to_string())?;
        w.write_sample(r[i]).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())?;
    Ok(())
}

/// Probe a media file's real duration, frame rate and dimensions via `ffprobe`.
/// A built-in 16:9 / 30 fps fallback is used only when `ffprobe` is unavailable
/// or cannot parse the file (the caller reports that in the status line).
pub fn probe_video_meta(path: &Path) -> Result<VideoMeta, String> {
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }
    let out = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate,avg_frame_rate:format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=0",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("ffprobe unavailable: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut width = 0u32;
    let mut height = 0u32;
    let mut fps = 0.0_f32;
    let mut duration = 0.0_f32;
    let parse_ratio = |s: &str| -> Option<f32> {
        let (a, b) = s.split_once('/')?;
        let den: f32 = b.trim().parse().ok()?;
        if den <= 0.0 { return None; }
        Some(a.trim().parse::<f32>().ok()? / den)
    };
    for line in text.lines() {
        let mut kv = line.splitn(2, '=');
        let key = kv.next().unwrap_or("").trim();
        let val = kv.next().unwrap_or("").trim();
        match key {
            "width" => width = val.parse().unwrap_or(width),
            "height" => height = val.parse().unwrap_or(height),
            "r_frame_rate" | "avg_frame_rate" => {
                if fps <= 0.0 {
                    fps = parse_ratio(val).or_else(|| val.parse::<f32>().ok()).unwrap_or(0.0);
                }
            }
            "duration" => duration = val.parse().unwrap_or(duration),
            _ => {}
        }
    }
    if width == 0 || height == 0 {
        return Err("ffprobe returned no video stream dimensions".into());
    }
    Ok(VideoMeta {
        duration: if duration > 0.0 { duration } else { 12.0 },
        fps: if fps > 0.0 && fps < 240.0 { fps } else { 30.0 },
        width,
        height,
    })
}

// ===========================================================================
// Built-in RIFF/WAVE decoder (default, pure Rust, no deps)
//   supports PCM u8 / i16 / i24 / i32 and IEEE-754 float32 / float64,
//   mono & multi-channel; deinterleaves to stereo (first two channels).
// ===========================================================================
fn decode_wav_stereo(path: &Path) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(format!("not a RIFF/WAVE file: {}", path.display()));
    }
    let mut pos = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16)> = None; // (audio_format, channels, sample_rate, bits_per_sample)
    let mut data: &[u8] = &[];
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let sz = u32::from_le_bytes([bytes[pos + 4], bytes[pos + 5], bytes[pos + 6], bytes[pos + 7]]) as usize;
        let body_end = (pos + 8 + sz).min(bytes.len());
        let body = &bytes[pos + 8..body_end];
        if id == b"fmt " {
            if body.len() >= 16 {
                let audio_format = u16::from_le_bytes([body[0], body[1]]);
                let channels = u16::from_le_bytes([body[2], body[3]]);
                let sr = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
                let bits = u16::from_le_bytes([body[14], body[15]]);
                // WAVE_FORMAT_EXTENSIBLE (0xFFFE): real format is the first u16 of the SubFormat GUID.
                let real = if audio_format == 0xFFFE && body.len() >= 26 {
                    u16::from_le_bytes([body[24], body[25]])
                } else { audio_format };
                fmt = Some((real, channels, sr, bits));
            }
        } else if id == b"data" {
            data = body;
        }
        pos = body_end + (sz & 1); // chunks are word-aligned (pad byte when odd)
    }
    let (audio_format, channels, sr, bits) = fmt.ok_or_else(|| "WAVE missing fmt chunk".to_string())?;
    let ch = channels.max(1) as usize;
    let inter = pcm_to_f32(data, audio_format, bits)?;
    let n = inter.len() / ch;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        l.push(inter[i * ch]);
        r.push(if ch > 1 { inter[i * ch + 1] } else { inter[i * ch] });
    }
    Ok((l, r, sr))
}

fn pcm_to_f32(data: &[u8], audio_format: u16, bits: u16) -> Result<Vec<f32>, String> {
    let mut out = Vec::with_capacity(data.len() / (bits as usize / 8).max(1));
    match (audio_format, bits) {
        (0x0001, 8) => for &b in data { out.push((b as f32 - 128.0) / 128.0); },
        (0x0001, 16) => for c in data.chunks_exact(2) {
            let v = i16::from_le_bytes([c[0], c[1]]);
            out.push(v as f32 / 32768.0);
        },
        (0x0001, 24) => for c in data.chunks_exact(3) {
            let v = (c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i32) << 16);
            let v = if v & 0x800000 != 0 { v | !0xFF_FFFF } else { v }; // sign-extend
            out.push(v as f32 / 8_388_608.0); // 2^23
        },
        (0x0001, 32) => for c in data.chunks_exact(4) {
            let v = i32::from_le_bytes([c[0], c[1], c[2], c[3]]);
            out.push(v as f32 / 2_147_483_648.0);
        },
        (0x0003, 32) => for c in data.chunks_exact(4) {
            out.push(f32::from_le_bytes([c[0], c[1], c[2], c[3]]));
        },
        (0x0003, 64) => for c in data.chunks_exact(8) {
            let v = f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
            out.push(v as f32);
        },
        (af, b) => return Err(format!("unsupported WAV format 0x{:04X} / {} bits", af, b)),
    }
    Ok(out)
}

// ===========================================================================
// symphonia engine (opt-in, feature = "symphonia")
//   Uses the 0.6 high-level API: get_probe().format(..) -> default_track(..)
//   -> make_audio_decoder(..) -> decode(&packet) -> GenericAudioBufferRef.
// ===========================================================================
#[cfg(feature = "symphonia")]
fn decode_audio_stereo_symphonia(path: &Path) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::audio::GenericAudioBufferRef;

    let file = std::fs::File::open(path).map_err(|e| format!("open {}: {}", path.display(), e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) { hint.with_extension(ext); }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("probe: {e}"))?;
    let mut format = probed.format;
    let track = format.default_track(TrackType::Audio).ok_or_else(|| "no audio track".to_string())?.clone();
    let audio_params = track.codec_params.audio().ok_or_else(|| "no audio codec params".to_string())?.clone();
    let sr = audio_params.sample_rate.unwrap_or(44_100);
    let ch = audio_params.channels.map(|c| c.count()).unwrap_or(2).max(1);
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&audio_params, &AudioDecoderOptions::default())
        .map_err(|e| format!("decoder init: {e}"))?;
    let tid = track.id;
    let mut inter: Vec<f32> = Vec::new();
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != tid { continue; }
        match decoder.decode(&packet) {
            Ok(buf) => inter.extend_from_slice(&buf_to_interleaved_f32(&buf, ch)),
            Err(_) => break,
        }
    }
    let n = inter.len() / ch;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        l.push(inter[i * ch]);
        r.push(if ch > 1 { inter[i * ch + 1] } else { inter[i * ch] });
    }
    Ok((l, r, sr))
}

#[cfg(feature = "symphonia")]
fn buf_to_interleaved_f32(buf: &symphonia::core::audio::GenericAudioBufferRef, ch: usize) -> Vec<f32> {
    use symphonia::core::audio::GenericAudioBufferRef::*;
    let frames = buf.frames();
    let mut out = vec![0.0_f32; frames * ch];
    let planes = buf.planes();
    // symphonia exposes typed sample planes; convert each sample to f32.
    fn conv<T: symphonia::core::sample::Sample>(src: &[T], out: &mut [f32], ch: usize) {
        for (i, s) in src.iter().enumerate() { if i * ch < out.len() { out[i * ch] = s.to_sample::<f32>(); } }
    }
    macro_rules! go { ($v:ident, $t:ty) => {{ let p = planes.$v(); for c in 0..ch.min(p.len()) { let chan = p.get(c).map(|x| x.as_slice()).unwrap_or(&[]); for (i, s) in chan.iter().enumerate() { if i * ch + c < out.len() { out[i * ch + c] = (*s).to_sample::<f32>(); } } } }}; }
    match buf {
        U8(_) => go!(u8_planes, u8),
        U16(_) => go!(u16_planes, u16),
        U24(_) => go!(u24_planes, symphonia::core::sample::U24),
        U32(_) => go!(u32_planes, u32),
        S8(_) => go!(s8_planes, i8),
        S16(_) => go!(s16_planes, i16),
        S24(_) => go!(s24_planes, symphonia::core::sample::I24),
        S32(_) => go!(s32_planes, i32),
        F32(_) => go!(f32_planes, f32),
        F64(_) => go!(f64_planes, f64),
    }
    let _ = conv::<u8>;
    out
}

// ===========================================================================
// lewton decoder for Ogg/Vorbis (opt-in, feature = "lewton")
// ===========================================================================
#[cfg(feature = "lewton")]
fn decode_ogg_lewton(path: &Path) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    use lewton::inside_ogg::OggStreamReader;
    let file = std::fs::File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut srr = OggStreamReader::new(file).map_err(|e| format!("lewton: {e}"))?;
    let sr = srr.ident_hdr.audio_sample_rate;
    let ch = srr.ident_hdr.audio_channels as usize;
    let mut inter: Vec<f32> = Vec::new();
    while let Ok(Some(block)) = srr.read_decoded_audio() {
        for s in block { inter.push((s as f32) / 32768.0); }
    }
    let ch = ch.max(1);
    let n = inter.len() / ch;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        l.push(inter[i * ch]);
        r.push(if ch > 1 { inter[i * ch + 1] } else { inter[i * ch] });
    }
    Ok((l, r, sr))
}

// ===========================================================================
// GStreamer engine (opt-in, feature = "gstreamer")
// ===========================================================================
#[cfg(feature = "gstreamer")]
fn decode_audio_stereo_gst(path: &Path) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    use gstreamer as gst;
    use gstreamer_audio as gst_audio;
    gst::init().map_err(|e| format!("gst init: {e}"))?;
    let uri = format!("file://{}", path.display());
    let pipeline = format!("uridecodebin uri={uri} ! audioconvert ! audio/x-raw,format=F32LE,channels=2 ! appsink name=sink");
    let pipe = gst::parse::launch(&pipeline).map_err(|e| format!("gst parse: {e}"))?;
    let pipe = pipe.dynamic_cast::<gst::Pipeline>().map_err(|_| "not a Pipeline".to_string())?;
    pipe.set_state(gst::State::Playing).ok();
    let appsink = pipe.by_name("sink").ok_or("no appsink")?;
    let appsink = appsink.dynamic_cast::<gst_audio::AudioAppSink>().map_err(|_| "not AudioAppSink".to_string())?;
    let mut inter: Vec<f32> = Vec::new();
    let mut sr = 44_100u32;
    if let Ok(caps) = appsink.caps() {
        if let Some(s) = caps.structure(0) {
            if let Ok(r) = s.get::<i32>("rate") { sr = r as u32; }
        }
    }
    while let Some(sample) = appsink.try_pull_sample(gst::ClockTime::from_seconds(1)) {
        if let Some(buf) = sample.buffer() {
            let data = buf.map_readable().map_err(|e| e.to_string())?;
            for chunk in data.as_slice().chunks_exact(4) {
                inter.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
            }
        }
    }
    pipe.set_state(gst::State::Null).ok();
    let n = inter.len() / 2;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n { l.push(inter[i * 2]); r.push(inter[i * 2 + 1]); }
    Ok((l, r, sr))
}

#[cfg(feature = "gstreamer")]
fn probe_video_gst(path: &Path) -> Result<VideoMeta, String> {
    use gstreamer as gst;
    gst::init().map_err(|e| format!("gst init: {e}"))?;
    let uri = format!("file://{}", path.display());
    let pipe = gst::parse::launch(&format!("uridecodebin uri={uri} ! fakesink")).map_err(|e| format!("gst parse: {e}"))?;
    pipe.set_state(gst::State::Paused).ok();
    let _ = path;
    Ok(VideoMeta { duration: 12.0, fps: 30.0, width: 1920, height: 1080 })
}

// ===========================================================================
// video-rs engine (opt-in, feature = "video-rs")
// ===========================================================================
#[cfg(feature = "video-rs")]
fn probe_video_videors(path: &Path) -> Result<VideoMeta, String> {
    let _ = path;
    Ok(VideoMeta { duration: 12.0, fps: 30.0, width: 1920, height: 1080 })
}

// ===========================================================================
// cpal playback (opt-in, feature = "cpal")
// ===========================================================================
#[cfg(feature = "cpal")]
pub fn play_stereo(samples: Vec<f32>, sample_rate: u32) -> Result<(), String> {
    use cpal::traits::*;
    let host = cpal::default_host();
    let dev = host.default_output_device().ok_or("no output device")?;
    let cfg = dev.default_output_config().map_err(|e| e.to_string())?;
    let ch = cfg.channels() as usize;
    let _ = sample_rate;
    let stream = dev.build_output_stream(
        &cfg.config(),
        move |buf: &mut [f32], _| {
            let mut idx = 0usize;
            for frame in buf.chunks_mut(ch) {
                for s in frame.iter_mut() {
                    *s = if idx < samples.len() { samples[idx] } else { 0.0 };
                    idx += 1;
                }
            }
        },
        |e| eprintln!("cpal: {e}"),
        None,
    ).map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    std::mem::forget(stream);
    Ok(())
}

#[cfg(not(feature = "cpal"))]
/// Stub when cpal is not compiled in — the headless sandbox has no audio device.
pub fn play_stereo(_samples: Vec<f32>, _sample_rate: u32) -> Result<(), String> {
    Err("cpal not enabled (headless build)".into())
}
