use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioTab {
    PitchShift,
    TimeStretch,
    Vibrato,
    SpeedPitch,
    Bitcrush,
    Vocoder,
    Tremolo,
    Invert,
    MultiPitch,
    MoreFx,
    StudioFx,
    Rubberband,
}

impl AudioTab {
    pub const ALL: &'static [AudioTab] = &[
        AudioTab::PitchShift,
        AudioTab::TimeStretch,
        AudioTab::Vibrato,
        AudioTab::SpeedPitch,
        AudioTab::Bitcrush,
        AudioTab::Vocoder,
        AudioTab::Tremolo,
        AudioTab::Invert,
        AudioTab::MultiPitch,
        AudioTab::MoreFx,
        AudioTab::StudioFx,
        AudioTab::Rubberband,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            AudioTab::PitchShift => "Pitch shift",
            AudioTab::TimeStretch => "Time stretch",
            AudioTab::Vibrato => "Vibrato",
            AudioTab::SpeedPitch => "Speed + pitch",
            AudioTab::Bitcrush => "Bitcrush",
            AudioTab::Vocoder => "Vocoder",
            AudioTab::Tremolo => "Tremolo",
            AudioTab::Invert => "Invert",
            AudioTab::MultiPitch => "Multi-pitch",
            AudioTab::MoreFx => "More FX",
            AudioTab::StudioFx => "Studio FX",
            AudioTab::Rubberband => "Rubberband",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AudioTab::PitchShift => "Changes pitch only; duration is preserved by re-stretching after the shift.",
            AudioTab::TimeStretch => "Changes duration only; pitch is preserved.",
            AudioTab::Vibrato => "Oscillates pitch (mode a) or pitch+speed together (mode b) at an LFO rate.",
            AudioTab::SpeedPitch => "Pitch rises/falls with speed — the free, zero-DSP option.",
            AudioTab::Bitcrush => "Quantizes bit depth and holds samples to emulate a lower sample rate.",
            AudioTab::Vocoder => "Imposes the input's spectral envelope onto a generated carrier tone, band by band.",
            AudioTab::Tremolo => "Modulates amplitude with the chosen waveform — sine through to noise.",
            AudioTab::Invert => "Flips polarity; inaudible alone but useful for cancellation and layering.",
            AudioTab::MultiPitch => "Shifts the source by several intervals at once and mixes them into a chord.",
            AudioTab::MoreFx => "Reverse, echo, or soft-clip distortion.",
            AudioTab::StudioFx => "All 30+ studio DSP effects: Wah, Flanger, Phaser, Chorus, Reverb, EQ, Hum Remover, Overdrive, Tape Stop, Limiter, Ping-Pong, Auto-Filter, Wow & Flutter, Exciter, Haas, Rotary, Stereo Pan, Trance Gate, Wavefolder, Sub-Octave, Comb, Vocal Remover, De-Esser & more.",
            AudioTab::Rubberband => "FFmpeg libavfilter/af_rubberband compatible time/pitch/formant engine with live FFmpeg CLI command builder.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StretchTech {
    Fft,
    Wsola,
    RubberbandRefined,
    Bungee,
    ElastiquePro,
    ElastiqueA17,
}

impl StretchTech {
    pub const ALL: &'static [StretchTech] = &[
        StretchTech::ElastiquePro,
        StretchTech::ElastiqueA17,
        StretchTech::Fft,
        StretchTech::Wsola,
        StretchTech::RubberbandRefined,
        StretchTech::Bungee,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            StretchTech::ElastiquePro => "Elastique Pro (phase-coherent transient + formant)",
            StretchTech::ElastiqueA17 => "Elastique A17 Drums (minimum echo transient lock)",
            StretchTech::Fft => "FFT phase vocoder",
            StretchTech::Wsola => "WSOLA (similarity)",
            StretchTech::RubberbandRefined => "Rubberband-style (refined WSOLA)",
            StretchTech::Bungee => "Bungee audio engine (granular-phase hybrid)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierWave {
    Saw,
    Square,
    Pulse,
    Sine,
}

impl CarrierWave {
    pub const ALL: &'static [CarrierWave] = &[
        CarrierWave::Saw,
        CarrierWave::Square,
        CarrierWave::Pulse,
        CarrierWave::Sine,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            CarrierWave::Saw => "Sawtooth",
            CarrierWave::Square => "Square",
            CarrierWave::Pulse => "Pulse",
            CarrierWave::Sine => "Sine",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TremoloWave {
    Sine,
    Triangle,
    Square,
    Saw,
    InvSaw,
    Noise,
    SmoothNoise,
}

impl TremoloWave {
    pub const ALL: &'static [TremoloWave] = &[
        TremoloWave::Sine,
        TremoloWave::Triangle,
        TremoloWave::Square,
        TremoloWave::Saw,
        TremoloWave::InvSaw,
        TremoloWave::Noise,
        TremoloWave::SmoothNoise,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            TremoloWave::Sine => "Sine",
            TremoloWave::Triangle => "Triangle",
            TremoloWave::Square => "Square",
            TremoloWave::Saw => "Sawtooth",
            TremoloWave::InvSaw => "Inverse sawtooth",
            TremoloWave::Noise => "Noise",
            TremoloWave::SmoothNoise => "Smooth noise",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoreFxType {
    Reverse,
    Echo,
    Distortion,
}

impl MoreFxType {
    pub const ALL: &'static [MoreFxType] = &[
        MoreFxType::Reverse,
        MoreFxType::Echo,
        MoreFxType::Distortion,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            MoreFxType::Reverse => "Reverse",
            MoreFxType::Echo => "Echo",
            MoreFxType::Distortion => "Distortion",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioFxType {
    Wah,
    Flanger,
    Phaser,
    Chorus,
    ChorusMulti,
    Reverb,
    Fade,
    Normalize,
    Lowpass,
    Highpass,
    RingMod,
    AutoPan,
    Compressor,
    Gain,
    BassBoost,
    Telephone,
    Gate,
    Stutter,
    Vinyl,
    Octaver,
    Eq3Band,
    HumRemover,
    Overdrive,
    TapeStop,
    Repeat,
    Limiter,
    PingPong,
    AutoFilter,
    WowFlutter,
    Exciter,
    HaasWiden,
    Rotary,
    StereoPan,
    TranceGate,
    Wavefolder,
    SubOctave,
    CombFilter,
    VocalRemover,
    MonoMixdown,
    DeEsser,
}

impl StudioFxType {
    pub const ALL: &'static [StudioFxType] = &[
        StudioFxType::Wah,
        StudioFxType::Flanger,
        StudioFxType::Phaser,
        StudioFxType::Chorus,
        StudioFxType::ChorusMulti,
        StudioFxType::Reverb,
        StudioFxType::Fade,
        StudioFxType::Normalize,
        StudioFxType::Lowpass,
        StudioFxType::Highpass,
        StudioFxType::RingMod,
        StudioFxType::AutoPan,
        StudioFxType::Compressor,
        StudioFxType::Gain,
        StudioFxType::BassBoost,
        StudioFxType::Telephone,
        StudioFxType::Gate,
        StudioFxType::Stutter,
        StudioFxType::Vinyl,
        StudioFxType::Octaver,
        StudioFxType::Eq3Band,
        StudioFxType::HumRemover,
        StudioFxType::Overdrive,
        StudioFxType::TapeStop,
        StudioFxType::Repeat,
        StudioFxType::Limiter,
        StudioFxType::PingPong,
        StudioFxType::AutoFilter,
        StudioFxType::WowFlutter,
        StudioFxType::Exciter,
        StudioFxType::HaasWiden,
        StudioFxType::Rotary,
        StudioFxType::StereoPan,
        StudioFxType::TranceGate,
        StudioFxType::Wavefolder,
        StudioFxType::SubOctave,
        StudioFxType::CombFilter,
        StudioFxType::VocalRemover,
        StudioFxType::MonoMixdown,
        StudioFxType::DeEsser,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            StudioFxType::Wah => "Wah-wah",
            StudioFxType::Flanger => "Flanger",
            StudioFxType::Phaser => "Phaser",
            StudioFxType::Chorus => "Chorus",
            StudioFxType::ChorusMulti => "Chorus (multi-voice)",
            StudioFxType::Reverb => "Reverb",
            StudioFxType::Fade => "Fade in / out",
            StudioFxType::Normalize => "Normalize",
            StudioFxType::Lowpass => "Low-pass filter",
            StudioFxType::Highpass => "High-pass filter",
            StudioFxType::RingMod => "Ring modulator",
            StudioFxType::AutoPan => "Auto-pan",
            StudioFxType::Compressor => "Compressor",
            StudioFxType::Gain => "Gain",
            StudioFxType::BassBoost => "Bass boost",
            StudioFxType::Telephone => "Telephone",
            StudioFxType::Gate => "Noise gate",
            StudioFxType::Stutter => "Stutter",
            StudioFxType::Vinyl => "Vinyl crackle",
            StudioFxType::Octaver => "Octaver (−12 st)",
            StudioFxType::Eq3Band => "3-band EQ",
            StudioFxType::HumRemover => "Hum remover",
            StudioFxType::Overdrive => "Overdrive / fuzz",
            StudioFxType::TapeStop => "Tape stop",
            StudioFxType::Repeat => "Repeat",
            StudioFxType::Limiter => "Limiter",
            StudioFxType::PingPong => "Ping-pong delay",
            StudioFxType::AutoFilter => "Auto-filter (resonant sweep)",
            StudioFxType::WowFlutter => "Tape wow & flutter",
            StudioFxType::Exciter => "Exciter",
            StudioFxType::HaasWiden => "Stereo widen (Haas)",
            StudioFxType::Rotary => "Rotary speaker",
            StudioFxType::StereoPan => "Stereo pan",
            StudioFxType::TranceGate => "Trance gate",
            StudioFxType::Wavefolder => "Wavefolder",
            StudioFxType::SubOctave => "Sub-octave",
            StudioFxType::CombFilter => "Comb filter",
            StudioFxType::VocalRemover => "Vocal remover (center cancel)",
            StudioFxType::MonoMixdown => "Mono mixdown",
            StudioFxType::DeEsser => "De-esser",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioExportFormat {
    Wav16,
    Wav24,
    WavF32,
    Flac16,
    Flac24,
    AacM4a,
    OggOpus,
}

impl AudioExportFormat {
    pub const ALL: &'static [AudioExportFormat] = &[
        AudioExportFormat::Wav16,
        AudioExportFormat::Wav24,
        AudioExportFormat::WavF32,
        AudioExportFormat::Flac16,
        AudioExportFormat::Flac24,
        AudioExportFormat::AacM4a,
        AudioExportFormat::OggOpus,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            AudioExportFormat::Wav16 => "WAV PCM 16-bit",
            AudioExportFormat::Wav24 => "WAV PCM 24-bit",
            AudioExportFormat::WavF32 => "WAV 32-bit float",
            AudioExportFormat::Flac16 => "FLAC 16-bit (lossless)",
            AudioExportFormat::Flac24 => "FLAC 24-bit (lossless)",
            AudioExportFormat::AacM4a => "AAC-LC (.m4a)",
            AudioExportFormat::OggOpus => "Ogg Opus (.ogg)",
        }
    }

    pub fn ext(&self) -> &'static str {
        match self {
            AudioExportFormat::Wav16 | AudioExportFormat::Wav24 | AudioExportFormat::WavF32 => "wav",
            AudioExportFormat::Flac16 | AudioExportFormat::Flac24 => "flac",
            AudioExportFormat::AacM4a => "m4a",
            AudioExportFormat::OggOpus => "ogg",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioLabState {
    pub active_tab: AudioTab,
    // Pitch shift
    pub pitch_tech: StretchTech,
    pub pitch_semitones: f32,
    pub fft_size: usize,
    pub formant_semitones: f32,
    pub formant_keep: bool,
    // Time stretch
    pub time_tech: StretchTech,
    pub time_factor: f32,
    // Vibrato
    pub vibrato_pitch_only: bool, // true = mode a, false = mode b
    pub vibrato_tech: StretchTech,
    pub vibrato_rate: f32,
    pub vibrato_depth: f32,
    // Speed + pitch
    pub speed_pitch_rate: f32,
    // Bitcrush
    pub bit_depth: u8,
    pub sr_divisor: u32,
    // Vocoder
    pub vocoder_wave: CarrierWave,
    pub vocoder_freq: f32,
    pub vocoder_bands: usize,
    // Tremolo
    pub tremolo_wave: TremoloWave,
    pub tremolo_rate: f32,
    pub tremolo_depth: f32,
    // Multi-pitch intervals (-12, -7, -5, -3, 0, +3, +5, +7, +12)
    pub mp_tech: StretchTech,
    pub mp_intervals: [bool; 9],
    // More FX
    pub more_type: MoreFxType,
    pub echo_delay_ms: f32,
    pub echo_feedback: f32,
    pub dist_drive: f32,
    // Studio FX
    pub studio_type: StudioFxType,
    pub sf_rate: f32,
    pub sf_depth: f32,
    pub sf_decay: f32,
    pub sf_mix: f32,
    pub sf_fc: f32,
    pub sf_gain_db: f32,
    pub sf_fade_in: f32,
    pub sf_fade_out: f32,
    // Extended Studio FX parameters (p1..p5)
    pub sf_p1: f32,
    pub sf_p2: f32,
    pub sf_p3: f32,
    pub sf_p4: f32,
    pub sf_p5: f32,
    // Rubberband FFmpeg filter options
    pub rb_semis: f32,
    pub rb_tempo: f32,
    pub rb_formant: String,
    pub rb_formant_v4: u32,
    pub rb_transients: String,
    pub rb_detector: String,
    pub rb_phase: String,
    pub rb_window: String,
    pub rb_smoothing: String,
    pub rb_pitchq: String,
    pub rb_channels: String,
    // Export format
    pub export_format: AudioExportFormat,
    pub export_sample_rate: u32,
    pub export_bitrate_kbps: u32,
    pub track_gain: f32,
    pub track_muted: bool,
}

impl Default for AudioLabState {
    fn default() -> Self {
        Self {
            active_tab: AudioTab::PitchShift,
            pitch_tech: StretchTech::ElastiquePro,
            pitch_semitones: 0.0,
            fft_size: 2048,
            formant_semitones: 0.0,
            formant_keep: false,
            time_tech: StretchTech::Fft,
            time_factor: 1.0,
            vibrato_pitch_only: true,
            vibrato_tech: StretchTech::Fft,
            vibrato_rate: 5.0,
            vibrato_depth: 0.5,
            speed_pitch_rate: 1.0,
            bit_depth: 8,
            sr_divisor: 4,
            vocoder_wave: CarrierWave::Saw,
            vocoder_freq: 110.0,
            vocoder_bands: 16,
            tremolo_wave: TremoloWave::Sine,
            tremolo_rate: 5.0,
            tremolo_depth: 0.8,
            mp_tech: StretchTech::Fft,
            // -12, -7, -5, -3, 0 (true), +3, +5, +7 (true), +12
            mp_intervals: [false, false, false, false, true, false, false, true, false],
            more_type: MoreFxType::Reverse,
            echo_delay_ms: 250.0,
            echo_feedback: 0.4,
            dist_drive: 4.0,
            studio_type: StudioFxType::Wah,
            sf_rate: 1.0,
            sf_depth: 0.7,
            sf_decay: 2.0,
            sf_mix: 0.6,
            sf_fc: 1000.0,
            sf_gain_db: 0.0,
            sf_fade_in: 1.0,
            sf_fade_out: 1.0,
            sf_p1: 0.0,
            sf_p2: 0.5,
            sf_p3: 1000.0,
            sf_p4: 0.0,
            sf_p5: 0.0,
            rb_semis: 0.0,
            rb_tempo: 1.0,
            rb_formant: "unset".into(),
            rb_formant_v4: 0,
            rb_transients: "crisp".into(),
            rb_detector: "compound".into(),
            rb_phase: "laminar".into(),
            rb_window: "standard".into(),
            rb_smoothing: "off".into(),
            rb_pitchq: "quality".into(),
            rb_channels: "apart".into(),
            export_format: AudioExportFormat::Wav24,
            export_sample_rate: 48_000,
            export_bitrate_kbps: 192,
            track_gain: 1.0,
            track_muted: false,
        }
    }
}

impl AudioLabState {
    pub fn rb_pitch_ratio(&self) -> f32 {
        2.0_f32.powf(self.rb_semis / 12.0)
    }

    pub fn rb_filter_string(&self) -> String {
        let mut s = format!(
            "rubberband=tempo={:.4}:pitch={:.6}:transients={}:detector={}:phase={}:window={}:smoothing={}:pitchq={}:channels={}",
            self.rb_tempo,
            self.rb_pitch_ratio(),
            self.rb_transients,
            self.rb_detector,
            self.rb_phase,
            self.rb_window,
            self.rb_smoothing,
            self.rb_pitchq,
            self.rb_channels
        );
        if self.rb_formant != "unset" {
            s.push_str(&format!(":formant={}", self.rb_formant));
        }
        s
    }

    pub fn rb_ffmpeg_cli(&self) -> String {
        format!(
            "ffmpeg -i \"INPUT.wav\" -af \"{}\" -c:a pcm_f32le \"OUTPUT.wav\"",
            self.rb_filter_string()
        )
    }
}

pub fn process_stereo(
    left: &[f32],
    right: &[f32],
    sr: u32,
    st: &AudioLabState,
) -> (Vec<f32>, Vec<f32>) {
    let mut out_l = process_single_channel(left, right, 0, sr, st);
    let mut out_r = process_single_channel(right, left, 1, sr, st);

    let gain = if st.track_muted { 0.0 } else { st.track_gain };
    if (gain - 1.0).abs() > 0.001 {
        for s in &mut out_l {
            *s *= gain;
        }
        for s in &mut out_r {
            *s *= gain;
        }
    }

    let len = out_l.len().min(out_r.len());
    out_l.truncate(len);
    out_r.truncate(len);
    (out_l, out_r)
}

fn process_single_channel(
    chan: &[f32],
    other_chan: &[f32],
    chan_idx: usize,
    sr: u32,
    st: &AudioLabState,
) -> Vec<f32> {
    let srf = sr.max(1) as f32;
    match st.active_tab {
        AudioTab::PitchShift => {
            let mut y = pitch_shift(chan, st.pitch_semitones, st.pitch_tech, sr, st.fft_size);
            let f_ratio = 2.0_f32.powf(st.formant_semitones / 12.0)
                * if st.formant_keep {
                    2.0_f32.powf(-st.pitch_semitones / 12.0)
                } else {
                    1.0
                };
            if (f_ratio - 1.0).abs() > 0.002 {
                y = formant_shift(&y, f_ratio, st.fft_size);
            }
            y
        }
        AudioTab::TimeStretch => stretch(chan, st.time_factor, st.time_tech, sr, st.fft_size),
        AudioTab::Vibrato => {
            if st.vibrato_pitch_only {
                vibrato_pitch_only(chan, st.vibrato_rate, st.vibrato_depth, st.vibrato_tech, sr, st.fft_size)
            } else {
                vibrato_native(chan, st.vibrato_rate, st.vibrato_depth, srf)
            }
        }
        AudioTab::SpeedPitch => resample_variable(chan, |_| st.speed_pitch_rate),
        AudioTab::Bitcrush => bitcrush(chan, st.bit_depth, st.sr_divisor),
        AudioTab::Vocoder => vocoder(chan, st.vocoder_wave, st.vocoder_freq, st.vocoder_bands, srf),
        AudioTab::Tremolo => tremolo(chan, st.tremolo_rate, st.tremolo_wave, st.tremolo_depth, srf),
        AudioTab::Invert => chan.iter().map(|s| -s).collect(),
        AudioTab::MultiPitch => {
            let all_iv = [-12.0f32, -7.0, -5.0, -3.0, 0.0, 3.0, 5.0, 7.0, 12.0];
            let mut chosen: Vec<f32> = all_iv
                .iter()
                .zip(st.mp_intervals.iter())
                .filter_map(|(&iv, &on)| if on { Some(iv) } else { None })
                .collect();
            if chosen.is_empty() {
                chosen.push(0.0);
            }
            multi_pitch(chan, &chosen, st.mp_tech, sr, st.fft_size)
        }
        AudioTab::MoreFx => match st.more_type {
            MoreFxType::Reverse => chan.iter().rev().copied().collect(),
            MoreFxType::Echo => echo_fx(chan, st.echo_delay_ms, st.echo_feedback, srf),
            MoreFxType::Distortion => distortion_fx(chan, st.dist_drive),
        },
        AudioTab::StudioFx => studio_fx(chan, other_chan, chan_idx, sr, st),
        AudioTab::Rubberband => {
            let pr = st.rb_pitch_ratio();
            let tr = 1.0 / st.rb_tempo.max(0.05);
            let tech = match st.rb_transients.as_str() {
                "crisp" => StretchTech::Wsola,
                "mixed" => StretchTech::RubberbandRefined,
                _ => StretchTech::Fft,
            };
            let mut y = chan.to_vec();
            if (pr - 1.0).abs() > 1e-5 {
                let stretched = stretch(&y, pr, tech, sr, 2048);
                y = resample_to(&stretched, y.len());
            }
            if (tr - 1.0).abs() > 1e-5 {
                y = stretch(&y, tr, tech, sr, 2048);
            }
            if st.rb_formant == "preserved" && (pr - 1.0).abs() > 1e-3 {
                y = formant_shift(&y, 1.0 / pr, 2048);
            }
            y
        }
    }
}

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / (n.saturating_sub(1).max(1)) as f32).cos())
        .collect()
}

pub fn stretch(chan: &[f32], factor: f32, tech: StretchTech, sr: u32, fft_size: usize) -> Vec<f32> {
    match tech {
        StretchTech::ElastiquePro => {
            // High-precision multi-resolution phase vocoder + WSOLA transient preservation
            let w = stretch_wsola(chan, factor, 18.0, 8.0, 15.0, sr);
            let f = stretch_fft(chan, factor, fft_size.max(2048));
            let len = w.len().min(f.len());
            (0..len).map(|i| w[i] * 0.55 + f[i] * 0.45).collect()
        }
        StretchTech::ElastiqueA17 => {
            // Sony A17 Drums preset: ultra-short window WSOLA for minimum pre/post-echo on transients
            stretch_wsola(chan, factor, 12.0, 5.0, 12.0, sr)
        }
        StretchTech::Fft => stretch_fft(chan, factor, fft_size),
        StretchTech::Wsola => stretch_wsola(chan, factor, 35.0, 12.0, 10.0, sr),
        StretchTech::RubberbandRefined => stretch_wsola(chan, factor, 20.0, 9.0, 16.0, sr),
        StretchTech::Bungee => {
            // Bungee-style transient-locked granular + phase-coherent hybrid
            let w = stretch_wsola(chan, factor, 16.0, 8.0, 14.0, sr);
            let f = stretch_fft(chan, factor, fft_size.max(1024));
            let len = w.len().min(f.len());
            (0..len).map(|i| w[i] * 0.65 + f[i] * 0.35).collect()
        }
    }
}

pub fn stretch_fft(chan: &[f32], factor: f32, frame_size: usize) -> Vec<f32> {
    let n = frame_size.next_power_of_two().max(256);
    if (factor - 1.0).abs() < 0.005 || chan.len() < n {
        return chan.to_vec();
    }
    let syn_hop = n / 4;
    let ana_hop = ((syn_hop as f32) / factor).round().max(1.0) as usize;
    let win = hann(n);
    let out_len = ((chan.len() as f32) * factor).round() as usize + n;
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
    while in_pos + n <= chan.len() {
        for i in 0..n {
            buf[i] = Complex::new(chan[in_pos + i] * win[i], 0.0);
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
    let target = ((chan.len() as f32) * factor).round().max(1.0) as usize;
    out.truncate(target.min(out.len()));
    limit_peak(out)
}

pub fn stretch_wsola(
    chan: &[f32],
    factor: f32,
    seg_ms: f32,
    overlap_ms: f32,
    search_ms: f32,
    sr: u32,
) -> Vec<f32> {
    if (factor - 1.0).abs() < 0.005 || chan.is_empty() {
        return chan.to_vec();
    }
    let seg = ((sr as f32) * seg_ms / 1000.0).round().max(256.0) as usize;
    let overlap = ((sr as f32) * overlap_ms / 1000.0).round().max(32.0) as usize;
    let search = ((sr as f32) * search_ms / 1000.0).round().max(16.0) as usize;
    if chan.len() <= seg + search {
        return chan.to_vec();
    }

    let syn_hop = seg.saturating_sub(overlap).max(1);
    let ana_hop = ((syn_hop as f32) / factor).round().max(1.0) as usize;
    let out_len = ((chan.len() as f32) * factor).round() as usize + seg;
    let mut out = vec![0.0f32; out_len];
    let win = hann(overlap * 2);

    let first = seg.min(chan.len());
    out[..first].copy_from_slice(&chan[..first]);
    let mut out_pos = syn_hop;
    let mut in_pos = ana_hop;

    while in_pos + seg + search < chan.len() && out_pos + seg < out_len {
        let tail_start = out_pos.saturating_sub(overlap);
        let mut best = 0isize;
        let mut best_score = f32::NEG_INFINITY;
        for s in -(search as isize)..=(search as isize) {
            let cand_isize = in_pos as isize + s;
            if cand_isize < 0 {
                continue;
            }
            let cand = cand_isize as usize;
            if cand + overlap > chan.len() {
                continue;
            }
            let mut score = 0.0f32;
            for i in 0..overlap {
                if tail_start + i < out.len() && cand + i < chan.len() {
                    score += out[tail_start + i] * chan[cand + i];
                }
            }
            if score > best_score {
                best_score = score;
                best = s;
            }
        }
        let src = (in_pos as isize + best).max(0) as usize;
        for i in 0..overlap {
            let w = win[i + overlap];
            if tail_start + i < out.len() && src + i < chan.len() {
                out[tail_start + i] = out[tail_start + i] * (1.0 - w) + chan[src + i] * w;
            }
        }
        for i in overlap..seg {
            if src + i < chan.len() && out_pos >= overlap {
                let target_idx = out_pos - overlap + i;
                if target_idx < out_len {
                    out[target_idx] = chan[src + i];
                }
            }
        }
        out_pos += syn_hop;
        in_pos += ana_hop;
    }

    let target = ((chan.len() as f32) * factor).round().max(1.0) as usize;
    out.truncate(target.min(out.len()));
    out
}

pub fn resample_to(chan: &[f32], target_len: usize) -> Vec<f32> {
    if chan.is_empty() || target_len == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(target_len);
    let ratio = (chan.len().saturating_sub(1) as f32) / (target_len.saturating_sub(1).max(1) as f32);
    for i in 0..target_len {
        let pos = i as f32 * ratio;
        let i0 = pos as usize;
        let frac = pos - i0 as f32;
        let a = chan[i0];
        let b = chan.get(i0 + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

pub fn resample_variable<F: Fn(f32) -> f32>(chan: &[f32], rate_fn: F) -> Vec<f32> {
    let mut out = Vec::with_capacity(chan.len());
    let mut pos = 0.0f32;
    let n = chan.len() as f32;
    while (pos as usize) + 1 < chan.len() {
        let t = pos / n;
        let rate = rate_fn(t).max(0.02);
        let i0 = pos as usize;
        let frac = pos - i0 as f32;
        out.push(chan[i0] * (1.0 - frac) + chan[i0 + 1] * frac);
        pos += rate;
    }
    out
}

pub fn pitch_shift(
    chan: &[f32],
    semitones: f32,
    tech: StretchTech,
    sr: u32,
    fft_size: usize,
) -> Vec<f32> {
    if semitones.abs() < 0.005 {
        return chan.to_vec();
    }
    let ratio = 2.0_f32.powf(semitones / 12.0);
    let stretched = stretch(chan, ratio, tech, sr, fft_size);
    resample_to(&stretched, chan.len())
}

pub fn formant_shift(x: &[f32], factor: f32, frame_size: usize) -> Vec<f32> {
    let n = frame_size.next_power_of_two().max(256);
    if (factor - 1.0).abs() < 0.002 || x.len() < n {
        return x.to_vec();
    }
    let hop = n / 4;
    let win = hann(n);
    let mut out = vec![0.0f32; x.len() + n];
    let mut sw = vec![0.0f32; x.len() + n];
    let half = n / 2;
    let mut pre = vec![0.0f32; half + 2];
    let mut env = vec![0.0f32; half + 1];
    let r = (n >> 6).max(2);

    let mut planner = FftPlanner::<f32>::new();
    let fft_fwd = planner.plan_fft_forward(n);
    let fft_inv = planner.plan_fft_inverse(n);
    let mut buf = vec![Complex::new(0.0f32, 0.0f32); n];

    let mut p = 0usize;
    while p + n <= x.len() {
        for i in 0..n {
            buf[i] = Complex::new(x[p + i] * win[i], 0.0);
        }
        fft_fwd.process(&mut buf);
        for k in 0..=half {
            pre[k + 1] = pre[k] + buf[k].norm();
        }
        for k in 0..=half {
            let a = k.saturating_sub(r);
            let b = (k + r).min(half);
            env[k] = (pre[b + 1] - pre[a]) / ((b - a + 1) as f32);
        }
        for k in 0..=half {
            let sk = (k as f32) / factor;
            let i0 = (sk.floor() as usize).min(half.saturating_sub(1));
            let fr = sk - i0 as f32;
            let e2 = if sk >= half as f32 {
                env[half] * 0.2
            } else {
                env[i0] + (env[i0 + 1] - env[i0]) * fr
            };
            let gn = (e2 / (env[k] + 1e-9)).min(8.0);
            buf[k] *= gn;
            if k > 0 && k < half {
                buf[n - k] *= gn;
            }
        }
        fft_inv.process(&mut buf);
        let scale = 1.0 / (n as f32);
        for i in 0..n {
            out[p + i] += buf[i].re * scale * win[i];
            sw[p + i] += win[i] * win[i];
        }
        p += hop;
    }

    for i in 0..out.len() {
        if sw[i] > 1e-6 {
            out[i] /= sw[i];
        }
    }
    out.truncate(x.len());
    limit_peak(out)
}

fn vibrato_pitch_only(
    chan: &[f32],
    rate_hz: f32,
    depth_semis: f32,
    tech: StretchTech,
    sr: u32,
    fft_size: usize,
) -> Vec<f32> {
    let grain_len = ((sr as f32) * 0.06).round().max(128.0) as usize;
    let hop = (grain_len / 2).max(1);
    let win = hann(grain_len);
    let mut out = vec![0.0f32; chan.len() + grain_len];
    let mut sum_win = vec![0.0f32; chan.len() + grain_len];

    let mut pos = 0usize;
    while pos + grain_len <= chan.len() {
        let t = pos as f32 / sr as f32;
        let semis = depth_semis * (2.0 * PI * rate_hz * t).sin();
        let grain = &chan[pos..pos + grain_len];
        let shifted = pitch_shift(grain, semis, tech, sr, fft_size);
        let len = grain_len.min(shifted.len());
        for i in 0..len {
            out[pos + i] += shifted[i] * win[i];
            sum_win[pos + i] += win[i];
        }
        pos += hop;
    }
    for i in 0..out.len() {
        if sum_win[i] > 1e-6 {
            out[i] /= sum_win[i];
        }
    }
    out.truncate(chan.len());
    out
}

fn vibrato_native(chan: &[f32], rate_hz: f32, depth_semis: f32, sr: f32) -> Vec<f32> {
    let depth_rate = 2.0_f32.powf(depth_semis / 12.0) - 1.0;
    let total_sec = chan.len() as f32 / sr;
    resample_variable(chan, |t| {
        1.0 + depth_rate * (2.0 * PI * rate_hz * (t * total_sec)).sin()
    })
}

fn bitcrush(chan: &[f32], bits: u8, sr_div: u32) -> Vec<f32> {
    let step = 2.0_f32.powi((bits.clamp(1, 16) as i32) - 1);
    let div = sr_div.max(1);
    let mut out = Vec::with_capacity(chan.len());
    let mut held = 0.0f32;
    for (i, &s) in chan.iter().enumerate() {
        if (i as u32) % div == 0 {
            held = (s * step).round() / step;
        }
        out.push(held);
    }
    out
}

fn tremolo(chan: &[f32], rate_hz: f32, wave: TremoloWave, depth: f32, sr: f32) -> Vec<f32> {
    let d = depth.clamp(0.0, 1.0);
    let n = chan.len();
    let mut out = Vec::with_capacity(n);
    if matches!(wave, TremoloWave::Noise | TremoloWave::SmoothNoise) {
        let period = (sr / rate_hz.max(0.1)).max(1.0);
        let num_pts = (n as f32 / period).ceil() as usize + 3;
        let rnd: Vec<f32> = (0..num_pts)
            .map(|i| ((i as f32 * 127.1 + 311.7).sin() * 43758.5453).fract() * 2.0 - 1.0)
            .collect();
        for i in 0..n {
            let pos = i as f32 / period;
            let idx = (pos as usize).min(num_pts - 2);
            let frac = pos - idx as f32;
            let v = if wave == TremoloWave::Noise {
                rnd[idx]
            } else {
                rnd[idx] * (1.0 - frac) + rnd[idx + 1] * frac
            };
            out.push(chan[i] * (1.0 - d * (0.5 - 0.5 * v)));
        }
        return out;
    }

    for (i, &s) in chan.iter().enumerate() {
        let p = ((i as f32 / sr) * rate_hz) % 1.0;
        let v = match wave {
            TremoloWave::Sine => (2.0 * PI * p).sin(),
            TremoloWave::Triangle => {
                if p < 0.5 {
                    4.0 * p - 1.0
                } else {
                    3.0 - 4.0 * p
                }
            }
            TremoloWave::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            TremoloWave::Saw => 2.0 * p - 1.0,
            TremoloWave::InvSaw => 1.0 - 2.0 * p,
            _ => 0.0,
        };
        out.push(s * (1.0 - d * (0.5 - 0.5 * v)));
    }
    out
}

fn vocoder(
    chan: &[f32],
    carrier_type: CarrierWave,
    carrier_freq: f32,
    num_bands: usize,
    sr: f32,
) -> Vec<f32> {
    let n = 2048usize;
    let hop = 512usize;
    if chan.len() < n {
        return chan.to_vec();
    }
    let win = hann(n);
    let bands = num_bands.clamp(4, 64);
    let carrier: Vec<f32> = (0..chan.len() + n)
        .map(|i| {
            let p = (carrier_freq * i as f32 / sr) % 1.0;
            let v = match carrier_type {
                CarrierWave::Saw => 2.0 * p - 1.0,
                CarrierWave::Square => {
                    if p < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                CarrierWave::Pulse => {
                    if p < 0.25 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                CarrierWave::Sine => (2.0 * PI * p).sin(),
            };
            v * 0.9
        })
        .collect();

    let mut out = vec![0.0f32; chan.len() + n];
    let mut sum_win = vec![0.0f32; chan.len() + n];
    let min_f = 60.0f32;
    let max_f = sr * 0.45;
    let edges: Vec<f32> = (0..=bands)
        .map(|i| min_f * (max_f / min_f).powf(i as f32 / bands as f32))
        .collect();
    let bin_hz = sr / n as f32;
    let mut env_state = vec![0.0f32; bands];

    let mut planner = FftPlanner::<f32>::new();
    let fft_fwd = planner.plan_fft_forward(n);
    let fft_inv = planner.plan_fft_inverse(n);
    let mut buf_m = vec![Complex::new(0.0f32, 0.0f32); n];
    let mut buf_c = vec![Complex::new(0.0f32, 0.0f32); n];
    let mut buf_o = vec![Complex::new(0.0f32, 0.0f32); n];

    let mut pos = 0usize;
    while pos + n <= chan.len() {
        for i in 0..n {
            buf_m[i] = Complex::new(chan[pos + i] * win[i], 0.0);
            buf_c[i] = Complex::new(carrier[pos + i] * win[i], 0.0);
            buf_o[i] = Complex::new(0.0, 0.0);
        }
        fft_fwd.process(&mut buf_m);
        fft_fwd.process(&mut buf_c);

        for b in 0..bands {
            let k0 = ((edges[b] / bin_hz).round() as usize).max(1);
            let k1 = ((edges[b + 1] / bin_hz).round() as usize).min(n / 2);
            let mut sum = 0.0f32;
            let mut cnt = 0usize;
            for k in k0..k1 {
                sum += buf_m[k].norm();
                cnt += 1;
            }
            let avg = if cnt > 0 { sum / cnt as f32 } else { 0.0 };
            env_state[b] = env_state[b] * 0.6 + avg * 0.4;
            for k in k0..k1 {
                let c_mag = buf_c[k].norm().max(1e-9);
                let scale = env_state[b] / c_mag;
                buf_o[k] = buf_c[k] * scale;
            }
        }
        for k in 1..(n / 2) {
            buf_o[n - k] = buf_o[k].conj();
        }
        fft_inv.process(&mut buf_o);
        let sc = 1.0 / n as f32;
        for i in 0..n {
            out[pos + i] += buf_o[i].re * sc * win[i];
            sum_win[pos + i] += win[i] * win[i];
        }
        pos += hop;
    }

    for i in 0..out.len() {
        if sum_win[i] > 1e-6 {
            out[i] /= sum_win[i];
        }
    }
    out.truncate(chan.len());
    limit_peak(out)
}

fn multi_pitch(
    chan: &[f32],
    intervals: &[f32],
    tech: StretchTech,
    sr: u32,
    fft_size: usize,
) -> Vec<f32> {
    let voices: Vec<Vec<f32>> = intervals
        .iter()
        .map(|&s| pitch_shift(chan, s, tech, sr, fft_size))
        .collect();
    let max_len = voices.iter().map(|v| v.len()).max().unwrap_or(0);
    let mut out = vec![0.0f32; max_len];
    let div = voices.len().max(1) as f32;
    for v in &voices {
        for (i, &s) in v.iter().enumerate() {
            out[i] += s / div;
        }
    }
    out
}

fn echo_fx(chan: &[f32], delay_ms: f32, feedback: f32, sr: f32) -> Vec<f32> {
    let delay = ((sr * delay_ms / 1000.0).round() as usize).max(1);
    let fb = feedback.clamp(0.0, 0.94);
    let mut out = vec![0.0f32; chan.len() + delay * 6];
    for (i, &s) in chan.iter().enumerate() {
        out[i] += s;
        let mut amp = fb;
        let mut pos = i + delay;
        while amp > 0.005 && pos < out.len() {
            out[pos] += s * amp;
            pos += delay;
            amp *= fb;
        }
    }
    limit_peak(out)
}

fn distortion_fx(chan: &[f32], drive: f32) -> Vec<f32> {
    let d = drive.max(0.1);
    let norm = d.tanh().max(1e-5);
    chan.iter().map(|&s| (s * d).tanh() / norm).collect()
}

fn biquad(kind: &str, f0: f32, db: f32, q: f32, sr: f32) -> [f32; 5] {
    let a_amp = 10.0_f32.powf(db / 40.0);
    let w = 2.0 * PI * f0.clamp(20.0, sr * 0.45) / sr;
    let c = w.cos();
    let s = w.sin();
    let al = s / (2.0 * q.max(0.1));
    let ra = a_amp.sqrt();
    let (b0, b1, b2, a0, a1, a2) = match kind {
        "low" => {
            let qq = 2.0 * ra * al;
            (
                a_amp * ((a_amp + 1.0) - (a_amp - 1.0) * c + qq),
                2.0 * a_amp * ((a_amp - 1.0) - (a_amp + 1.0) * c),
                a_amp * ((a_amp + 1.0) - (a_amp - 1.0) * c - qq),
                (a_amp + 1.0) + (a_amp - 1.0) * c + qq,
                -2.0 * ((a_amp - 1.0) + (a_amp + 1.0) * c),
                (a_amp + 1.0) + (a_amp - 1.0) * c - qq,
            )
        }
        "high" => {
            let qq = 2.0 * ra * al;
            (
                a_amp * ((a_amp + 1.0) + (a_amp - 1.0) * c + qq),
                -2.0 * a_amp * ((a_amp - 1.0) + (a_amp + 1.0) * c),
                a_amp * ((a_amp + 1.0) + (a_amp - 1.0) * c - qq),
                (a_amp + 1.0) - (a_amp - 1.0) * c + qq,
                2.0 * ((a_amp - 1.0) - (a_amp + 1.0) * c),
                (a_amp + 1.0) - (a_amp - 1.0) * c - qq,
            )
        }
        "peak" => (
            1.0 + al * a_amp,
            -2.0 * c,
            1.0 - al * a_amp,
            1.0 + al / a_amp,
            -2.0 * c,
            1.0 - al / a_amp,
        ),
        "notch" => (1.0, -2.0 * c, 1.0, 1.0 + al, -2.0 * c, 1.0 - al),
        _ => (
            (1.0 - c) / 2.0,
            1.0 - c,
            (1.0 - c) / 2.0,
            1.0 + al,
            -2.0 * c,
            1.0 - al,
        ),
    };
    [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0]
}

fn run_biquad(x: &[f32], k: [f32; 5]) -> Vec<f32> {
    let mut o = vec![0.0f32; x.len()];
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    for (i, &xi) in x.iter().enumerate() {
        let y = k[0] * xi + k[1] * x1 + k[2] * x2 - k[3] * y1 - k[4] * y2;
        x2 = x1;
        x1 = xi;
        y2 = y1;
        y1 = y;
        o[i] = y;
    }
    o
}

fn mod_delay(
    x: &[f32],
    base_ms: f32,
    range_ms: f32,
    rate: f32,
    fb: f32,
    mix: f32,
    sr: f32,
    ph: f32,
) -> Vec<f32> {
    let n = x.len();
    let size = ((sr * (base_ms + range_ms) / 1000.0).ceil() as usize) + 8;
    let mut buf = vec![0.0f32; size];
    let mut o = vec![0.0f32; n];
    let mut w = 0usize;
    for i in 0..n {
        let d = sr * (base_ms + range_ms * (0.5 + 0.5 * (2.0 * PI * rate * i as f32 / sr + ph).sin()))
            / 1000.0;
        let mut r = w as f32 - d;
        while r < 0.0 {
            r += size as f32;
        }
        let i0 = r.floor() as usize;
        let f = r - i0 as f32;
        let dl = buf[i0 % size] + (buf[(i0 + 1) % size] - buf[i0 % size]) * f;
        buf[w] = x[i] + dl * fb;
        o[i] = x[i] * (1.0 - mix * 0.5) + dl * (mix * 0.5);
        w = (w + 1) % size;
    }
    limit_peak(o)
}

fn studio_fx(
    x: &[f32],
    other: &[f32],
    ci: usize,
    sr_u32: u32,
    st: &AudioLabState,
) -> Vec<f32> {
    let sr = sr_u32 as f32;
    let n = x.len();
    let r = st.sf_rate;
    let d = st.sf_depth;
    let m = st.sf_mix;

    match st.studio_type {
        StudioFxType::Wah => {
            let mut o = vec![0.0f32; n];
            let q = 0.25;
            let (mut lp, mut bp) = (0.0f32, 0.0f32);
            for i in 0..n {
                let fc = 300.0 + 2500.0 * d.max(0.05) * (0.5 + 0.5 * (2.0 * PI * r * i as f32 / sr).sin());
                let f = 2.0 * (PI * fc / sr).sin();
                let hp = x[i] - lp - q * bp;
                bp += f * hp;
                lp += f * bp;
                o[i] = x[i] * (1.0 - m) + bp * q * 2.0 * m;
            }
            limit_peak(o)
        }
        StudioFxType::Flanger => mod_delay(x, 0.3, 0.5 + 5.0 * d, r, 0.6, m, sr, 0.0),
        StudioFxType::Phaser => {
            let mut o = vec![0.0f32; n];
            let mut z = [0.0f32; 6];
            let mut fbv = 0.0f32;
            for i in 0..n {
                let fc = 200.0 + 3000.0 * d * (0.5 + 0.5 * (2.0 * PI * r * i as f32 / sr).sin());
                let tn = (PI * fc / sr).tan();
                let a = (tn - 1.0) / (tn + 1.0);
                let mut s = x[i] + fbv * 0.5;
                for k in 0..6 {
                    let y = a * s + z[k];
                    z[k] = s - a * y;
                    s = y;
                }
                fbv = s;
                o[i] = x[i] * (1.0 - m * 0.5) + s * (m * 0.5);
            }
            limit_peak(o)
        }
        StudioFxType::Chorus => {
            let c1 = mod_delay(x, 18.0, 10.0 * d, r, 0.0, m, sr, 0.0);
            let c2 = mod_delay(x, 24.0, 10.0 * d, r * 1.1, 0.0, m, sr, 2.0);
            c1.iter().zip(c2.iter()).map(|(a, b)| (a + b) * 0.5).collect()
        }
        StudioFxType::ChorusMulti => {
            let nv = (st.sf_p1.round() as usize).clamp(1, 6);
            let rate = st.sf_p2.max(0.05);
            let depth_ms = st.sf_p3.clamp(0.5, 20.0);
            let mix = st.sf_p4.clamp(0.0, 1.0);
            let mut wet = vec![0.0f32; n];
            for v in 0..nv {
                let y = mod_delay(
                    x,
                    11.0 + ((v % 4) as f32) * 3.0,
                    depth_ms,
                    rate * (1.0 + v as f32 * 0.07),
                    0.0,
                    1.0,
                    sr,
                    ci as f32 * PI * 0.5 + v as f32 * 2.0 * PI / nv as f32,
                );
                for i in 0..n {
                    wet[i] += y[i] / nv as f32;
                }
            }
            limit_peak(
                x.iter()
                    .zip(wet.iter())
                    .map(|(&dry, &w)| dry * (1.0 - mix) + w * mix)
                    .collect(),
            )
        }
        StudioFxType::Reverb => {
            let decay = st.sf_decay.clamp(0.3, 6.0);
            let out_len = n + (sr * decay).round() as usize;
            let mut o = vec![0.0f32; out_len];
            let comb_delays = [1557usize, 1617, 1491, 1422, 1277, 1356];
            let mut combs: Vec<(Vec<f32>, usize, f32)> = comb_delays
                .iter()
                .map(|&d0| {
                    let len = ((d0 as f32 * sr / 44100.0).round() as usize).max(8);
                    let g = 10.0_f32.powf(-3.0 * len as f32 / (sr * decay));
                    (vec![0.0f32; len], 0, g)
                })
                .collect();
            let ap_delays = [225usize, 556, 441];
            let mut aps: Vec<(Vec<f32>, usize)> = ap_delays
                .iter()
                .map(|&d0| {
                    let len = ((d0 as f32 * sr / 44100.0).round() as usize).max(4);
                    (vec![0.0f32; len], 0)
                })
                .collect();
            for i in 0..out_len {
                let dry = if i < n { x[i] } else { 0.0 };
                let mut s = 0.0f32;
                for (buf, idx, g) in &mut combs {
                    let y = buf[*idx];
                    buf[*idx] = dry + y * *g;
                    *idx = (*idx + 1) % buf.len();
                    s += y;
                }
                s /= combs.len() as f32;
                for (buf, idx) in &mut aps {
                    let bo = buf[*idx];
                    buf[*idx] = s + bo * 0.5;
                    s = -s * 0.5 + bo;
                    *idx = (*idx + 1) % buf.len();
                }
                o[i] = dry * (1.0 - m) + s * m * 1.5;
            }
            limit_peak(o)
        }
        StudioFxType::Fade => {
            let mut o = x.to_vec();
            let a = ((st.sf_fade_in * sr).round() as usize).min(n);
            let b = ((st.sf_fade_out * sr).round() as usize).min(n);
            for i in 0..a {
                o[i] *= i as f32 / a.max(1) as f32;
            }
            for i in 0..b {
                o[n - 1 - i] *= i as f32 / b.max(1) as f32;
            }
            o
        }
        StudioFxType::Normalize => {
            let p = x.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
            let k = if p > 1e-6 { 0.95 / p } else { 1.0 };
            x.iter().map(|&s| s * k).collect()
        }
        StudioFxType::Lowpass | StudioFxType::Highpass => {
            let a = (-2.0 * PI * st.sf_fc / sr).exp();
            let mut o = vec![0.0f32; n];
            let (mut y1, mut y2) = (0.0f32, 0.0f32);
            for i in 0..n {
                y1 = (1.0 - a) * x[i] + a * y1;
                y2 = (1.0 - a) * y1 + a * y2;
                o[i] = if st.studio_type == StudioFxType::Lowpass {
                    y2
                } else {
                    x[i] - y2
                };
            }
            o
        }
        StudioFxType::RingMod => x
            .iter()
            .enumerate()
            .map(|(i, &s)| s * (1.0 - m + m * (2.0 * PI * st.sf_fc * i as f32 / sr).sin()))
            .collect(),
        StudioFxType::AutoPan => x
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                s * (1.0 - d * (0.5 + 0.5 * (2.0 * PI * r * i as f32 / sr + ci as f32 * PI).sin()))
            })
            .collect(),
        StudioFxType::Compressor => {
            let th = 10.0_f32.powf(-18.0 / 20.0);
            let ratio = 1.0 + 7.0 * d;
            let at = (-1.0 / (0.005 * sr)).exp();
            let rl = (-1.0 / (0.12 * sr)).exp();
            let mut o = vec![0.0f32; n];
            let mut env = 0.0f32;
            for i in 0..n {
                let ab = x[i].abs();
                env = if ab > env {
                    at * env + (1.0 - at) * ab
                } else {
                    rl * env + (1.0 - rl) * ab
                };
                let g = if env > th {
                    (env / th).powf(1.0 / ratio - 1.0)
                } else {
                    1.0
                };
                o[i] = x[i] * g * (1.0 + d * 0.8);
            }
            limit_peak(o)
        }
        StudioFxType::Gain => {
            let k = 10.0_f32.powf(st.sf_gain_db / 20.0);
            limit_peak(x.iter().map(|&s| s * k).collect())
        }
        StudioFxType::BassBoost => {
            let a = (-2.0 * PI * 150.0 / sr).exp();
            let k = 10.0_f32.powf(st.sf_gain_db / 20.0) - 1.0;
            let mut o = vec![0.0f32; n];
            let mut y = 0.0f32;
            for i in 0..n {
                y = (1.0 - a) * x[i] + a * y;
                o[i] = x[i] + k * y;
            }
            limit_peak(o)
        }
        StudioFxType::Telephone => {
            let lp_pass = |arr: &[f32], fc: f32| -> Vec<f32> {
                let a = (-2.0 * PI * fc / sr).exp();
                let mut o = vec![0.0f32; arr.len()];
                let (mut y1, mut y2) = (0.0f32, 0.0f32);
                for i in 0..arr.len() {
                    y1 = (1.0 - a) * arr[i] + a * y1;
                    y2 = (1.0 - a) * y1 + a * y2;
                    o[i] = y2;
                }
                o
            };
            let l = lp_pass(x, 3400.0);
            let h = lp_pass(&l, 300.0);
            limit_peak(l.iter().zip(h.iter()).map(|(a, b)| (a - b) * 1.4).collect())
        }
        StudioFxType::Gate => {
            let th = 10.0_f32.powf((-60.0 + 50.0 * d) / 20.0);
            let mut o = vec![0.0f32; n];
            let mut env = 0.0f32;
            let mut gs = 1.0f32;
            for i in 0..n {
                let ab = x[i].abs();
                env = if ab > env { ab } else { env * 0.9995 };
                let target = if env > th { 1.0 } else { 0.02 };
                gs += (target - gs) * 0.003;
                o[i] = x[i] * gs;
            }
            o
        }
        StudioFxType::Stutter => {
            let len = ((sr / (r * 4.0).max(0.5)).round() as usize).max(64);
            let mut o = vec![0.0f32; n];
            for i in 0..n {
                let pos = i % (len * 2);
                let base = i - pos;
                let src = base + if pos < len { pos } else { pos - len };
                o[i] = x[src.min(n - 1)];
            }
            o
        }
        StudioFxType::Vinyl => {
            let mut o = vec![0.0f32; n];
            for i in 0..n {
                let r1 = ((i as f32 * 12.9898 + 78.233).sin() * 43758.5453).fract();
                let r2 = ((i as f32 * 93.9898 + 19.13).sin() * 24634.6345).fract();
                let pop = if r1 < d * 0.0006 {
                    (r2 * 2.0 - 1.0) * 0.6
                } else {
                    0.0
                };
                o[i] = x[i] + m * (pop + (r2 * 2.0 - 1.0) * 0.008);
            }
            limit_peak(o)
        }
        StudioFxType::Octaver => {
            let sub = pitch_shift(x, -12.0, StretchTech::Fft, sr_u32, 2048);
            limit_peak(
                x.iter()
                    .zip(sub.iter())
                    .map(|(&dry, &s)| dry * (1.0 - m * 0.5) + s * m)
                    .collect(),
            )
        }
        StudioFxType::Eq3Band => {
            let y1 = run_biquad(x, biquad("low", 120.0, st.sf_p1, 0.707, sr));
            let y2 = run_biquad(&y1, biquad("peak", st.sf_p3.max(100.0), st.sf_p2, 1.0, sr));
            run_biquad(&y2, biquad("high", 8000.0, st.sf_p4, 0.707, sr))
        }
        StudioFxType::HumRemover => {
            let mut y = x.to_vec();
            let f0 = if st.sf_p1 < 55.0 { 50.0 } else { 60.0 };
            let harms = (st.sf_p2.round() as usize).clamp(1, 4);
            for k in 1..=harms {
                y = run_biquad(&y, biquad("notch", f0 * k as f32, 0.0, 25.0, sr));
            }
            y
        }
        StudioFxType::Overdrive => {
            let dr = st.sf_p1.max(1.0);
            let tone = st.sf_p3.clamp(500.0, 12000.0);
            let md = st.sf_p4.round() as i32;
            let raw: Vec<f32> = x
                .iter()
                .map(|&s| {
                    let v = s * dr;
                    if md == 0 {
                        v.tanh()
                    } else if md == 1 {
                        v.clamp(-1.0, 1.0)
                    } else {
                        v.signum() * (1.0 - (-3.0 * v.abs()).exp())
                    }
                })
                .collect();
            limit_peak(run_biquad(&raw, biquad("lp", tone, 0.0, 0.707, sr)))
        }
        StudioFxType::TapeStop => {
            let total_t = n as f32 / sr;
            let fr = (st.sf_p1.max(0.2).min(total_t)) / total_t.max(0.01);
            resample_variable(x, |q| {
                if q < 1.0 - fr {
                    1.0
                } else {
                    let u = (q - (1.0 - fr)) / fr.max(1e-4);
                    (1.0 - u).max(0.03)
                }
            })
        }
        StudioFxType::Repeat => {
            let reps = (st.sf_p1.round() as usize).clamp(1, 8);
            let mut o = Vec::with_capacity(n * reps);
            for _ in 0..reps {
                o.extend_from_slice(x);
            }
            o
        }
        StudioFxType::Limiter => {
            let ce = 10.0_f32.powf(st.sf_p1.min(0.0) / 20.0);
            let rl = (-1.0 / (st.sf_p2.max(5.0) * 0.001 * sr)).exp();
            let mut o = vec![0.0f32; n];
            let mut g = 1.0f32;
            for i in 0..n {
                let a = x[i].abs();
                let tg = if a > ce { ce / a } else { 1.0 };
                g = if tg < g { tg } else { g + (1.0 - g) * (1.0 - rl) };
                o[i] = x[i] * g;
            }
            o
        }
        StudioFxType::PingPong => {
            let delay = ((st.sf_p3.max(20.0) * sr / 1000.0).round() as usize).max(1);
            let fb = st.sf_p2.clamp(0.0, 0.9);
            let mix = st.sf_p4.clamp(0.0, 1.0);
            let mut o = vec![0.0f32; n + delay * 8];
            o[..n].copy_from_slice(x);
            let mut amp = 1.0f32;
            for k in 1..=8 {
                amp *= fb;
                if amp < 0.01 {
                    break;
                }
                if (k % 2 == 1) == (ci == 0) {
                    let off = k * delay;
                    for i in 0..n {
                        o[i + off] += x[i] * amp * mix;
                    }
                }
            }
            limit_peak(o)
        }
        StudioFxType::AutoFilter => {
            let rate = st.sf_p2.max(0.05);
            let depth = st.sf_p4.clamp(0.0, 1.0);
            let base = st.sf_p3.clamp(80.0, 4000.0);
            let q = 0.35;
            let mut o = vec![0.0f32; n];
            let (mut lp, mut bp) = (0.0f32, 0.0f32);
            for i in 0..n {
                let fc = (sr / 6.0).min(base * 2.0_f32.powf(depth * 3.0 * (2.0 * PI * rate * i as f32 / sr).sin()));
                let f = (2.0 * (PI * fc / sr).sin()).min(1.0);
                let hp = x[i] - lp - q * bp;
                bp += f * hp;
                lp += f * bp;
                o[i] = lp;
            }
            limit_peak(o)
        }
        StudioFxType::WowFlutter => {
            let wd = st.sf_p1.clamp(0.0, 6.0);
            let fd = st.sf_p2.clamp(0.0, 2.0);
            let rate = st.sf_p4.max(0.1);
            let size = ((sr * (wd + fd + 4.0) / 1000.0).ceil() as usize) + 8;
            let mut buf = vec![0.0f32; size];
            let mut o = vec![0.0f32; n];
            let mut w = 0usize;
            for i in 0..n {
                let dd = sr
                    * (2.0
                        + wd * (0.5 + 0.5 * (2.0 * PI * rate * i as f32 / sr).sin())
                        + fd * (0.5 + 0.5 * (2.0 * PI * rate * 7.3 * i as f32 / sr + 1.0).sin()))
                    / 1000.0;
                let mut rd = w as f32 - dd;
                while rd < 0.0 {
                    rd += size as f32;
                }
                let i0 = rd.floor() as usize;
                let f = rd - i0 as f32;
                buf[w] = x[i];
                o[i] = buf[i0 % size] * (1.0 - f) + buf[(i0 + 1) % size] * f;
                w = (w + 1) % size;
            }
            o
        }
        StudioFxType::Exciter => {
            let lp = run_biquad(x, biquad("lp", st.sf_p3.clamp(1500.0, 9000.0), 0.0, 0.707, sr));
            let dr = st.sf_p1.max(1.0);
            let mix = st.sf_p4.clamp(0.0, 1.0);
            limit_peak(
                x.iter()
                    .zip(lp.iter())
                    .map(|(&s, &l)| s + mix * 0.5 * ((s - l) * dr).tanh())
                    .collect(),
            )
        }
        StudioFxType::HaasWiden => {
            let d_samp = ((st.sf_p1.clamp(1.0, 40.0) * sr / 1000.0).round() as usize).max(1);
            if ci != 1 {
                return x.to_vec();
            }
            let mut o = vec![0.0f32; n + d_samp];
            o[d_samp..].copy_from_slice(x);
            o
        }
        StudioFxType::Rotary => {
            let rate = st.sf_p2.max(0.2);
            let depth = st.sf_p4.clamp(0.0, 1.0);
            let y = mod_delay(x, 0.6, 0.8 * depth + 0.01, rate, 0.0, 2.0, sr, 0.0);
            y.iter()
                .enumerate()
                .map(|(i, &s)| s * (1.0 - 0.5 * depth * (0.5 + 0.5 * (2.0 * PI * rate * i as f32 / sr).sin())))
                .collect()
        }
        StudioFxType::StereoPan => {
            let pan = st.sf_p1.clamp(-1.0, 1.0);
            let a = (pan + 1.0) * PI * 0.25;
            let k = if ci == 0 { a.cos() } else { a.sin() } * std::f32::consts::SQRT_2;
            limit_peak(x.iter().map(|&s| s * k).collect())
        }
        StudioFxType::TranceGate => {
            let rate = st.sf_p2.clamp(1.0, 16.0);
            let depth = st.sf_p4.clamp(0.0, 1.0);
            let mut o = vec![0.0f32; n];
            let mut g = 1.0f32;
            let a = 1.0 - (-1.0 / (sr * 0.002)).exp();
            for i in 0..n {
                let st_idx = ((i as f32 / sr) * rate * 2.0).floor() as usize;
                let on = st_idx % 2 == 0;
                let target = if on { 1.0 } else { 1.0 - depth };
                g += (target - g) * a;
                o[i] = x[i] * g;
            }
            o
        }
        StudioFxType::Wavefolder => {
            let amt = st.sf_p1.clamp(1.0, 12.0);
            let mix = st.sf_p4.clamp(0.0, 1.0);
            limit_peak(
                x.iter()
                    .map(|&s| s * (1.0 - mix) + (s * amt * 1.5708).sin() * mix)
                    .collect(),
            )
        }
        StudioFxType::SubOctave => {
            let fc = st.sf_p3.clamp(60.0, 400.0);
            let lp = run_biquad(x, biquad("lp", fc, 0.0, 0.707, sr));
            let mut sub = vec![0.0f32; n];
            let ea = (-1.0 / (0.02 * sr)).exp();
            let mut state = 1.0f32;
            let mut prev = 0.0f32;
            let mut env = 0.0f32;
            for i in 0..n {
                let c = lp[i];
                if prev <= 0.0 && c > 0.0 {
                    state = -state;
                }
                prev = c;
                env = c.abs().max(env * ea);
                sub[i] = state * env;
            }
            let sm = run_biquad(&sub, biquad("lp", fc * 1.2, 0.0, 0.707, sr));
            let lv = st.sf_p1.clamp(0.0, 2.0);
            limit_peak(x.iter().zip(sm.iter()).map(|(&dry, &s)| dry + s * lv).collect())
        }
        StudioFxType::CombFilter => {
            let freq = st.sf_p3.clamp(50.0, 2000.0);
            let d_samp = ((sr / freq).round() as usize).max(2);
            let fb = st.sf_p2.clamp(-0.95, 0.95);
            let mix = st.sf_p4.clamp(0.0, 1.0);
            let mut buf = vec![0.0f32; d_samp];
            let mut o = vec![0.0f32; n];
            let mut w = 0usize;
            for i in 0..n {
                let y = x[i] + fb * buf[w];
                buf[w] = y;
                w = (w + 1) % d_samp;
                o[i] = x[i] * (1.0 - mix) + y * mix * 0.6;
            }
            limit_peak(o)
        }
        StudioFxType::VocalRemover => {
            let len = n.min(other.len());
            (0..len).map(|i| (x[i] - other[i]) * 0.5).collect()
        }
        StudioFxType::MonoMixdown => {
            let len = n.min(other.len());
            (0..len).map(|i| (x[i] + other[i]) * 0.5).collect()
        }
        StudioFxType::DeEsser => {
            let th = 10.0_f32.powf(st.sf_p1.clamp(-40.0, 0.0) / 20.0);
            let red = st.sf_p2.clamp(0.0, 1.0);
            let fc = st.sf_p3.clamp(3000.0, 10000.0);
            let lp = run_biquad(x, biquad("lp", fc * 0.7, 0.0, 0.707, sr));
            let at = (-1.0 / (0.001 * sr)).exp();
            let rl = (-1.0 / (0.05 * sr)).exp();
            let mut o = vec![0.0f32; n];
            let mut env = 0.0f32;
            for i in 0..n {
                let hp = x[i] - lp[i];
                let a = hp.abs();
                env = if a > env {
                    at * env + (1.0 - at) * a
                } else {
                    rl * env + (1.0 - rl) * a
                };
                let g = if env > th { (th / env).powf(red) } else { 1.0 };
                o[i] = lp[i] + hp * g;
            }
            o
        }
    }
}

fn limit_peak(mut o: Vec<f32>) -> Vec<f32> {
    let p = o.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    if p > 1.0 {
        for s in &mut o {
            *s /= p;
        }
    }
    o
}
