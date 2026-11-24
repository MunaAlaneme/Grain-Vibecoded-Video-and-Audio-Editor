// Simple pitch-shift sketch (phase-vocoder outline).
// Full implementation will replace the JS FFT / WSOLA paths.

use std::f32::consts::PI;

/// Naive pitch shift via resampling + optional overlap-add skeleton.
/// Real version should use a proper phase vocoder or Rubber Band / SoundTouch.
pub fn pitch_shift_simple(samples: &[f32], semitones: f32) -> Vec<f32> {
    if semitones.abs() < 0.01 {
        return samples.to_vec();
    }
    let factor = 2.0_f32.powf(semitones / 12.0);
    let out_len = (samples.len() as f32 / factor) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f32 * factor;
        let idx = src as usize;
        if idx + 1 >= samples.len() {
            break;
        }
        let frac = src - idx as f32;
        let s = samples[idx] * (1.0 - frac) + samples[idx + 1] * frac;
        out.push(s);
    }
    out
}

/// Placeholder for a real phase-vocoder pitch shift.
/// TODO: STFT → phase adjustment → ISTFT (see JS FFT path for reference).
pub fn pitch_shift_phase_vocoder(samples: &[f32], _semitones: f32, _fft_size: usize) -> Vec<f32> {
    // For now fall back to the simple method so the API is usable.
    pitch_shift_simple(samples, _semitones)
}
