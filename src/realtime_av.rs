use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Shared lock-free master clock & transport state driven directly by the hardware audio callback.
pub struct MasterAvClock {
    /// Current playback sample index in the active buffer (at hardware sample rate)
    pub sample_cursor: AtomicU64,
    /// Whether real-time playback is currently active
    pub is_playing: AtomicBool,
    /// Generation counter incremented whenever a seek or live DSP update swaps the buffer
    pub buffer_gen: AtomicU64,
}

impl MasterAvClock {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            sample_cursor: AtomicU64::new(0),
            is_playing: AtomicBool::new(false),
            buffer_gen: AtomicU64::new(0),
        })
    }
}

/// Stereo PCM buffer loaded into the real-time CPAL audio engine
#[derive(Clone, Default)]
pub struct StereoPcmBuffer {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub sample_rate: u32,
}

/// True zero-latency hardware audio engine backed by CPAL + lock-free atomic cursor.
/// Eliminates external `ffplay` subprocess spawning and guarantees sample-accurate A/V sync.
pub struct RealtimeAudioEngine {
    _stream: Option<cpal::Stream>,
    pub device_sample_rate: u32,
    pub clock: Arc<MasterAvClock>,
    pub buffer: Arc<Mutex<StereoPcmBuffer>>,
    pub gain: Arc<Mutex<f32>>,
    pub muted: Arc<AtomicBool>,
    pub status_msg: String,
}

impl RealtimeAudioEngine {
    pub fn new() -> Self {
        let clock = MasterAvClock::new();
        let buffer = Arc::new(Mutex::new(StereoPcmBuffer::default()));
        let gain = Arc::new(Mutex::new(1.0_f32));
        let muted = Arc::new(AtomicBool::new(false));

        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return Self {
                _stream: None,
                device_sample_rate: 48_000,
                clock,
                buffer,
                gain,
                muted,
                status_msg: "CPAL: No default audio output device found".into(),
            };
        };

        let Ok(supported_cfg) = device.default_output_config() else {
            return Self {
                _stream: None,
                device_sample_rate: 48_000,
                clock,
                buffer,
                gain,
                muted,
                status_msg: "CPAL: Could not query output config".into(),
            };
        };

        let dev_sr = supported_cfg.sample_rate().0;
        let channels = supported_cfg.channels() as usize;
        let sample_fmt = supported_cfg.sample_format();
        let stream_cfg: cpal::StreamConfig = supported_cfg.into();

        let clock_cb = Arc::clone(&clock);
        let buf_cb = Arc::clone(&buffer);
        let gain_cb = Arc::clone(&gain);
        let muted_cb = Arc::clone(&muted);

        let err_fn = |err| eprintln!("[Grain CPAL Audio Error]: {}", err);

        let stream_res = match sample_fmt {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &stream_cfg,
                move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if !clock_cb.is_playing.load(Ordering::Relaxed)
                        || muted_cb.load(Ordering::Relaxed)
                    {
                        out.fill(0.0);
                        return;
                    }
                    let g = gain_cb.try_lock().map(|v| *v).unwrap_or(1.0);
                    let Ok(pcm) = buf_cb.try_lock() else {
                        out.fill(0.0);
                        return;
                    };
                    let total_frames = pcm.left.len().min(pcm.right.len());
                    if total_frames == 0 {
                        out.fill(0.0);
                        return;
                    }
                    let src_sr = pcm.sample_rate.max(1) as f64;
                    let ratio = src_sr / dev_sr.max(1) as f64;
                    let mut cur = clock_cb.sample_cursor.load(Ordering::Relaxed);

                    for frame in out.chunks_mut(channels) {
                        let src_pos = (cur as f64 * ratio) as usize;
                        if src_pos >= total_frames {
                            clock_cb.is_playing.store(false, Ordering::Relaxed);
                            for s in frame.iter_mut() {
                                *s = 0.0;
                            }
                        } else {
                            let l = pcm.left[src_pos] * g;
                            let r = pcm.right[src_pos] * g;
                            if channels == 1 {
                                frame[0] = (l + r) * 0.5;
                            } else {
                                frame[0] = l;
                                frame[1] = r;
                                for extra in frame.iter_mut().skip(2) {
                                    *extra = 0.0;
                                }
                            }
                            cur += 1;
                        }
                    }
                    clock_cb.sample_cursor.store(cur, Ordering::Relaxed);
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                &stream_cfg,
                move |out: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    if !clock_cb.is_playing.load(Ordering::Relaxed)
                        || muted_cb.load(Ordering::Relaxed)
                    {
                        out.fill(0);
                        return;
                    }
                    let g = gain_cb.try_lock().map(|v| *v).unwrap_or(1.0);
                    let Ok(pcm) = buf_cb.try_lock() else {
                        out.fill(0);
                        return;
                    };
                    let total_frames = pcm.left.len().min(pcm.right.len());
                    if total_frames == 0 {
                        out.fill(0);
                        return;
                    }
                    let src_sr = pcm.sample_rate.max(1) as f64;
                    let ratio = src_sr / dev_sr.max(1) as f64;
                    let mut cur = clock_cb.sample_cursor.load(Ordering::Relaxed);

                    for frame in out.chunks_mut(channels) {
                        let src_pos = (cur as f64 * ratio) as usize;
                        if src_pos >= total_frames {
                            clock_cb.is_playing.store(false, Ordering::Relaxed);
                            for s in frame.iter_mut() {
                                *s = 0;
                            }
                        } else {
                            let l = (pcm.left[src_pos] * g).clamp(-1.0, 1.0);
                            let r = (pcm.right[src_pos] * g).clamp(-1.0, 1.0);
                            frame[0] = (l * 32767.0) as i16;
                            if channels > 1 {
                                frame[1] = (r * 32767.0) as i16;
                            }
                            cur += 1;
                        }
                    }
                    clock_cb.sample_cursor.store(cur, Ordering::Relaxed);
                },
                err_fn,
                None,
            ),
            _ => {
                return Self {
                    _stream: None,
                    device_sample_rate: dev_sr,
                    clock,
                    buffer,
                    gain,
                    muted,
                    status_msg: "CPAL: Unsupported hardware sample format".into(),
                };
            }
        };

        match stream_res {
            Ok(stream) => {
                let _ = stream.play();
                Self {
                    _stream: Some(stream),
                    device_sample_rate: dev_sr,
                    clock,
                    buffer,
                    gain,
                    muted,
                    status_msg: format!("CPAL Real-Time Audio Engine Active ({} Hz)", dev_sr),
                }
            }
            Err(e) => Self {
                _stream: None,
                device_sample_rate: dev_sr,
                clock,
                buffer,
                gain,
                muted,
                status_msg: format!("CPAL stream error: {}", e),
            },
        }
    }

    pub fn load_stereo_buffer(&self, left: Vec<f32>, right: Vec<f32>, sample_rate: u32) {
        if let Ok(mut guard) = self.buffer.lock() {
            guard.left = left;
            guard.right = right;
            guard.sample_rate = sample_rate.max(1);
            self.clock.buffer_gen.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn seek_seconds(&self, sec: f64) {
        let target_sample = (sec.max(0.0) * self.device_sample_rate as f64).round() as u64;
        self.clock.sample_cursor.store(target_sample, Ordering::Relaxed);
    }

    pub fn play_from_seconds(&self, sec: f64) {
        self.seek_seconds(sec);
        self.clock.is_playing.store(true, Ordering::Relaxed);
    }

    pub fn pause(&self) {
        self.clock.is_playing.store(false, Ordering::Relaxed);
    }

    pub fn current_time_seconds(&self) -> f64 {
        let cur = self.clock.sample_cursor.load(Ordering::Relaxed);
        cur as f64 / self.device_sample_rate.max(1) as f64
    }
}

/// Persistent continuous FFmpeg raw-frame video decoder stream.
/// Keeps a single warm `ffmpeg` child process streaming RGBA frames into a ring buffer
/// instead of spawning a new `ffmpeg` process per frame (eliminates 50-120ms per-frame startup lag).
pub struct ContinuousVideoStream {
    child: Option<Child>,
    reader: Option<BufReader<std::process::ChildStdout>>,
    pub active_path: Option<PathBuf>,
    pub active_vf: String,
    pub stream_time_sec: f64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
}

impl ContinuousVideoStream {
    pub fn new(width: u32, height: u32, fps: f64) -> Self {
        Self {
            child: None,
            reader: None,
            active_path: None,
            active_vf: String::new(),
            stream_time_sec: -1.0,
            fps: fps.max(1.0),
            width,
            height,
        }
    }

    pub fn stop(&mut self) {
        self.reader = None;
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    /// Reads the exact RGBA frame at `target_sec` with `vf_chain`, reusing the running pipe
    /// whenever sequential playback advances forward within 0.25s.
    pub fn read_frame_at(
        &mut self,
        path: &Path,
        target_sec: f64,
        vf_chain: &str,
    ) -> Result<Vec<u8>, String> {
        let frame_bytes = (self.width * self.height * 4) as usize;
        let dt = target_sec - self.stream_time_sec;
        let same_source = self.active_path.as_deref() == Some(path) && self.active_vf == vf_chain;

        // Restart pipe only on seek backwards, large jump, clip change, or FX chain change
        if !same_source || self.reader.is_none() || dt < -0.02 || dt > 0.35 {
            self.stop();
            let mut child = Command::new("ffmpeg")
                .args([
                    "-loglevel",
                    "error",
                    "-fflags",
                    "nobuffer+fastseek",
                    "-flags",
                    "low_delay",
                    "-analyzeduration",
                    "0",
                    "-probesize",
                    "32768",
                    "-ss",
                    &format!("{:.3}", target_sec.max(0.0)),
                    "-i",
                ])
                .arg(path)
                .args([
                    "-an",
                    "-sn",
                    "-r",
                    &format!("{:.2}", self.fps),
                    "-vf",
                    vf_chain,
                    "-f",
                    "rawvideo",
                    "-pix_fmt",
                    "rgba",
                    "pipe:1",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("Failed to spawn continuous video stream: {}", e))?;

            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| "Failed to capture video stream stdout".to_string())?;
            self.reader = Some(BufReader::with_capacity(frame_bytes * 2, stdout));
            self.child = Some(child);
            self.active_path = Some(path.to_path_buf());
            self.active_vf = vf_chain.to_string();
            self.stream_time_sec = target_sec;
        }

        let reader = self.reader.as_mut().unwrap();
        let step = 1.0 / self.fps;
        let mut buf = vec![0u8; frame_bytes];

        // Advance sequentially until `stream_time_sec` reaches `target_sec`
        loop {
            reader
                .read_exact(&mut buf)
                .map_err(|e| format!("Stream EOF/read error: {}", e))?;
            let current_frame_t = self.stream_time_sec;
            self.stream_time_sec += step;
            if current_frame_t + step * 0.5 >= target_sec {
                return Ok(buf);
            }
        }
    }
}

impl Drop for ContinuousVideoStream {
    fn drop(&mut self) {
        self.stop();
    }
}
