// src/elastique_cli.rs
use crate::audio_dsp::{process_stereo, AudioLabState, AudioTab, StretchTech};
use crate::media_engine::{decode_audio_stereo, process_and_export_audio};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ElastiqueOptions {
    pub shifts: Vec<f64>,
    pub input: String,
    pub output: String,
    pub engine: String,
    pub audio_only: bool,
    pub formants: bool,
    pub force: bool,
    pub a17_preset: bool,
}

pub fn print_help() {
    println!(
        "Usage: elastique SHIFT [SHIFT ...] INPUT [-o OUTPUT.wav|OUTPUT.mkv]\n\
                elastique --a17 -6 8 input.mp4\n\n\
         Shifts are semitones, including decimals, from -36 to +36.\n\
         If INPUT is omitted, its path is requested interactively.\n\
           --engine elastique|a17  Engine (default: original Elastique Pro)\n\
           --a17                  Sony A17 Drums (minimum echo) preset\n\
           --formants             Preserve formants (Elastique only)\n\
           --audio-only           Extract audio from video, output WAV\n\
           -i, --input FILE       Explicit input path\n\
           -o, --output FILE      Output; default: INPUT.pitched.wav/.mkv\n\
           --force                Replace an existing output\n\
           -h, --help             Show this help\n\n\
         Voices are summed at full gain; no averaging, normalization or limiter.\n\
         Output is float32 PCM. Video is copied into MKV without re-encoding."
    );
}

fn parse_shift(s: &str) -> Option<f64> {
    if let Ok(val) = s.parse::<f64>() {
        if val.is_finite() && (-36.0..=36.0).contains(&val) {
            return Some(val);
        }
    }
    None
}

pub fn parse_args(args: &[String]) -> Result<Option<ElastiqueOptions>, String> {
    let mut o = ElastiqueOptions {
        shifts: Vec::new(),
        input: String::new(),
        output: String::new(),
        engine: "elastique".to_string(),
        audio_only: false,
        formants: false,
        force: false,
        a17_preset: false,
    };
    let mut input_seen = false;
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];
        if arg == "-h" || arg == "--help" {
            return Ok(None);
        }
        if arg == "--audio-only" {
            o.audio_only = true;
            i += 1;
            continue;
        }
        if arg == "--formants" {
            o.formants = true;
            i += 1;
            continue;
        }
        if arg == "--force" {
            o.force = true;
            i += 1;
            continue;
        }
        if arg == "--a17" {
            o.a17_preset = true;
            o.engine = "a17".to_string();
            i += 1;
            continue;
        }
        if arg == "--engine" || arg == "-i" || arg == "--input" || arg == "-o" || arg == "--output" {
            i += 1;
            if i >= args.len() {
                return Err(format!("Missing option value: {}", arg));
            }
            let val = &args[i];
            if arg == "--engine" {
                o.engine = val.clone();
            } else if arg == "-i" || arg == "--input" {
                o.input = val.clone();
                input_seen = true;
            } else {
                o.output = val.clone();
            }
            i += 1;
            continue;
        }

        if let Some(shift) = parse_shift(arg) {
            o.shifts.push(shift);
            i += 1;
            continue;
        }

        if arg.starts_with('-') {
            return Err(format!("Unknown option or invalid shift: {}", arg));
        }

        if !input_seen {
            o.input = arg.clone();
            input_seen = true;
        } else {
            return Err(format!("Unexpected argument: {}", arg));
        }
        i += 1;
    }

    if o.shifts.is_empty() {
        return Err("At least one pitch shift is required.".to_string());
    }
    if o.engine != "elastique" && o.engine != "a17" {
        return Err(format!("Unsupported engine: {}", o.engine));
    }
    if o.formants && o.engine != "elastique" {
        return Err("Formant preservation is supported only by Elastique.".to_string());
    }

    if o.input.is_empty() {
        print!("Input audio/video path: ");
        io::stdout().flush().map_err(|e| e.to_string())?;
        let mut input = String::new();
        io::stdin().read_line(&mut input).map_err(|e| e.to_string())?;
        o.input = input.trim().to_string();
    }

    if o.input.is_empty() {
        return Err("No input path supplied.".to_string());
    }

    if o.output.is_empty() {
        let p = Path::new(&o.input);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
        let parent = p.parent().unwrap_or(Path::new(""));
        let ext = if o.audio_only { ".wav" } else { ".mkv" };
        let out_name = format!("{}.pitched{}", stem, ext);
        o.output = parent.join(out_name).to_string_lossy().into_owned();
    }

    Ok(Some(o))
}

pub fn run_elastique_cli(args: &[String]) -> i32 {
    match parse_args(args) {
        Ok(Some(o)) => {
            let input_path = Path::new(&o.input);
            if !input_path.is_file() {
                eprintln!("Input must be a regular local audio or video file.");
                return 2;
            }
            if let (Ok(in_canonical), Ok(out_canonical)) = (fs::canonicalize(&o.input), fs::canonicalize(&o.output)) {
                if in_canonical == out_canonical {
                    eprintln!("Output must differ from input.");
                    return 2;
                }
            }
            if Path::new(&o.output).exists() && !o.force {
                eprintln!("Output exists; use --force to replace it.");
                return 2;
            }

            let mut st = AudioLabState::default();
            st.active_tab = AudioTab::PitchShift;
            st.pitch_tech = if o.engine == "a17" {
                StretchTech::Wsola
            } else {
                StretchTech::RubberbandRefined
            };
            st.formant_keep = o.formants;

            let mut sum_l: Vec<f32> = Vec::new();
            let mut sum_r: Vec<f32> = Vec::new();
            let sr_out;

            match decode_audio_stereo(input_path) {
                Ok((l, r, sr)) => {
                    sr_out = sr;
                    for &shift in &o.shifts {
                        st.pitch_semitones = shift as f32;
                        let (vl, vr) = process_stereo(&l, &r, sr, &st);
                        if sum_l.is_empty() {
                            sum_l = vl;
                            sum_r = vr;
                        } else {
                            let len = sum_l.len().min(vl.len());
                            for idx in 0..len {
                                sum_l[idx] += vl[idx];
                                sum_r[idx] += vr[idx];
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Decode failed: {}", e);
                    return 2;
                }
            }

            let out_path = Path::new(&o.output);
            if o.audio_only || out_path.extension().and_then(|s| s.to_str()) == Some("wav") {
                if let Err(e) = process_and_export_audio(input_path, out_path, &st) {
                    eprintln!("Export failed: {}", e);
                    return 2;
                }
            } else {
                // Save temporary WAV and mux with original video into MKV
                let temp_wav = std::env::temp_dir().join("elastique_temp_out.wav");
                let spec = hound::WavSpec {
                    channels: 2,
                    sample_rate: sr_out,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                };
                if let Ok(mut w) = hound::WavWriter::create(&temp_wav, spec) {
                    for i in 0..sum_l.len() {
                        let _ = w.write_sample(sum_l[i]);
                        let _ = w.write_sample(sum_r[i]);
                    }
                    let _ = w.finalize();
                }
                let status = std::process::Command::new("ffmpeg")
                    .args(["-y", "-i"])
                    .arg(input_path)
                    .arg("-i")
                    .arg(&temp_wav)
                    .args(["-map", "0:v?", "-map", "1:a", "-c:v", "copy", "-c:a", "pcm_f32le"])
                    .arg(out_path)
                    .output();
                if let Ok(res) = status {
                    if !res.status.success() {
                        eprintln!("FFmpeg MKV mux error: {}", String::from_utf8_lossy(&res.stderr));
                        return 2;
                    }
                }
            }

            println!("Elastique CLI ({}) exported successfully -> {}", o.engine, o.output);
            0
        }
        Ok(None) => {
            print_help();
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            2
        }
    }
}
