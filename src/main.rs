#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_dsp;
mod media_engine;

use audio_dsp::{AudioDspParams, AudioEffect};
use eframe::egui;
use media_engine::{
    check_ffmpeg_available, export_audio_file, export_timeline, probe_duration,
    validate_export_combo, AudioCodec, ContainerFormat, ExportSettings, ResolutionPreset,
    TimelineClip, VideoCodec,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorMode {
    VideoEditor,
    AudioLab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorTab {
    Media,
    AdjustColor,
    Effects,
    Export,
}

struct GrainApp {
    mode: EditorMode,
    inspector_tab: InspectorTab,
    ffmpeg_ok: bool,

    // Video Timeline State
    clips: Vec<TimelineClip>,
    selected_clip: Option<usize>,
    playhead_sec: f64,
    export_settings: ExportSettings,

    // Audio Lab State
    audio_input: Option<PathBuf>,
    audio_params: AudioDspParams,

    // Status Banner
    status_message: String,
    is_error: bool,
}

impl Default for GrainApp {
    fn default() -> Self {
        let ffmpeg_ok = check_ffmpeg_available();
        let status_message = if ffmpeg_ok {
            format!(
                "Ready — Pure Rust Engine ({} / {}) + Native FFmpeg & VideoToolbox",
                std::env::consts::OS,
                std::env::consts::ARCH
            )
        } else {
            "Warning: `ffmpeg` not found on PATH. Run `brew install ffmpeg` on macOS to enable hardware export."
                .into()
        };

        Self {
            mode: EditorMode::VideoEditor,
            inspector_tab: InspectorTab::Export, // Open on Export tab by default so it's immediately visible & usable
            ffmpeg_ok,
            clips: Vec::new(),
            selected_clip: None,
            playhead_sec: 0.0,
            export_settings: ExportSettings::default(),
            audio_input: None,
            audio_params: AudioDspParams::default(),
            status_message,
            is_error: !ffmpeg_ok,
        }
    }
}

impl GrainApp {
    fn set_status(&mut self, msg: impl Into<String>, err: bool) {
        self.status_message = msg.into();
        self.is_error = err;
    }

    fn import_video_clip(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Video Files", &["mp4", "mov", "mkv", "webm", "avi", "m4v"])
            .pick_file()
        {
            let dur = probe_duration(&path);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Clip".into());
            self.clips.push(TimelineClip {
                path,
                name: name.clone(),
                duration_sec: dur,
                trim_start_sec: 0.0,
                trim_end_sec: dur,
                speed: 1.0,
                brightness: 0.0,
                contrast: 1.0,
                saturation: 1.0,
                hue_deg: 0.0,
                blur: 0.0,
                grayscale: false,
                sepia: false,
                invert: false,
                mirror_h: false,
                mirror_v: false,
                overlay_text: String::new(),
            });
            self.selected_clip = Some(self.clips.len() - 1);
            self.set_status(format!("Loaded video clip: {} ({:.2}s)", name, dur), false);
        }
    }

    fn import_audio_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Audio Files", &["wav", "mp3", "flac", "ogg", "m4a", "aac"])
            .pick_file()
        {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Audio".into());
            self.audio_input = Some(path);
            self.set_status(format!("Loaded audio track: {}", name), false);
        }
    }

    fn trigger_video_export(&mut self) {
        if self.clips.is_empty() {
            self.set_status("Add at least one video clip before exporting.", true);
            return;
        }
        if let Err(msg) = validate_export_combo(&self.export_settings) {
            self.set_status(msg, true);
            return;
        }
        let ext = self.export_settings.container.ext();
        let default_name = format!("grain_export.{}", ext);
        if let Some(out_path) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter(ext.to_uppercase(), &[ext])
            .save_file()
        {
            self.set_status("Exporting via Rust + macOS VideoToolbox...", false);
            match export_timeline(&self.clips, &out_path, &self.export_settings) {
                Ok(msg) => self.set_status(msg, false),
                Err(err) => self.set_status(err, true),
            }
        }
    }

    fn trigger_audio_export(&mut self) {
        let Some(ref in_path) = self.audio_input else {
            self.set_status("Load an audio file first.", true);
            return;
        };
        if let Some(out_path) = rfd::FileDialog::new()
            .set_file_name("grain_processed.wav")
            .add_filter("WAV Audio", &["wav"])
            .save_file()
        {
            match export_audio_file(in_path, &out_path, &self.audio_params) {
                Ok(msg) => self.set_status(msg, false),
                Err(err) => self.set_status(err, true),
            }
        }
    }
}

impl eframe::App for GrainApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Dark pro-editor theme
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(20, 22, 27);
        visuals.window_fill = egui::Color32::from_rgb(24, 27, 34);
        ctx.set_visuals(visuals);

        // Top Mode Bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(
                    egui::RichText::new("GRAIN")
                        .strong()
                        .color(egui::Color32::from_rgb(45, 212, 191)),
                );
                ui.label(
                    egui::RichText::new("100% Pure Rust • Zero JS")
                        .small()
                        .color(egui::Color32::GRAY),
                );
                ui.separator();
                ui.selectable_value(&mut self.mode, EditorMode::VideoEditor, "🎬 Video Editor");
                ui.selectable_value(&mut self.mode, EditorMode::AudioLab, "🎛 Audio Pitch & Time Lab");
            });
            ui.add_space(6.0);
        });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            let color = if self.is_error {
                egui::Color32::from_rgb(255, 107, 107)
            } else {
                egui::Color32::from_rgb(45, 212, 191)
            };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&self.status_message).color(color));
            });
            ui.add_space(4.0);
        });

        match self.mode {
            EditorMode::VideoEditor => self.render_video_editor(ctx),
            EditorMode::AudioLab => self.render_audio_lab(ctx),
        }
    }
}

impl GrainApp {
    fn render_video_editor(&mut self, ctx: &egui::Context) {
        // Right Inspector Panel (Media | Adjust | Effects | Export)
        egui::SidePanel::right("inspector_panel")
            .min_width(340.0)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.inspector_tab, InspectorTab::Media, "Media");
                    ui.selectable_value(&mut self.inspector_tab, InspectorTab::AdjustColor, "Adjust");
                    ui.selectable_value(&mut self.inspector_tab, InspectorTab::Effects, "Effects");
                    ui.selectable_value(
                        &mut self.inspector_tab,
                        InspectorTab::Export,
                        egui::RichText::new("Export").strong(),
                    );
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| match self.inspector_tab {
                    InspectorTab::Media => {
                        if ui.button("+ Import Video Clip...").clicked() {
                            self.import_video_clip();
                        }
                        ui.add_space(8.0);
                        if let Some(idx) = self.selected_clip {
                            if let Some(clip) = self.clips.get_mut(idx) {
                                ui.label(egui::RichText::new(&clip.name).strong());
                                ui.label(format!("Source Duration: {:.2} s", clip.duration_sec));
                                ui.add_space(6.0);
                                ui.add(
                                    egui::Slider::new(
                                        &mut clip.trim_start_sec,
                                        0.0..=(clip.trim_end_sec - 0.1).max(0.0),
                                    )
                                    .text("Trim In (s)"),
                                );
                                ui.add(
                                    egui::Slider::new(
                                        &mut clip.trim_end_sec,
                                        (clip.trim_start_sec + 0.1)..=clip.duration_sec,
                                    )
                                    .text("Trim Out (s)"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut clip.speed, 0.25..=3.0)
                                        .text("Speed (x)"),
                                );
                            }
                        } else {
                            ui.label("Import a video file to configure trim and speed.");
                        }
                    }

                    InspectorTab::AdjustColor => {
                        if let Some(idx) = self.selected_clip {
                            if let Some(clip) = self.clips.get_mut(idx) {
                                ui.heading("Color & Grade");
                                ui.add(
                                    egui::Slider::new(&mut clip.brightness, -0.5..=0.5)
                                        .text("Brightness"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut clip.contrast, 0.2..=2.0)
                                        .text("Contrast"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut clip.saturation, 0.0..=3.0)
                                        .text("Saturation"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut clip.hue_deg, -180.0..=180.0)
                                        .text("Hue (°)"),
                                );
                                ui.add_space(8.0);
                                ui.checkbox(&mut clip.grayscale, "Grayscale");
                                ui.checkbox(&mut clip.sepia, "Sepia Tone");
                                ui.checkbox(&mut clip.invert, "Invert Colors");
                                ui.add_space(8.0);
                                if ui.button("Reset Color").clicked() {
                                    clip.brightness = 0.0;
                                    clip.contrast = 1.0;
                                    clip.saturation = 1.0;
                                    clip.hue_deg = 0.0;
                                    clip.grayscale = false;
                                    clip.sepia = false;
                                    clip.invert = false;
                                }
                            }
                        } else {
                            ui.label("Select a clip on the timeline first.");
                        }
                    }

                    InspectorTab::Effects => {
                        if let Some(idx) = self.selected_clip {
                            if let Some(clip) = self.clips.get_mut(idx) {
                                ui.heading("Transform & Filters");
                                ui.add(
                                    egui::Slider::new(&mut clip.blur, 0.0..=10.0)
                                        .text("Gaussian / Box Blur"),
                                );
                                ui.checkbox(&mut clip.mirror_h, "Mirror Horizontal");
                                ui.checkbox(&mut clip.mirror_v, "Mirror Vertical");
                                ui.add_space(10.0);
                                ui.heading("Text Overlay");
                                ui.label("Subtitle / Title Burn-in:");
                                ui.text_edit_singleline(&mut clip.overlay_text);
                            }
                        } else {
                            ui.label("Select a clip on the timeline first.");
                        }
                    }

                    InspectorTab::Export => {
                        ui.heading("Export Settings (Rust + FFmpeg)");
                        ui.add_space(6.0);

                        egui::ComboBox::from_label("Container")
                            .selected_text(self.export_settings.container.label())
                            .show_ui(ui, |ui| {
                                for &c in ContainerFormat::ALL {
                                    ui.selectable_value(
                                        &mut self.export_settings.container,
                                        c,
                                        c.label(),
                                    );
                                }
                            });

                        egui::ComboBox::from_label("Video Codec")
                            .selected_text(self.export_settings.video_codec.label())
                            .show_ui(ui, |ui| {
                                for &vc in VideoCodec::ALL {
                                    ui.selectable_value(
                                        &mut self.export_settings.video_codec,
                                        vc,
                                        vc.label(),
                                    );
                                }
                            });

                        egui::ComboBox::from_label("Audio Codec")
                            .selected_text(self.export_settings.audio_codec.label())
                            .show_ui(ui, |ui| {
                                for &ac in AudioCodec::ALL {
                                    ui.selectable_value(
                                        &mut self.export_settings.audio_codec,
                                        ac,
                                        ac.label(),
                                    );
                                }
                            });

                        egui::ComboBox::from_label("Resolution")
                            .selected_text(self.export_settings.resolution.label())
                            .show_ui(ui, |ui| {
                                for &res in ResolutionPreset::ALL {
                                    ui.selectable_value(
                                        &mut self.export_settings.resolution,
                                        res,
                                        res.label(),
                                    );
                                }
                            });

                        ui.add_space(6.0);
                        ui.add(
                            egui::Slider::new(&mut self.export_settings.fps, 24..=120)
                                .text("Frame Rate (FPS)"),
                        );
                        ui.add(
                            egui::Slider::new(
                                &mut self.export_settings.video_bitrate_kbps,
                                1000..=100_000,
                            )
                            .text("Video Bitrate (kbps)"),
                        );
                        ui.add(
                            egui::Slider::new(
                                &mut self.export_settings.audio_bitrate_kbps,
                                64..=512,
                            )
                            .text("Audio Bitrate (kbps)"),
                        );

                        egui::ComboBox::from_label("Audio Sample Rate")
                            .selected_text(format!("{} Hz", self.export_settings.sample_rate))
                            .show_ui(ui, |ui| {
                                for sr in [44_100u32, 48_000, 96_000, 192_000] {
                                    ui.selectable_value(
                                        &mut self.export_settings.sample_rate,
                                        sr,
                                        format!("{} Hz", sr),
                                    );
                                }
                            });

                        ui.add_space(6.0);
                        ui.checkbox(
                            &mut self.export_settings.hardware_encode,
                            "macOS VideoToolbox Hardware Encode",
                        );
                        ui.checkbox(
                            &mut self.export_settings.hardware_decode,
                            "macOS VideoToolbox Hardware Decode",
                        );

                        ui.add_space(8.0);
                        if let Err(validation_err) = validate_export_combo(&self.export_settings) {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 180, 70),
                                validation_err,
                            );
                        }

                        ui.add_space(10.0);
                        let export_btn = egui::Button::new(
                            egui::RichText::new(format!(
                                "▶ Export {} Now",
                                self.export_settings.container.ext().to_uppercase()
                            ))
                            .strong()
                            .color(egui::Color32::BLACK),
                        )
                        .fill(egui::Color32::from_rgb(45, 212, 191))
                        .min_size(egui::vec2(ui.available_width(), 38.0));

                        if ui.add(export_btn).clicked() {
                            self.trigger_video_export();
                        }
                    }
                });
            });

        // Bottom Multi-Track Timeline Panel
        egui::TopBottomPanel::bottom("timeline_panel")
            .min_height(190.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("+ Import Video").clicked() {
                        self.import_video_clip();
                    }
                    if let Some(idx) = self.selected_clip {
                        if ui.button("✂ Split at Playhead").clicked() {
                            if let Some(c) = self.clips.get(idx).cloned() {
                                let mid = (c.trim_start_sec + c.trim_end_sec) * 0.5;
                                let mut first = c.clone();
                                first.trim_end_sec = mid;
                                let mut second = c;
                                second.trim_start_sec = mid;
                                self.clips[idx] = first;
                                self.clips.insert(idx + 1, second);
                            }
                        }
                        if ui.button("✕ Remove Clip").clicked() {
                            self.clips.remove(idx);
                            self.selected_clip = if self.clips.is_empty() {
                                None
                            } else {
                                Some(idx.min(self.clips.len() - 1))
                            };
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Go to Export →").clicked() {
                            self.inspector_tab = InspectorTab::Export;
                        }
                    });
                });
                ui.separator();

                // Visual Timeline Lanes (V1 & A1)
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if self.clips.is_empty() {
                            ui.label("Timeline is empty. Click '+ Import Video' to add clips.");
                        }
                        for (i, clip) in self.clips.iter().enumerate() {
                            let is_sel = self.selected_clip == Some(i);
                            let width = (clip.effective_duration() as f32 * 28.0).clamp(110.0, 420.0);
                            let fill = if is_sel {
                                egui::Color32::from_rgb(45, 212, 191)
                            } else {
                                egui::Color32::from_rgb(35, 130, 125)
                            };
                            let frame = egui::Frame::none()
                                .fill(fill)
                                .rounding(4.0)
                                .inner_margin(8.0);
                            let resp = frame.show(ui, |ui| {
                                ui.set_min_width(width);
                                ui.label(
                                    egui::RichText::new(format!("V1 • {}", clip.name))
                                        .color(egui::Color32::BLACK)
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{:.2}s ({:.2}x)",
                                        clip.effective_duration(),
                                        clip.speed
                                    ))
                                    .small()
                                    .color(egui::Color32::from_rgb(10, 25, 25)),
                                );
                            });
                            if resp.response.interact(egui::Sense::click()).clicked() {
                                self.selected_clip = Some(i);
                            }
                        }
                    });
                });
            });

        // Central Preview Monitor
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                if let Some(idx) = self.selected_clip {
                    if let Some(clip) = self.clips.get(idx) {
                        ui.vertical_centered(|ui| {
                            ui.add_space(24.0);
                            ui.heading(format!("Preview Monitor — {}", clip.name));
                            ui.add_space(8.0);
                            ui.label(format!(
                                "In: {:.2}s  |  Out: {:.2}s  |  Duration: {:.2}s  |  Speed: {:.2}x",
                                clip.trim_start_sec,
                                clip.trim_end_sec,
                                clip.effective_duration(),
                                clip.speed
                            ));
                            ui.label(format!(
                                "Brightness: {:+.2}  |  Contrast: {:.2}  |  Saturation: {:.2}  |  Hue: {:.0}°",
                                clip.brightness, clip.contrast, clip.saturation, clip.hue_deg
                            ));
                            if !clip.overlay_text.is_empty() {
                                ui.label(format!("Overlay Text: "{}"", clip.overlay_text));
                            }
                            ui.add_space(16.0);
                            ui.add(
                                egui::Slider::new(
                                    &mut self.playhead_sec,
                                    clip.trim_start_sec..=clip.trim_end_sec,
                                )
                                .text("Scrub Playhead (s)"),
                            );
                        });
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.heading("Grain Video Editor (Native Rust)");
                        ui.label("Import a video file to begin editing and hardware-accelerated export.");
                        ui.add_space(12.0);
                        if ui.button("📂 Open Video File").clicked() {
                            self.import_video_clip();
                        }
                    });
                }
            });
        });
    }

    fn render_audio_lab(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Grain — Rust Audio Pitch & Time Lab");
                ui.label("Native Rust FFT Phase Vocoder, WSOLA Time-Stretching, and DSP Effects.");
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.button("📂 Choose Audio File...").clicked() {
                        self.import_audio_file();
                    }
                    if let Some(ref p) = self.audio_input {
                        ui.label(format!("Selected: {}", p.display()));
                    }
                });

                ui.add_space(12.0);
                egui::ComboBox::from_label("DSP Effect")
                    .selected_text(self.audio_params.effect.label())
                    .show_ui(ui, |ui| {
                        for &fx in AudioEffect::ALL {
                            ui.selectable_value(&mut self.audio_params.effect, fx, fx.label());
                        }
                    });

                ui.add_space(10.0);
                match self.audio_params.effect {
                    AudioEffect::PitchShift => {
                        ui.add(
                            egui::Slider::new(&mut self.audio_params.semitones, -24.0..=24.0)
                                .text("Semitones"),
                        );
                        egui::ComboBox::from_label("FFT Window Size")
                            .selected_text(format!("{}", self.audio_params.fft_size))
                            .show_ui(ui, |ui| {
                                for sz in [512usize, 1024, 2048, 4096, 8192] {
                                    ui.selectable_value(
                                        &mut self.audio_params.fft_size,
                                        sz,
                                        format!("{} samples", sz),
                                    );
                                }
                            });
                    }
                    AudioEffect::TimeStretch | AudioEffect::SpeedPitch => {
                        ui.add(
                            egui::Slider::new(&mut self.audio_params.time_factor, 0.4..=2.5)
                                .text("Duration Factor (x)"),
                        );
                    }
                    AudioEffect::Vibrato => {
                        ui.add(
                            egui::Slider::new(&mut self.audio_params.rate_hz, 0.5..=15.0)
                                .text("LFO Rate (Hz)"),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.audio_params.depth, 0.05..=4.0)
                                .text("Depth (Semitones)"),
                        );
                    }
                    AudioEffect::Bitcrush => {
                        ui.add(
                            egui::Slider::new(&mut self.audio_params.bit_depth, 1..=16)
                                .text("Bit Depth"),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.audio_params.sample_rate_div, 1..=40)
                                .text("Sample Rate Divisor"),
                        );
                    }
                    AudioEffect::Tremolo => {
                        ui.add(
                            egui::Slider::new(&mut self.audio_params.rate_hz, 0.5..=20.0)
                                .text("Rate (Hz)"),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.audio_params.depth, 0.0..=1.0)
                                .text("Depth"),
                        );
                    }
                    AudioEffect::Echo => {
                        ui.add(
                            egui::Slider::new(&mut self.audio_params.delay_ms, 10.0..=1000.0)
                                .text("Delay (ms)"),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.audio_params.feedback, 0.0..=0.9)
                                .text("Feedback"),
                        );
                    }
                    AudioEffect::Distortion => {
                        ui.add(
                            egui::Slider::new(&mut self.audio_params.drive, 1.0..=20.0)
                                .text("Drive"),
                        );
                    }
                    AudioEffect::Reverse | AudioEffect::Invert | AudioEffect::Normalize => {
                        ui.label("No additional parameters required for this effect.");
                    }
                }

                ui.add_space(16.0);
                let run_btn = egui::Button::new(
                    egui::RichText::new("▶ Process & Save WAV (Pure Rust DSP)")
                        .strong()
                        .color(egui::Color32::BLACK),
                )
                .fill(egui::Color32::from_rgb(45, 212, 191))
                .min_size(egui::vec2(280.0, 38.0));

                if ui.add(run_btn).clicked() {
                    self.trigger_audio_export();
                }
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Grain Video & Audio Editor")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([920.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Grain Video Editor",
        options,
        Box::new(|_cc| Ok(Box::new(GrainApp::default()))),
    )
}
