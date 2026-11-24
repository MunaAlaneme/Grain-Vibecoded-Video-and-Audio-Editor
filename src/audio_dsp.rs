use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioEffect {
    PitchShift,
    TimeStretch,
    Vibrato,
    SpeedPitch,
    Bitcrush,
    Tremolo,
    Echo,
    Distortion,
    Reverse,
    Invert,
    Normalize,
}

impl AudioEffect {
    pub const ALL: &'static [AudioEffect] = &[
        AudioEffect::PitchShift,
        AudioEffect::TimeStretch,
        AudioEffect::Vibrato,
        AudioEffect::SpeedPitch,
        AudioEffect::Bitcrush,
        AudioEffect::Tremolo,
        AudioEffect::Echo,
        AudioEffect::Distortion,
        AudioEffect::Reverse,
        AudioEffect::Invert,
        AudioEffect::Normalize,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            AudioEffect::PitchShift => "Pitch Shift (Phase Vocoder)",
            AudioEffect::TimeStretch => "Time Stretch (WSOLA)",
            AudioEffect::Vibrato => "Vibrato (LFO)",
            AudioEffect::SpeedPitch => "Speed + Pitch (Resample)",
            AudioEffect::Bitcrush => "Bitcrush",
            AudioEffect::Tremolo => "Tremolo",
            AudioEffect::Echo => "Echo / Delay",
            AudioEffect::Distortion => "Soft-Clip Distortion",
            AudioEffect::Reverse => "Reverse",
            AudioEffect::Invert => "Invert Polarity",
            AudioEffect::Normalize => "Normalize (-0.5 dB)",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioDspParams {
    pub effect: AudioEffect,
    pub semitones: f32,
    pub time_factor: f32,
    pub rate_hz: f32,
    pub depth: f32,
    pub bit_depth: u8,
    pub sample_rate_div: u32,
    pub delay_ms: f32,
    pub feedback: f32,
    pub drive: f32,
    pub fft_size: usize,
}

impl Default for AudioDspParams {
    fn default() -> Self {
        Self {
            effect: AudioEffect::PitchShift,
            semitones: 0.0,
            time_factor: 1.0,
            rate_hz: 5.0,
            depth: 0.5,
            bit_depth: 8,
            sample_rate_div: 4,
            delay_ms: 250.0,
            feedback: 0.4,
            drive: 4.0,
            fft_size: 2048,
        }
    }
}

pub fn process_channel(samples: &[f32], sample_rate: u32, params: &AudioDspParams) -> Vec<f32> {
    let sr = sample_rate.max(1) as f32;
    match params.effect {
        AudioEffect::PitchShift => {
            let ratio = 2.0_f32.powf(params.semitones / 12.0);
            let stretched = stretch_phase_vocoder(samples, ratio, params.fft_size);
            resample_to(&stretched, samples.len())
        }
        AudioEffect::TimeStretch => stretch_wsola(samples, params.time_factor, sample_rate),
        AudioEffect::Vibrato => vibrato(samples, params.rate_hz, params.depth, sr),
        AudioEffect::SpeedPitch => {
            let semitones = 12.0 * params.time_factor.max(0.1).log2();
            pitch_shift_simple(samples, semitones)
        }
        AudioEffect::Bitcrush => bitcrush(samples, params.bit_depth, params.sample_rate_div),
        AudioEffect::Tremolo => tremolo(samples, params.rate_hz, params.depth, sr),
        AudioEffect::Echo => echo(samples, params.delay_ms, params.feedback, sr),
        AudioEffect::Distortion => distortion(samples, params.drive),
        AudioEffect::Reverse => samples.iter().rev().copied().collect(),
        AudioEffect::Invert => samples.iter().map(|s| -s).collect(),
        AudioEffect::Normalize => normalize(samples),
    }
}

fn hann_window(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / (n.saturating_sub(1).max(1)) as f32).cos())
        .collect()
}

pub fn stretch_phase_vocoder(samples: &[f32], factor: f32, fft_size: usize) -> Vec<f32> {
    if (factor - 1.0).abs() < 0.005 || samples.len() < fft_size {
        return samples.to_vec();
    }
    let n = fft_size.next_power_of_two().max(256);
    let syn_hop = n / 4;
    let ana_hop = ((syn_hop as f32) / factor).round().max(1.0) as usize;
    let win = hann_window(n);

    let out_len = ((samples.len() as f32) * factor).round() as usize + n;
    let mut out = vec![0.0f32; out_len];
    let mut sum_win = vec![0.0f32; out_len];

    let half = n / 2;
    let mut last_phase = vec![0.0f32; half + 1];
    let mut sum_phase = vec![0.0f32; half + 1];

    let mut planner = FftPlanner::<f32>::new();
    let fft_fwd = planner.plan_fft_forward(n);
    let fft_inv = planner.plan_fft_inverse(n);

    let mut buf = vec![Complex::new(0.0f32, 0.0f32); n];
    let mut in_pos = 0usize;
    let mut out_pos = 0usize;

    while in_pos + n <= samples.len() {
        for i in 0..n {
            buf[i] = Complex::new(samples[in_pos + i] * win[i], 0.0);
        }
        fft_fwd.process(&mut buf);

        for k in 0..=half {
            let mag = buf[k].norm();
            let ph = buf[k].arg();
            let mut delta = ph - last_phase[k];
            last_phase[k] = ph;

            let expected = (ana_hop as f32) * 2.0 * PI * (k as f32) / (n as f32);
            delta -= expected;
            delta -= 2.0 * PI * (delta / (2.0 * PI)).round();
            let true_freq = (expected + delta) / (ana_hop as f32);
            sum_phase[k] += (syn_hop as f32) * true_freq;

            let c = Complex::from_polar(mag, sum_phase[k]);
            buf[k] = c;
            if k > 0 && k < half {
                buf[n - k] = c.conj();
            }
        }

        fft_inv.process(&mut buf);
        let scale = 1.0 / (n as f32);
        for i in 0..n {
            if out_pos + i < out_len {
                out[out_pos + i] += buf[i].re * scale * win[i];
                sum_win[out_pos + i] += win[i] * win[i];
            }
        }

        in_pos += ana_hop;
        out_pos += syn_hop;
    }

    for i in 0..out_len {
        if sum_win[i] > 1e-6 {
            out[i] /= sum_win[i];
        }
    }
    let target = ((samples.len() as f32) * factor).round().max(1.0) as usize;
    out.truncate(target.min(out.len()));
    normalize_if_clipping(out)
}

pub fn stretch_wsola(samples: &[f32], factor: f32, sr: u32) -> Vec<f32> {
    if (factor - 1.0).abs() < 0.005 || samples.is_empty() {
        return samples.to_vec();
    }
    let seg = ((sr as f32) * 0.035).round().max(256.0) as usize;
    let overlap = ((sr as f32) * 0.012).round().max(32.0) as usize;
    let search = ((sr as f32) * 0.010).round().max(16.0) as usize;
    if samples.len() <= seg + search {
        return samples.to_vec();
    }

    let syn_hop = seg.saturating_sub(overlap).max(1);
    let ana_hop = ((syn_hop as f32) / factor).round().max(1.0) as usize;
    let out_len = ((samples.len() as f32) * factor).round() as usize + seg;
    let mut out = vec![0.0f32; out_len];
    let win = hann_window(overlap * 2);

    let first_copy = seg.min(samples.len());
    out[..first_copy].copy_from_slice(&samples[..first_copy]);

    let mut in_pos = ana_hop;
    let mut out_pos = syn_hop;

    while in_pos + seg + search < samples.len() && out_pos + seg < out_len {
        let tail_start = out_pos.saturating_sub(overlap);
        let mut best_offset = 0isize;
        let mut best_score = f32::NEG_INFINITY;

        for s in -(search as isize)..=(search as isize) {
            let cand = (in_pos as isize + s) as usize;
            if cand + overlap > samples.len() {
                continue;
            }
            let mut score = 0.0f32;
            for i in 0..overlap {
                score += out[tail_start + i] * samples[cand + i];
            }
            if score > best_score {
                best_score = score;
                best_offset = s;
            }
        }

        let src = (in_pos as isize + best_offset).max(0) as usize;
        for i in 0..overlap {
            let w = win[i + overlap];
            out[tail_start + i] = out[tail_start + i] * (1.0 - w) + samples[src + i] * w;
        }
        for i in overlap..seg {
            if src + i < samples.len() && out_pos - overlap + i < out_len {
                out[out_pos - overlap + i] = samples[src + i];
            }
        }

        in_pos += ana_hop;
        out_pos += syn_hop;
    }

    let target = ((samples.len() as f32) * factor).round().max(1.0) as usize;
    out.truncate(target.min(out.len()));
    out
}

pub fn resample_to(samples: &[f32], target_len: usize) -> Vec<f32> {
    if samples.is_empty() || target_len == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(target_len);
    let ratio = (samples.len().saturating_sub(1) as f32) / (target_len.saturating_sub(1).max(1) as f32);
    for i in 0..target_len {
        let pos = (i as f32) * ratio;
        let idx = pos as usize;
        let frac = pos - (idx as f32);
        let a = samples[idx];
        let b = samples.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

pub fn pitch_shift_simple(samples: &[f32], semitones: f32) -> Vec<f32> {
    if semitones.abs() < 0.01 {
        return samples.to_vec();
    }
    let factor = 2.0_f32.powf(semitones / 12.0);
    let out_len = ((samples.len() as f32) / factor).max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = (i as f32) * factor;
        let idx = src as usize;
        if idx + 1 >= samples.len() {
            break;
        }
        let frac = src - (idx as f32);
        out.push(samples[idx] * (1.0 - frac) + samples[idx + 1] * frac);
    }
    out
}

pub fn vibrato(samples: &[f32], rate_hz: f32, depth_semitones: f32, sample_rate: f32) -> Vec<f32> {
    let depth_rate = 2.0_f32.powf(depth_semitones / 12.0) - 1.0;
    let mut out = Vec::with_capacity(samples.len());
    let mut pos = 0.0f32;
    while (pos as usize) + 1 < samples.len() {
        let t = pos / sample_rate;
        let rate = 1.0 + depth_rate * (2.0 * PI * rate_hz * t).sin();
        let i0 = pos as usize;
        let frac = pos - (i0 as f32);
        out.push(samples[i0] * (1.0 - frac) + samples[i0 + 1] * frac);
        pos += rate.max(0.1);
    }
    out
}

pub fn bitcrush(samples: &[f32], bits: u8, sample_rate_div: u32) -> Vec<f32> {
    let levels = 2u32.pow(bits.clamp(1, 16) as u32) as f32;
    let div = sample_rate_div.max(1);
    let mut out = Vec::with_capacity(samples.len());
    let mut hold = 0.0;
    for (i, &s) in samples.iter().enumerate() {
        if (i as u32) % div == 0 {
            hold = (s * levels).round() / levels;
        }
        out.push(hold);
    }
    out
}

pub fn tremolo(samples: &[f32], rate_hz: f32, depth: f32, sample_rate: f32) -> Vec<f32> {
    let d = depth.clamp(0.0, 1.0);
    samples
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let v = (2.0 * PI * rate_hz * (i as f32) / sample_rate).sin();
            s * (1.0 - d * (0.5 - 0.5 * v))
        })
        .collect()
}

pub fn echo(samples: &[f32], delay_ms: f32, feedback: f32, sample_rate: f32) -> Vec<f32> {
    let delay = ((sample_rate * delay_ms / 1000.0).round() as usize).max(1);
    let fb = feedback.clamp(0.0, 0.92);
    let mut out = vec![0.0f32; samples.len() + delay * 6];
    for (i, &s) in samples.iter().enumerate() {
        out[i] += s;
        let mut amp = fb;
        let mut pos = i + delay;
        while amp > 0.005 && pos < out.len() {
            out[pos] += s * amp;
            pos += delay;
            amp *= fb;
        }
    }
    normalize_if_clipping(out)
}

pub fn distortion(samples: &[f32], drive: f32) -> Vec<f32> {
    let d = drive.max(0.1);
    let norm = d.tanh().max(1e-5);
    samples.iter().map(|&s| (s * d).tanh() / norm).collect()
}

pub fn normalize(samples: &[f32]) -> Vec<f32> {
    let peak = samples.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    if peak > 1e-6 {
        let gain = 0.944 / peak;
        samples.iter().map(|&s| s * gain).collect()
    } else {
        samples.to_vec()
    }
}

fn normalize_if_clipping(mut samples: Vec<f32>) -> Vec<f32> {
    let peak = samples.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    if peak > 1.0 {
        for s in &mut samples {
            *s /= peak;
        }
    }
    samples
}
