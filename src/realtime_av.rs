#[cfg(feature = "cpal")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};

/// Shared lock-free master clock & transport state. With CPAL enabled this is
/// driven directly by the hardware callback; without CPAL a lightweight UI tick
/// advances the same cursor so the rest of the app has one playback API.
pub struct MasterAvClock {
    /// Current playback sample index at `device_sample_rate`.
    pub sample_cursor: AtomicU64,
    /// Whether real-time playback is currently active.
    pub is_playing: AtomicBool,
    /// Generation counter incremented whenever the buffer is swapped.
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

/// Stereo PCM buffer loaded into the real-time audio engine.
#[derive(Clone, Default)]
pub struct StereoPcmBuffer {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub sample_rate: u32,
}

fn lock_or_recover<'a, T>(mutex: &'a Mutex<T>) -> MutexGuard<'a, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(feature = "cpal")]
#[allow(clippy::too_many_arguments)]
fn write_output<T>(
    out: &mut [T],
    clock: &MasterAvClock,
    pcm: &StereoPcmBuffer,
    gain: f32,
    pan: f32,
    muted: bool,
    dev_sr: u32,
    channels: usize,
) where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    if !clock.is_playing.load(Ordering::Relaxed) || muted {
        out.fill(T::EQUILIBRIUM);
        return;
    }
    let total_frames = pcm.left.len().min(pcm.right.len());
    if total_frames == 0 {
        out.fill(T::EQUILIBRIUM);
        return;
    }
    let ratio = pcm.sample_rate.max(1) as f64 / dev_sr.max(1) as f64;
    let pan = pan.clamp(-1.0, 1.0);
    let angle = std::f64::consts::FRAC_PI_4 * (pan + 1.0) as f64;
    let gl = (gain as f64 * angle.cos()).max(-4.0).min(4.0) as f32;
    let gr = (gain as f64 * angle.sin()).max(-4.0).min(4.0) as f32;
    let mut cur = clock.sample_cursor.load(Ordering::Relaxed);

    for frame in out.chunks_mut(channels) {
        let src_pos = (cur as f64 * ratio) as usize;
        if src_pos >= total_frames {
            clock.is_playing.store(false, Ordering::Relaxed);
            frame.fill(T::EQUILIBRIUM);
            continue;
        }
        let l = pcm.left[src_pos] * gl;
        let r = pcm.right[src_pos] * gr;
        if channels == 1 {
            frame[0] = T::from_sample_((l + r) * 0.5);
        } else {
            frame[0] = T::from_sample_(l);
            frame[1] = T::from_sample_(r);
            for extra in frame.iter_mut().skip(2) {
                *extra = T::EQUILIBRIUM;
            }
        }
        cur += 1;
    }
    clock.sample_cursor.store(cur, Ordering::Relaxed);
}

/// Real-time audio engine backed by CPAL when the `cpal` feature is enabled.
/// The UI always talks to this type; a no-device stub keeps `--no-default-features`
/// builds compiling while still providing a usable silent transport clock.
pub struct RealtimeAudioEngine {
    #[cfg(feature = "cpal")]
    _stream: Option<cpal::Stream>,
    pub device_sample_rate: u32,
    pub clock: Arc<MasterAvClock>,
    pub buffer: Arc<Mutex<StereoPcmBuffer>>,
    gain: Arc<Mutex<f32>>,
    pan: Arc<Mutex<f32>>,
    muted: Arc<AtomicBool>,
    status_msg: String,
}

impl Default for RealtimeAudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RealtimeAudioEngine {
    #[cfg(feature = "cpal")]
    pub fn new() -> Self {
        let clock = MasterAvClock::new();
        let buffer = Arc::new(Mutex::new(StereoPcmBuffer::default()));
        let gain = Arc::new(Mutex::new(1.0_f32));
        let pan = Arc::new(Mutex::new(0.0_f32));
        let muted = Arc::new(AtomicBool::new(false));

        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return Self {
                _stream: None,
                device_sample_rate: 48_000,
                clock,
                buffer,
                gain,
                pan,
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
                pan,
                muted,
                status_msg: "CPAL: Could not query output config".into(),
            };
        };

        let dev_sr = supported_cfg.sample_rate().0.max(1);
        let channels = supported_cfg.channels().max(1) as usize;
        let sample_fmt = supported_cfg.sample_format();
        let stream_cfg: cpal::StreamConfig = supported_cfg.into();
        let err_fn = |err| eprintln!("[Grain CPAL Audio Error]: {}", err);

        macro_rules! build_stream {
            ($ty:ty) => {{
                let clock_cb = Arc::clone(&clock);
                let buf_cb = Arc::clone(&buffer);
                let gain_cb = Arc::clone(&gain);
                let pan_cb = Arc::clone(&pan);
                let muted_cb = Arc::clone(&muted);
                device.build_output_stream(
                    &stream_cfg,
                    move |out: &mut [$ty], _: &cpal::OutputCallbackInfo| {
                        let pcm = lock_or_recover(&buf_cb);
                        let g = *lock_or_recover(&gain_cb);
                        let pn = *lock_or_recover(&pan_cb);
                        write_output(
                            out,
                            &clock_cb,
                            &pcm,
                            g,
                            pn,
                            muted_cb.load(Ordering::Relaxed),
                            dev_sr,
                            channels,
                        );
                    },
                    err_fn,
                    None,
                )
            }};
        }

        let stream_res = match sample_fmt {
            cpal::SampleFormat::F32 => build_stream!(f32),
            cpal::SampleFormat::I16 => build_stream!(i16),
            cpal::SampleFormat::U16 => build_stream!(u16),
            cpal::SampleFormat::I32 => build_stream!(i32),
            cpal::SampleFormat::U32 => build_stream!(u32),
            cpal::SampleFormat::I8 => build_stream!(i8),
            cpal::SampleFormat::U8 => build_stream!(u8),
            other => {
                return Self {
                    _stream: None,
                    device_sample_rate: dev_sr,
                    clock,
                    buffer,
                    gain,
                    pan,
                    muted,
                    status_msg: format!("CPAL: Unsupported hardware sample format {:?}", other),
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
                    pan,
                    muted,
                    status_msg: format!("CPAL audio ready · {} Hz", dev_sr),
                }
            }
            Err(e) => Self {
                _stream: None,
                device_sample_rate: dev_sr,
                clock,
                buffer,
                gain,
                pan,
                muted,
                status_msg: format!("CPAL stream error: {}", e),
            },
        }
    }

    #[cfg(not(feature = "cpal"))]
    pub fn new() -> Self {
        Self {
            device_sample_rate: 48_000,
            clock: MasterAvClock::new(),
            buffer: Arc::new(Mutex::new(StereoPcmBuffer::default())),
            gain: Arc::new(Mutex::new(1.0)),
            pan: Arc::new(Mutex::new(0.0)),
            muted: Arc::new(AtomicBool::new(false)),
            status_msg: "CPAL audio disabled — build with --features cpal for sound".into(),
        }
    }

    pub fn load_stereo_buffer(&self, left: Vec<f32>, right: Vec<f32>, sample_rate: u32) {
        let mut guard = lock_or_recover(&self.buffer);
        guard.left = left;
        guard.right = right;
        guard.sample_rate = sample_rate.max(1);
        self.clock.buffer_gen.fetch_add(1, Ordering::Relaxed);
    }

    fn effective_rate(&self) -> u32 {
        #[cfg(not(feature = "cpal"))]
        {
            lock_or_recover(&self.buffer).sample_rate.max(1)
        }
        #[cfg(feature = "cpal")]
        {
            self.device_sample_rate.max(1)
        }
    }

    pub fn seek_seconds(&self, sec: f64) {
        let target = (sec.max(0.0) * self.effective_rate() as f64).round() as u64;
        self.clock.sample_cursor.store(target, Ordering::Relaxed);
    }

    pub fn play_from_seconds(&self, sec: f64) {
        self.seek_seconds(sec);
        self.clock.is_playing.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn pause(&self) {
        self.clock.is_playing.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// Advance the clock only in non-CPAL builds; hardware callbacks drive the
    /// clock in normal builds.
    pub fn advance(&self, dt: f32) {
        #[cfg(not(feature = "cpal"))]
        {
            if self.clock.is_playing.load(std::sync::atomic::Ordering::Relaxed) {
                let step = (dt.max(0.0) as f64 * self.effective_rate() as f64).round() as u64;
                self.clock.sample_cursor.fetch_add(step, Ordering::Relaxed);
            }
        }
        #[cfg(feature = "cpal")]
        let _ = dt;
    }

    pub fn current_time_seconds(&self) -> f64 {
        self.clock.sample_cursor.load(Ordering::Relaxed) as f64
            / self.effective_rate() as f64
    }

    pub fn is_playing(&self) -> bool {
        self.clock.is_playing.load(Ordering::Relaxed)
    }

    pub fn set_gain(&self, gain: f32) {
        *lock_or_recover(&self.gain) = gain;
    }

    pub fn set_pan(&self, pan: f32) {
        *lock_or_recover(&self.pan) = pan.clamp(-1.0, 1.0);
    }

    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    pub fn status_message(&self) -> &str {
        &self.status_msg
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
    /// whenever sequential playback advances forward within 0.35s.
    pub fn read_frame_at(
        &mut self,
        path: &Path,
        target_sec: f64,
        vf_chain: &str,
    ) -> Result<Vec<u8>, String> {
        let frame_bytes = (self.width * self.height * 4) as usize;
        let dt = target_sec - self.stream_time_sec;
        let same_source = self.active_path.as_deref() == Some(path) && self.active_vf == vf_chain;

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
