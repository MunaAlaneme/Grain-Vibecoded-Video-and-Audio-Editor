// Expanded audio DSP module — ports of the JS phase vocoder / WSOLA / effects.
// Full real-time versions will replace these skeletons.

use std::f32::consts::PI;

/// Simple linear-interpolation pitch shift (speed changes with pitch).
pub fn pitch_shift_simple(samples: &[f32], semitones: f32) -> Vec<f32> {
    if semitones.abs() < 0.01 { return samples.to_vec(); }
    let factor = 2.0_f32.powf(semitones / 12.0);
    let out_len = ((samples.len() as f32) / factor).max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f32 * factor;
        let idx = src as usize;
        if idx + 1 >= samples.len() { break; }
        let frac = src - idx as f32;
        out.push(samples[idx] * (1.0 - frac) + samples[idx + 1] * frac);
    }
    out
}

/// Phase-vocoder pitch shift outline (STFT-based, formant-preserving possible).
/// TODO: replace with real FFT (rustfft) + phase accumulation from the JS FFT path.
pub fn pitch_shift_phase_vocoder(samples: &[f32], semitones: f32, fft_size: usize) -> Vec<f32> {
    // Skeleton: fall back for now so the command is usable.
    let _ = fft_size;
    pitch_shift_simple(samples, semitones)
}

/// WSOLA-style time stretch (pitch unchanged).
/// TODO: full similarity search + overlap-add from the JS WSOLA implementation.
pub fn time_stretch_wsola(samples: &[f32], factor: f32) -> Vec<f32> {
    if (factor - 1.0).abs() < 0.01 { return samples.to_vec(); }
    // Placeholder: simple resampling for now
    pitch_shift_simple(samples, 12.0 * factor.log2())
}

/// Vibrato (modulated pitch).
pub fn vibrato(samples: &[f32], rate_hz: f32, depth_semitones: f32, sample_rate: f32) -> Vec<f32> {
    let mut out = Vec::with_capacity(samples.len());
    for (i, &s) in samples.iter().enumerate() {
        let t = i as f32 / sample_rate;
        let mod_semi = depth_semitones * (2.0 * PI * rate_hz * t).sin();
        // Approximate instantaneous pitch shift
        let factor = 2.0_f32.powf(mod_semi / 12.0);
        let src = (i as f32 * factor) as usize;
        out.push(if src < samples.len() { samples[src] } else { 0.0 });
    }
    out
}

/// Bitcrush.
pub fn bitcrush(samples: &[f32], bits: u8, sample_rate_div: u32) -> Vec<f32> {
    let levels = 2u32.pow(bits as u32) as f32;
    let mut out = Vec::with_capacity(samples.len());
    let mut hold = 0.0;
    for (i, &s) in samples.iter().enumerate() {
        if i as u32 % sample_rate_div == 0 {
            hold = (s * levels).round() / levels;
        }
        out.push(hold);
    }
    out
}
