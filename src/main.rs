#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_dsp;
mod media_engine;
mod ofx_host;

use audio_dsp::{
    AudioExportFormat, AudioLabState, AudioTab, CarrierWave, MoreFxType, StretchTech,
    StudioFxType, TremoloWave,
};
use eframe::egui;
use media_engine::{
    check_ffmpeg, decode_audio_stereo, export_video_sequence, probe_media_info,
    process_and_export_audio, validate_video_export, AspectRatioPreset, ContainerFormat,
    EasingCurve, FramingMode, MaskType, MusicTrackItem, OverlayTrackItem, ProjectSettings,
    ResolutionPreset, TextOverlayItem, TimelineClip, TransitionType, VideoAudioCodec, VideoCodec,
    VideoExportSettings, VideoFxParams, VideoKeyframe, VideoLook,
};
use ofx_host::{default_ofx_search_paths, inspect_ofx_binary, scan_ofx_directories, OfxPluginInfo};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppMode {
    Audio,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LibraryPage {
    Media,
    Effects,
    Transitions,
    Text,
    Adjust,
    OpenFx,
    Export,
}

impl LibraryPage {
    const ALL: &'static [LibraryPage] = &[
        LibraryPage::Media,
        LibraryPage::Effects,
        LibraryPage::Transitions,
        LibraryPage::Text,
        LibraryPage::Adjust,
        LibraryPage::OpenFx,
        LibraryPage::Export,
    ];

    fn label(&self) -> &'static str {
        match self {
            LibraryPage::Media => "Media",
            LibraryPage::Effects => "Effects",
            LibraryPage::Transitions => "Transitions",
            LibraryPage::Text => "Text",
            LibraryPage::Adjust => "Adjust",
            LibraryPage::OpenFx => "OpenFX",
            LibraryPage::Export => "Export",
        }
    }
}

#[derive(Clone)]
struct TimelineSnapshot {
    clips: Vec<TimelineClip>,
    overlays: Vec<OverlayTrackItem>,
    music: Vec<MusicTrackItem>,
    texts: Vec<TextOverlayItem>,
}

struct GrainStudioApp {
    mode: AppMode,
    lib_page: LibraryPage,

    // Audio Mode State
    audio_path: Option<PathBuf>,
    audio_filename: String,
    audio_duration_sec: f64,
    audio_sr: u32,
    audio_peaks_l: Vec<(f32, f32)>,
    audio_peaks_r: Vec<(f32, f32)>,
    audio_playhead_frac: f32,
    audio_state: AudioLabState,

    // Video Mode State
    project: ProjectSettings,
    show_project_modal: bool,
    effect_search: String,

    clips: Vec<TimelineClip>,
    selected_clip_idx: Option<usize>,
    overlays: Vec<OverlayTrackItem>,
    music_tracks: Vec<MusicTrackItem>,
    text_overlays: Vec<TextOverlayItem>,
    draft_text: TextOverlayItem,

    // OpenFX (OFX 1.4/1.5) Host State
    ofx_extra_dirs: Vec<PathBuf>,
    ofx_plugins: Vec<OfxPluginInfo>,
    active_ofx_identifiers: Vec<String>,

    // Keyframe & Curve Editor State
    keyframes: Vec<VideoKeyframe>,
    selected_easing: EasingCurve,

    // Vegas Pro 15 Timeline Controls
    tl_zoom: f32,
    snap_enabled: bool,
    playhead_sec: f64,
    is_playing: bool,
    source_res: (u32, u32),
    undo_stack: Vec<TimelineSnapshot>,
    redo_stack: Vec<TimelineSnapshot>,

    // Export Settings
    video_export: VideoExportSettings,

    // Status Banner & Progress
    status_text: String,
    status_error: bool,
    progress_pct: Option<f32>,
}

impl Default for GrainStudioApp {
    fn default() -> Self {
        let ffmpeg_available = check_ffmpeg();
        let ofx_plugins = scan_ofx_directories(&[]);
        let status_text = if ffmpeg_available {
            format!(
                "Ready — Pure Rust Studio ({} / {}) • {} OpenFX plugins detected • VideoToolbox Ready",
                std::env::consts::OS,
                std::env::consts::ARCH,
                ofx_plugins.len()
            )
        } else {
            "Warning: `ffmpeg` not found. Run `brew install ffmpeg` on macOS to enable hardware VideoToolbox export."
                .into()
        };

        Self {
            mode: AppMode::Video,
            lib_page: LibraryPage::Effects,
            audio_path: None,
            audio_filename: "No file loaded".into(),
            audio_duration_sec: 0.0,
            audio_sr: 48_000,
            audio_peaks_l: Vec::new(),
            audio_peaks_r: Vec::new(),
            audio_playhead_frac: 0.0,
            audio_state: AudioLabState::default(),
            project: ProjectSettings::default(),
            show_project_modal: false,
            effect_search: String::new(),
            clips: Vec::new(),
            selected_clip_idx: None,
            overlays: Vec::new(),
            music_tracks: Vec::new(),
            text_overlays: Vec::new(),
            draft_text: TextOverlayItem {
                text: "Your text".into(),
                size_pct: 8.0,
                x_pct: 50.0,
                y_pct: 85.0,
                start_sec: 0.0,
                end_sec: 0.0,
                color: [255, 255, 255],
            },
            ofx_extra_dirs: Vec::new(),
            ofx_plugins,
            active_ofx_identifiers: Vec::new(),
            keyframes: Vec::new(),
            selected_easing: EasingCurve::InOutCubic,
            tl_zoom: 60.0,
            snap_enabled: true,
            playhead_sec: 0.0,
            is_playing: false,
            source_res: (1920, 1080),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            video_export: VideoExportSettings::default(),
            status_text,
            status_error: !ffmpeg_available,
            progress_pct: None,
        }
    }
}

impl GrainStudioApp {
    fn set_status(&mut self, msg: impl Into<String>, err: bool) {
        self.status_text = msg.into();
        self.status_error = err;
    }

    fn snapshot(&self) -> TimelineSnapshot {
        TimelineSnapshot {
            clips: self.clips.clone(),
            overlays: self.overlays.clone(),
            music: self.music_tracks.clone(),
            texts: self.text_overlays.clone(),
        }
    }

    fn push_undo(&mut self) {
        let s = self.snapshot();
        self.undo_stack.push(s);
        if self.undo_stack.len() > 60 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.snapshot());
            self.clips = prev.clips;
            self.overlays = prev.overlays;
            self.music_tracks = prev.music;
            self.text_overlays = prev.texts;
            self.set_status("Undid timeline action.", false);
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.snapshot());
            self.clips = next.clips;
            self.overlays = next.overlays;
            self.music_tracks = next.music;
            self.text_overlays = next.texts;
            self.set_status("Redid timeline action.", false);
        }
    }

    fn total_timeline_duration(&self) -> f64 {
        let v_dur: f64 = self.clips.iter().map(|c| c.effective_duration()).sum();
        let o_dur: f64 = self
            .overlays
            .iter()
            .map(|o| o.start_sec + o.duration_sec)
            .fold(0.0, f64::max);
        let m_dur: f64 = self
            .music_tracks
            .iter()
            .map(|m| m.start_sec + m.duration_sec)
            .fold(0.0, f64::max);
        v_dur.max(o_dur).max(m_dur).max(1.0)
    }

    fn import_audio_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Audio / Video Media",
                &["wav", "mp3", "flac", "ogg", "m4a", "aac", "mp4", "mov"],
            )
            .pick_file()
        {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Track".into());
            match decode_audio_stereo(&path) {
                Ok((l, r, sr)) => {
                    self.audio_sr = sr;
                    self.audio_duration_sec = l.len() as f64 / sr.max(1) as f64;
                    self.audio_peaks_l = compute_waveform_peaks(&l, 420);
                    self.audio_peaks_r = compute_waveform_peaks(&r, 420);
                    self.audio_filename = name.clone();
                    self.audio_path = Some(path);
                    self.set_status(
                        format!(
                            "{} — {:.2}s, {} Hz, 2 ch",
                            name, self.audio_duration_sec, sr
                        ),
                        false,
                    );
                }
                Err(e) => self.set_status(e, true),
            }
        }
    }

    fn run_audio_process(&mut self) {
        // Clone PathBuf so `self` is not borrowed immutably during `self.set_status(...)`
        let Some(in_path) = self.audio_path.clone() else {
            self.set_status("Load an audio file first.", true);
            return;
        };
        let ext = self.audio_state.export_format.ext();
        let default_name = format!("processed.{}", ext);
        if let Some(out_path) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter(ext.to_uppercase(), &[ext])
            .save_file()
        {
            self.progress_pct = Some(15.0);
            self.set_status("Processing audio DSP... 15%", false);
            match process_and_export_audio(&in_path, &out_path, &self.audio_state) {
                Ok(msg) => {
                    self.progress_pct = Some(100.0);
                    self.set_status(format!("{} (100%)", msg), false);
                }
                Err(e) => {
                    self.progress_pct = None;
                    self.set_status(e, true);
                }
            }
        }
    }

    fn import_video_clip(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Video Files", &["mp4", "mov", "mkv", "webm", "avi", "m4v"])
            .pick_file()
        {
            self.push_undo();
            let (dur, w, h) = probe_media_info(&path);
            if self.clips.is_empty() {
                self.source_res = (w, h);
            }
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
                is_compound: false,
                fx: VideoFxParams::default(),
            });
            self.selected_clip_idx = Some(self.clips.len() - 1);
            self.set_status(format!("{} — {:.2}s, {}×{}", name, dur, w, h), false);
        }
    }

    fn import_overlay_clip(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Overlay Video", &["mp4", "mov", "mkv", "webm"])
            .pick_file()
        {
            self.push_undo();
            let (dur, _, _) = probe_media_info(&path);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Overlay".into());
            self.overlays.push(OverlayTrackItem {
                path,
                name: name.clone(),
                duration_sec: dur,
                start_sec: self.playhead_sec,
                x_pct: 75.0,
                y_pct: 75.0,
                size_pct: 30.0,
                opacity_pct: 100.0,
            });
            self.set_status(format!("Added V2 Overlay: {}", name), false);
        }
    }

    fn import_music_clip(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Audio Track", &["wav", "mp3", "flac", "ogg", "m4a", "aac"])
            .pick_file()
        {
            self.push_undo();
            let (dur, _, _) = probe_media_info(&path);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Audio".into());
            self.music_tracks.push(MusicTrackItem {
                path,
                name: name.clone(),
                duration_sec: dur,
                in_sec: 0.0,
                start_sec: self.playhead_sec,
                volume_pct: 60.0,
            });
            self.set_status(format!("Added A2 Music Track: {}", name), false);
        }
    }

    fn split_selected_at_playhead(&mut self) {
        let Some(idx) = self.selected_clip_idx else {
            return;
        };
        if idx >= self.clips.len() {
            return;
        }
        self.push_undo();
        let c = self.clips[idx].clone();
        let mid = (c.trim_start_sec + c.trim_end_sec) * 0.5;
        if mid > c.trim_start_sec + 0.05 && mid < c.trim_end_sec - 0.05 {
            let mut first = c.clone();
            first.trim_end_sec = mid;
            let mut second = c;
            second.trim_start_sec = mid;
            self.clips[idx] = first;
            self.clips.insert(idx + 1, second);
            self.selected_clip_idx = Some(idx + 1);
            self.set_status("Split clip at playhead.", false);
        }
    }

    fn duplicate_selected_clip(&mut self) {
        let Some(idx) = self.selected_clip_idx else {
            return;
        };
        if let Some(c) = self.clips.get(idx).cloned() {
            self.push_undo();
            self.clips.insert(idx + 1, c);
            self.selected_clip_idx = Some(idx + 1);
            self.set_status("Duplicated clip.", false);
        }
    }

    fn reset_effects_to_none(&mut self) {
        if let Some(idx) = self.selected_clip_idx {
            if let Some(c) = self.clips.get_mut(idx) {
                c.fx = VideoFxParams::default();
            }
        }
        self.keyframes.clear();
        self.text_overlays.clear();
        self.active_ofx_identifiers.clear();
        self.set_status("Reset all effects, curves, wheels, and keyframes to neutral.", false);
    }

    fn trigger_video_export(&mut self) {
        if self.clips.is_empty() {
            self.set_status("Add a clip first.", true);
            return;
        }
        if let Err(err) = validate_video_export(&self.video_export) {
            self.set_status(err, true);
            return;
        }
        let ext = self.video_export.container.ext();
        let default_name = format!("{}.{}", self.project.name.replace(' ', "_"), ext);
        if let Some(out_path) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter(ext.to_uppercase(), &[ext])
            .save_file()
        {
            self.progress_pct = Some(25.0);
            self.set_status("Rendering & encoding video via Rust + VideoToolbox... 25%", false);
            match export_video_sequence(
                &self.clips,
                &self.text_overlays,
                &self.project,
                &self.video_export,
                &out_path,
            ) {
                Ok(msg) => {
                    self.progress_pct = Some(100.0);
                    self.set_status(format!("{} (100%)", msg), false);
                }
                Err(e) => {
                    self.progress_pct = None;
                    self.set_status(e, true);
                }
            }
        }
    }
}

fn compute_waveform_peaks(samples: &[f32], bins: usize) -> Vec<(f32, f32)> {
    if samples.is_empty() || bins == 0 {
        return Vec::new();
    }
    let step = (samples.len() / bins).max(1);
    let mut out = Vec::with_capacity(bins);
    for b in 0..bins {
        let s0 = b * step;
        if s0 >= samples.len() {
            break;
        }
        let s1 = (s0 + step).min(samples.len());
        let mut mn = 0.0f32;
        let mut mx = 0.0f32;
        for &v in &samples[s0..s1] {
            if v < mn {
                mn = v;
            }
            if v > mx {
                mx = v;
            }
        }
        out.push((mn, mx));
    }
    out
}

fn format_timecode(sec: f64) -> String {
    let s = sec.max(0.0);
    let mins = (s / 60.0).floor() as u64;
    let rem = s - (mins as f64 * 60.0);
    format!("{:02}:{:05.2}", mins, rem)
}

impl eframe::App for GrainStudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(14, 16, 20);
        visuals.window_fill = egui::Color32::from_rgb(22, 25, 32);
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(28, 32, 40);
        visuals.selection.bg_fill = egui::Color32::from_rgb(45, 212, 191);
        visuals.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(6, 32, 29));
        ctx.set_visuals(visuals);

        // Top Mode Bar: Grain | Audio | Video
        egui::TopBottomPanel::top("grain_mode_bar").show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Grain")
                        .size(16.0)
                        .strong()
                        .color(egui::Color32::from_rgb(232, 234, 239)),
                );
                ui.add_space(10.0);
                ui.selectable_value(&mut self.mode, AppMode::Audio, "Audio");
                ui.selectable_value(&mut self.mode, AppMode::Video, "Video");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "100% Pure Rust • OpenFX 1.5 Host ({} plugins) • Apple Silicon M3",
                            self.ofx_plugins.len()
                        ))
                        .small()
                        .color(egui::Color32::from_rgb(139, 146, 161)),
                    );
                });
            });
            ui.add_space(5.0);
        });

        // Bottom Status Bar with Processing Percentage
        egui::TopBottomPanel::bottom("grain_status_bar").show(ctx, |ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                let col = if self.status_error {
                    egui::Color32::from_rgb(251, 113, 133)
                } else {
                    egui::Color32::from_rgb(45, 212, 191)
                };
                ui.label(egui::RichText::new(&self.status_text).size(12.0).color(col));
                if let Some(pct) = self.progress_pct {
                    ui.add(
                        egui::ProgressBar::new((pct / 100.0).clamp(0.0, 1.0))
                            .text(format!("{:.0}%", pct))
                            .desired_width(180.0),
                    );
                }
            });
            ui.add_space(3.0);
        });

        if self.show_project_modal {
            self.render_project_modal(ctx);
        }

        match self.mode {
            AppMode::Audio => self.render_audio_mode(ctx),
            AppMode::Video => self.render_video_mode(ctx),
        }
    }
}

impl GrainStudioApp {
    fn render_project_modal(&mut self, ctx: &egui::Context) {
        egui::Window::new("Project settings")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.heading("Details");
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.project.name);
                });
                egui::ComboBox::from_label("Color space")
                    .selected_text(&self.project.color_space)
                    .show_ui(ui, |ui| {
                        for cs in ["Rec. 709 SDR", "sRGB", "Rec. 2020 HDR"] {
                            ui.selectable_value(
                                &mut self.project.color_space,
                                cs.to_string(),
                                cs,
                            );
                        }
                    });
                ui.separator();
                ui.heading("Aspect ratio");
                egui::ComboBox::from_label("Aspect ratio")
                    .selected_text(self.project.aspect.label())
                    .show_ui(ui, |ui| {
                        for &ar in AspectRatioPreset::ALL {
                            ui.selectable_value(&mut self.project.aspect, ar, ar.label());
                        }
                    });
                egui::ComboBox::from_label("Framing")
                    .selected_text(self.project.framing.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.project.framing,
                            FramingMode::FitLetterbox,
                            FramingMode::FitLetterbox.label(),
                        );
                        ui.selectable_value(
                            &mut self.project.framing,
                            FramingMode::FillCrop,
                            FramingMode::FillCrop.label(),
                        );
                    });
                ui.separator();
                ui.heading("Timeline settings");
                egui::ComboBox::from_label("Resolution")
                    .selected_text(self.video_export.resolution.label())
                    .show_ui(ui, |ui| {
                        for &r in ResolutionPreset::ALL {
                            ui.selectable_value(&mut self.video_export.resolution, r, r.label());
                        }
                    });
                ui.add(egui::Slider::new(&mut self.video_export.fps, 24..=120).text("Frame rate"));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Close / Save").clicked() {
                        self.show_project_modal = false;
                    }
                });
            });
    }

    fn render_audio_mode(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_max_width(820.0);
                    ui.add_space(12.0);
                    ui.heading("Grain");
                    ui.label("Upload audio, then shift pitch, stretch time, or add vibrato using a phase vocoder, WSOLA, Rubberband, or Bungee — each effect names its own trade-off.");
                    ui.add_space(12.0);

                    // Panel 1: Drop / File Picker + Audacity-style Stereo Track View
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_min_width(780.0);
                        ui.horizontal(|ui| {
                            if ui.button("📂 Choose Audio / Video File...").clicked() {
                                self.import_audio_file();
                            }
                            ui.label(egui::RichText::new(&self.audio_filename).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.track_gain, 0.0..=1.5)
                                        .text("Gain"),
                                );
                                let mute_lbl = if self.audio_state.track_muted {
                                    "Unmute"
                                } else {
                                    "Mute"
                                };
                                if ui.button(mute_lbl).clicked() {
                                    self.audio_state.track_muted = !self.audio_state.track_muted;
                                }
                            });
                        });

                        ui.add_space(8.0);
                        let desired = egui::vec2(ui.available_width(), 190.0);
                        let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
                        if resp.clicked() {
                            if let Some(pos) = resp.interact_pointer_pos() {
                                self.audio_playhead_frac =
                                    ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                            }
                        }
                        let painter = ui.painter();
                        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(238, 241, 255));
                        let ax_w = 42.0;
                        let ax_rect =
                            egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + ax_w, rect.max.y));
                        painter.rect_filled(ax_rect, 0.0, egui::Color32::from_rgb(223, 227, 245));

                        let band_h = rect.height() * 0.5;
                        for (ch_idx, peaks) in [&self.audio_peaks_l, &self.audio_peaks_r]
                            .iter()
                            .enumerate()
                        {
                            let top = rect.top() + ch_idx as f32 * band_h;
                            let mid = top + band_h * 0.5;
                            let amp = band_h * 0.5 - 6.0;
                            painter.line_segment(
                                [egui::pos2(rect.left() + ax_w, mid), egui::pos2(rect.right(), mid)],
                                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(198, 203, 230)),
                            );
                            if !peaks.is_empty() {
                                let pw = rect.width() - ax_w;
                                let step_x = pw / peaks.len() as f32;
                                for (i, &(mn, mx)) in peaks.iter().enumerate() {
                                    let x = rect.left() + ax_w + i as f32 * step_x;
                                    let y_top = mid - mx * amp;
                                    let y_bot = mid - mn * amp;
                                    painter.line_segment(
                                        [egui::pos2(x, y_top), egui::pos2(x, y_bot.max(y_top + 1.0))],
                                        egui::Stroke::new(
                                            step_x.max(1.2_f32),
                                            egui::Color32::from_rgb(91, 95, 214),
                                        ),
                                    );
                                }
                            }
                        }
                        let ph_x = rect.left() + ax_w + (rect.width() - ax_w) * self.audio_playhead_frac;
                        painter.line_segment(
                            [egui::pos2(ph_x, rect.top()), egui::pos2(ph_x, rect.bottom())],
                            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(251, 113, 133)),
                        );
                    });

                    ui.add_space(10.0);

                    // Panel 2: All 12 Effect Tabs + Parameters + Export Format
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_min_width(780.0);
                        ui.horizontal_wrapped(|ui| {
                            for &tab in AudioTab::ALL {
                                ui.selectable_value(
                                    &mut self.audio_state.active_tab,
                                    tab,
                                    tab.label(),
                                );
                            }
                        });
                        ui.separator();

                        match self.audio_state.active_tab {
                            AudioTab::PitchShift => {
                                egui::ComboBox::from_label("Technique")
                                    .selected_text(self.audio_state.pitch_tech.label())
                                    .show_ui(ui, |ui| {
                                        for &t in StretchTech::ALL {
                                            ui.selectable_value(
                                                &mut self.audio_state.pitch_tech,
                                                t,
                                                t.label(),
                                            );
                                        }
                                    });
                                ui.add(
                                    egui::Slider::new(
                                        &mut self.audio_state.pitch_semitones,
                                        -24.0..=24.0,
                                    )
                                    .text("Semitones"),
                                );
                                egui::ComboBox::from_label("FFT size (samples)")
                                    .selected_text(format!("{}", self.audio_state.fft_size))
                                    .show_ui(ui, |ui| {
                                        for sz in [512usize, 1024, 2048, 4096, 8192] {
                                            ui.selectable_value(
                                                &mut self.audio_state.fft_size,
                                                sz,
                                                format!("{}", sz),
                                            );
                                        }
                                    });
                                ui.add(
                                    egui::Slider::new(
                                        &mut self.audio_state.formant_semitones,
                                        -12.0..=12.0,
                                    )
                                    .text("Formant / timbre (semitones)"),
                                );
                                ui.checkbox(
                                    &mut self.audio_state.formant_keep,
                                    "Preserve original formants when shifting pitch",
                                );
                            }
                            AudioTab::TimeStretch => {
                                egui::ComboBox::from_label("Technique")
                                    .selected_text(self.audio_state.time_tech.label())
                                    .show_ui(ui, |ui| {
                                        for &t in StretchTech::ALL {
                                            ui.selectable_value(
                                                &mut self.audio_state.time_tech,
                                                t,
                                                t.label(),
                                            );
                                        }
                                    });
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.time_factor, 0.4..=2.5)
                                        .text("Duration factor (pitch unchanged)"),
                                );
                            }
                            AudioTab::Vibrato => {
                                ui.horizontal(|ui| {
                                    ui.selectable_value(
                                        &mut self.audio_state.vibrato_pitch_only,
                                        true,
                                        "a — pitch only",
                                    );
                                    ui.selectable_value(
                                        &mut self.audio_state.vibrato_pitch_only,
                                        false,
                                        "b — pitch + speed (native)",
                                    );
                                });
                                if self.audio_state.vibrato_pitch_only {
                                    egui::ComboBox::from_label("Technique")
                                        .selected_text(self.audio_state.vibrato_tech.label())
                                        .show_ui(ui, |ui| {
                                            for &t in StretchTech::ALL {
                                                ui.selectable_value(
                                                    &mut self.audio_state.vibrato_tech,
                                                    t,
                                                    t.label(),
                                                );
                                            }
                                        });
                                }
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.vibrato_rate, 1.0..=12.0)
                                        .text("Rate (Hz)"),
                                );
                                ui.add(
                                    egui::Slider::new(
                                        &mut self.audio_state.vibrato_depth,
                                        0.1..=4.0,
                                    )
                                    .text("Depth (semitones)"),
                                );
                            }
                            AudioTab::SpeedPitch => {
                                ui.add(
                                    egui::Slider::new(
                                        &mut self.audio_state.speed_pitch_rate,
                                        0.4..=2.5,
                                    )
                                    .text("Rate (x)"),
                                );
                            }
                            AudioTab::Bitcrush => {
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.bit_depth, 1..=16)
                                        .text("Bit depth"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.sr_divisor, 1..=40)
                                        .text("Sample-rate divisor"),
                                );
                            }
                            AudioTab::Vocoder => {
                                egui::ComboBox::from_label("Carrier waveform")
                                    .selected_text(self.audio_state.vocoder_wave.label())
                                    .show_ui(ui, |ui| {
                                        for &w in CarrierWave::ALL {
                                            ui.selectable_value(
                                                &mut self.audio_state.vocoder_wave,
                                                w,
                                                w.label(),
                                            );
                                        }
                                    });
                                ui.add(
                                    egui::Slider::new(
                                        &mut self.audio_state.vocoder_freq,
                                        40.0..=440.0,
                                    )
                                    .text("Carrier pitch (Hz)"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.vocoder_bands, 4..=64)
                                        .text("Bands"),
                                );
                            }
                            AudioTab::Tremolo => {
                                egui::ComboBox::from_label("Waveform")
                                    .selected_text(self.audio_state.tremolo_wave.label())
                                    .show_ui(ui, |ui| {
                                        for &w in TremoloWave::ALL {
                                            ui.selectable_value(
                                                &mut self.audio_state.tremolo_wave,
                                                w,
                                                w.label(),
                                            );
                                        }
                                    });
                                ui.add(
                                    egui::Slider::new(
                                        &mut self.audio_state.tremolo_rate,
                                        0.5..=20.0,
                                    )
                                    .text("Rate (Hz)"),
                                );
                                ui.add(
                                    egui::Slider::new(
                                        &mut self.audio_state.tremolo_depth,
                                        0.0..=1.0,
                                    )
                                    .text("Depth"),
                                );
                            }
                            AudioTab::Invert => {
                                ui.label("Flips polarity sample-by-sample.");
                            }
                            AudioTab::MultiPitch => {
                                egui::ComboBox::from_label("Technique")
                                    .selected_text(self.audio_state.mp_tech.label())
                                    .show_ui(ui, |ui| {
                                        for &t in StretchTech::ALL {
                                            ui.selectable_value(
                                                &mut self.audio_state.mp_tech,
                                                t,
                                                t.label(),
                                            );
                                        }
                                    });
                                let labels = ["-12", "-7", "-5", "-3", "0", "+3", "+5", "+7", "+12"];
                                ui.horizontal_wrapped(|ui| {
                                    for (i, lbl) in labels.iter().enumerate() {
                                        ui.checkbox(&mut self.audio_state.mp_intervals[i], *lbl);
                                    }
                                });
                            }
                            AudioTab::MoreFx => {
                                egui::ComboBox::from_label("Effect")
                                    .selected_text(self.audio_state.more_type.label())
                                    .show_ui(ui, |ui| {
                                        for &m in MoreFxType::ALL {
                                            ui.selectable_value(
                                                &mut self.audio_state.more_type,
                                                m,
                                                m.label(),
                                            );
                                        }
                                    });
                                match self.audio_state.more_type {
                                    MoreFxType::Reverse => {}
                                    MoreFxType::Echo => {
                                        ui.add(
                                            egui::Slider::new(
                                                &mut self.audio_state.echo_delay_ms,
                                                10.0..=800.0,
                                            )
                                            .text("Delay (ms)"),
                                        );
                                        ui.add(
                                            egui::Slider::new(
                                                &mut self.audio_state.echo_feedback,
                                                0.0..=0.95,
                                            )
                                            .text("Feedback"),
                                        );
                                    }
                                    MoreFxType::Distortion => {
                                        ui.add(
                                            egui::Slider::new(
                                                &mut self.audio_state.dist_drive,
                                                1.0..=20.0,
                                            )
                                            .text("Drive"),
                                        );
                                    }
                                }
                            }
                            AudioTab::StudioFx => {
                                egui::ComboBox::from_label("Studio Effect (40 DSP processors)")
                                    .selected_text(self.audio_state.studio_type.label())
                                    .show_ui(ui, |ui| {
                                        for &sfx in StudioFxType::ALL {
                                            ui.selectable_value(
                                                &mut self.audio_state.studio_type,
                                                sfx,
                                                sfx.label(),
                                            );
                                        }
                                    });
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.sf_rate, 0.1..=10.0)
                                        .text("Rate (Hz)"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.sf_depth, 0.0..=1.0)
                                        .text("Depth"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.sf_mix, 0.0..=1.0)
                                        .text("Mix"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.sf_decay, 0.3..=6.0)
                                        .text("Reverb Decay (s)"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.sf_fc, 50.0..=12000.0)
                                        .text("Filter / Carrier Freq (Hz)"),
                                );
                                ui.add(
                                    egui::Slider::new(
                                        &mut self.audio_state.sf_gain_db,
                                        -24.0..=24.0,
                                    )
                                    .text("Gain / Bass Boost (dB)"),
                                );
                                ui.add(
                                    egui::Slider::new(
                                        &mut self.audio_state.sf_fade_in,
                                        0.0..=10.0,
                                    )
                                    .text("Fade In (s)"),
                                );
                                ui.add(
                                    egui::Slider::new(
                                        &mut self.audio_state.sf_fade_out,
                                        0.0..=10.0,
                                    )
                                    .text("Fade Out (s)"),
                                );
                                ui.collapsing("Advanced Studio Parameters (EQ / Drive / Gate / Pan / Sub)", |ui| {
                                    ui.add(egui::Slider::new(&mut self.audio_state.sf_p1, -12.0..=30.0).text("Param 1 (Low EQ / Drive / Pan / Threshold)"));
                                    ui.add(egui::Slider::new(&mut self.audio_state.sf_p2, -12.0..=12.0).text("Param 2 (Mid EQ / Feedback / Release)"));
                                    ui.add(egui::Slider::new(&mut self.audio_state.sf_p3, 50.0..=12000.0).text("Param 3 (Freq Hz / Delay ms)"));
                                    ui.add(egui::Slider::new(&mut self.audio_state.sf_p4, -12.0..=12.0).text("Param 4 (High EQ / Mix / Depth)"));
                                });
                            }
                            AudioTab::Rubberband => {
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.rb_semis, -24.0..=24.0)
                                        .text("Pitch (semitones)"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut self.audio_state.rb_tempo, 0.25..=4.0)
                                        .text("Tempo (speed)"),
                                );
                                ui.horizontal(|ui| {
                                    ui.label("Formant v4 (0..2,147,483,647; 0=auto):");
                                    ui.add(
                                        egui::DragValue::new(&mut self.audio_state.rb_formant_v4)
                                            .range(0..=2_147_483_647u32),
                                    );
                                });
                                egui::ComboBox::from_label("Formant (FFmpeg af_rubberband)")
                                    .selected_text(&self.audio_state.rb_formant)
                                    .show_ui(ui, |ui| {
                                        for v in ["unset", "preserved", "shifted"] {
                                            ui.selectable_value(
                                                &mut self.audio_state.rb_formant,
                                                v.into(),
                                                v,
                                            );
                                        }
                                    });
                                egui::ComboBox::from_label("Transients")
                                    .selected_text(&self.audio_state.rb_transients)
                                    .show_ui(ui, |ui| {
                                        for v in ["crisp", "mixed", "smooth"] {
                                            ui.selectable_value(
                                                &mut self.audio_state.rb_transients,
                                                v.into(),
                                                v,
                                            );
                                        }
                                    });
                                ui.label("FFmpeg CLI Command:");
                                let mut cli = self.audio_state.rb_ffmpeg_cli();
                                ui.add(
                                    egui::TextEdit::multiline(&mut cli)
                                        .desired_rows(2)
                                        .desired_width(f32::INFINITY),
                                );
                            }
                        }

                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(self.audio_state.active_tab.description())
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                        ui.separator();

                        ui.horizontal(|ui| {
                            egui::ComboBox::from_label("Format")
                                .selected_text(self.audio_state.export_format.label())
                                .show_ui(ui, |ui| {
                                    for &fmt in AudioExportFormat::ALL {
                                        ui.selectable_value(
                                            &mut self.audio_state.export_format,
                                            fmt,
                                            fmt.label(),
                                        );
                                    }
                                });
                            egui::ComboBox::from_label("Sample Rate")
                                .selected_text(format!("{} kHz", self.audio_state.export_sample_rate / 1000))
                                .show_ui(ui, |ui| {
                                    for sr in [44_100u32, 48_000, 96_000, 192_000] {
                                        ui.selectable_value(
                                            &mut self.audio_state.export_sample_rate,
                                            sr,
                                            format!("{} kHz", sr / 1000),
                                        );
                                    }
                                });
                            egui::ComboBox::from_label("Bitrate")
                                .selected_text(format!("{} kbps", self.audio_state.export_bitrate_kbps))
                                .show_ui(ui, |ui| {
                                    for br in [128u32, 192, 256, 320, 512] {
                                        ui.selectable_value(
                                            &mut self.audio_state.export_bitrate_kbps,
                                            br,
                                            format!("{} kbps", br),
                                        );
                                    }
                                });
                        });

                        ui.add_space(10.0);
                        let run_btn = egui::Button::new(
                            egui::RichText::new("Process & Export Audio")
                                .strong()
                                .color(egui::Color32::from_rgb(6, 32, 29)),
                        )
                        .fill(egui::Color32::from_rgb(45, 212, 191))
                        .min_size(egui::vec2(ui.available_width(), 38.0));

                        if ui.add(run_btn).clicked() {
                            self.run_audio_process();
                        }
                    });
                });
            });
        });
    }

    fn render_video_mode(&mut self, ctx: &egui::Context) {
        // 1. Left Library Panel
        egui::SidePanel::left("cap_lib_panel")
            .min_width(360.0)
            .default_width(385.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for &page in LibraryPage::ALL {
                        ui.selectable_value(&mut self.lib_page, page, page.label());
                    }
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| match self.lib_page {
                    LibraryPage::Media => self.render_lib_media(ui),
                    LibraryPage::Effects => self.render_lib_effects(ui),
                    LibraryPage::Transitions => self.render_lib_transitions(ui),
                    LibraryPage::Text => self.render_lib_text(ui),
                    LibraryPage::Adjust => self.render_lib_adjust(ui),
                    LibraryPage::OpenFx => self.render_lib_openfx(ui),
                    LibraryPage::Export => self.render_lib_export(ui),
                });
            });

        // 2. Right Details & Keyframe Easing Curve Editor Panel
        egui::SidePanel::right("cap_details_panel")
            .min_width(300.0)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Details")
                            .strong()
                            .color(egui::Color32::from_rgb(60, 207, 224)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Project settings").clicked() {
                            self.show_project_modal = true;
                        }
                    });
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let dur_str = if let Some(idx) = self.selected_clip_idx {
                        self.clips
                            .get(idx)
                            .map(|c| format!("{:.2} s", c.effective_duration()))
                            .unwrap_or_else(|| "—".into())
                    } else {
                        "—".into()
                    };
                    egui::Grid::new("details_grid").num_columns(2).show(ui, |ui| {
                        ui.label("Name:");
                        ui.label(&self.project.name);
                        ui.end_row();
                        ui.label("Source size:");
                        ui.label(format!("{}×{}", self.source_res.0, self.source_res.1));
                        ui.end_row();
                        ui.label("Duration:");
                        ui.label(dur_str);
                        ui.end_row();
                        ui.label("Export size:");
                        ui.label(self.video_export.resolution.label());
                        ui.end_row();
                        ui.label("Export frame rate:");
                        ui.label(format!("{} fps", self.video_export.fps));
                        ui.end_row();
                        ui.label("Active OpenFX:");
                        ui.label(format!("{} loaded", self.active_ofx_identifiers.len()));
                        ui.end_row();
                    });

                    ui.separator();
                    ui.label("Keyframe easing (from this key to the next)");
                    egui::ComboBox::from_id_salt("kf_easing_box")
                        .selected_text(self.selected_easing.label())
                        .show_ui(ui, |ui| {
                            for &ec in EasingCurve::ALL {
                                ui.selectable_value(&mut self.selected_easing, ec, ec.label());
                            }
                        });

                    ui.add_space(6.0);
                    ui.label("Curve editor preview:");
                    let (curve_rect, _) =
                        ui.allocate_exact_size(egui::vec2(180.0, 180.0), egui::Sense::hover());
                    let p = ui.painter();
                    p.rect_filled(curve_rect, 4.0, egui::Color32::from_rgb(21, 22, 26));
                    p.rect_stroke(
                        curve_rect,
                        4.0,
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(55, 60, 72)),
                    );
                    let mut pts = Vec::with_capacity(51);
                    for i in 0..=50 {
                        let t = i as f32 / 50.0;
                        let y_val = self.selected_easing.eval(t);
                        let px = curve_rect.left() + 12.0 + t * (curve_rect.width() - 24.0);
                        let py = curve_rect.bottom() - 24.0 - y_val * (curve_rect.height() - 48.0);
                        pts.push(egui::pos2(px, py));
                    }
                    p.add(egui::Shape::line(
                        pts,
                        egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(34, 199, 184)),
                    ));

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("◆ Add keyframe").clicked() {
                            if let Some(idx) = self.selected_clip_idx {
                                if let Some(c) = self.clips.get(idx) {
                                    self.keyframes.push(VideoKeyframe {
                                        time_sec: self.playhead_sec,
                                        easing: self.selected_easing,
                                        brightness: c.fx.brightness,
                                        contrast: c.fx.contrast,
                                        saturation: c.fx.saturation,
                                        hue: c.fx.hue_deg,
                                        blur: c.fx.blur_px,
                                        zoom: c.fx.zoom_pct,
                                        rotate: c.fx.rotate_deg,
                                    });
                                    self.set_status(
                                        format!(
                                            "Added keyframe @ {:.2}s ({})",
                                            self.playhead_sec,
                                            self.selected_easing.label()
                                        ),
                                        false,
                                    );
                                }
                            }
                        }
                        if ui.button("Clear keyframes").clicked() {
                            self.keyframes.clear();
                            self.set_status("Cleared all keyframes.", false);
                        }
                    });
                    if !self.keyframes.is_empty() {
                        ui.label(format!("Active keyframes: {}", self.keyframes.len()));
                    }
                });
            });

        // 3. Bottom Vegas Pro 15-Style Multi-Track Timeline Panel
        egui::TopBottomPanel::bottom("vegas_timeline_panel")
            .min_height(250.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.button("+ Add video clip").clicked() {
                        self.import_video_clip();
                    }
                    if ui.button("✂ Split").clicked() {
                        self.split_selected_at_playhead();
                    }
                    if ui.button("✕ Delete").clicked() {
                        if let Some(idx) = self.selected_clip_idx {
                            if idx < self.clips.len() {
                                self.push_undo();
                                self.clips.remove(idx);
                                self.selected_clip_idx = if self.clips.is_empty() {
                                    None
                                } else {
                                    Some(idx.min(self.clips.len() - 1))
                                };
                            }
                        }
                    }
                    if ui.button("◀").clicked() {
                        if let Some(idx) = self.selected_clip_idx {
                            if idx > 0 {
                                self.push_undo();
                                self.clips.swap(idx, idx - 1);
                                self.selected_clip_idx = Some(idx - 1);
                            }
                        }
                    }
                    if ui.button("▶").clicked() {
                        if let Some(idx) = self.selected_clip_idx {
                            if idx + 1 < self.clips.len() {
                                self.push_undo();
                                self.clips.swap(idx, idx + 1);
                                self.selected_clip_idx = Some(idx + 1);
                            }
                        }
                    }
                    if ui.button("+ Overlay").clicked() {
                        self.import_overlay_clip();
                    }
                    if ui.button("+ Audio clip").clicked() {
                        self.import_music_clip();
                    }
                    if ui.button("+ Text").clicked() {
                        self.push_undo();
                        self.text_overlays.push(self.draft_text.clone());
                    }
                    if ui.button("Undo").clicked() {
                        self.undo();
                    }
                    if ui.button("Redo").clicked() {
                        self.redo();
                    }
                    if ui.button("Duplicate").clicked() {
                        self.duplicate_selected_clip();
                    }
                    let snap_lbl = if self.snap_enabled {
                        "Snap: on"
                    } else {
                        "Snap: off"
                    };
                    if ui.button(snap_lbl).clicked() {
                        self.snap_enabled = !self.snap_enabled;
                    }
                    ui.add(egui::Slider::new(&mut self.tl_zoom, 15.0..=180.0).text("Zoom"));
                    let exp_seq = egui::Button::new(
                        egui::RichText::new("▶ Export sequence")
                            .strong()
                            .color(egui::Color32::BLACK),
                    )
                    .fill(egui::Color32::from_rgb(60, 207, 224));
                    if ui.add(exp_seq).clicked() {
                        self.trigger_video_export();
                    }
                });
                ui.separator();

                let total_dur = self.total_timeline_duration();
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    let row_h = 34.0;
                    let header_w = 135.0;
                    let canvas_w = (header_w + total_dur as f32 * self.tl_zoom + 120.0)
                        .max(ui.available_width());
                    let canvas_h = 22.0 + row_h * 5.0;
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(canvas_w, canvas_h), egui::Sense::click());

                    if resp.clicked() {
                        if let Some(pos) = resp.interact_pointer_pos() {
                            if pos.x > rect.left() + header_w {
                                self.playhead_sec = ((pos.x - (rect.left() + header_w)) / self.tl_zoom)
                                    .max(0.0) as f64;
                            }
                        }
                    }

                    let p = ui.painter();
                    p.rect_filled(rect, 2.0, egui::Color32::from_rgb(43, 43, 43));

                    let lanes = [
                        ("T1", "Text 1", egui::Color32::from_rgb(226, 182, 74)),
                        ("V2", "Overlay 2", egui::Color32::from_rgb(91, 143, 224)),
                        ("V1", "Video 1", egui::Color32::from_rgb(63, 182, 187)),
                        ("A1", "Audio 1", egui::Color32::from_rgb(143, 116, 217)),
                        ("A2", "Music 2", egui::Color32::from_rgb(154, 132, 217)),
                    ];

                    for (idx, (badge, name, col)) in lanes.iter().enumerate() {
                        let y0 = rect.top() + 22.0 + idx as f32 * row_h;
                        let hdr_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.left(), y0),
                            egui::vec2(header_w, row_h),
                        );
                        p.rect_filled(hdr_rect, 0.0, egui::Color32::from_rgb(58, 58, 58));
                        let badge_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.left() + 8.0, y0 + 8.0),
                            egui::vec2(22.0, 18.0),
                        );
                        p.rect_filled(badge_rect, 2.0, *col);
                        p.text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            *badge,
                            egui::FontId::proportional(10.0),
                            egui::Color32::BLACK,
                        );
                        p.text(
                            egui::pos2(rect.left() + 36.0, y0 + row_h * 0.5),
                            egui::Align2::LEFT_CENTER,
                            *name,
                            egui::FontId::proportional(11.0),
                            egui::Color32::from_rgb(230, 230, 230),
                        );
                    }

                    let mut x_cursor = rect.left() + header_w;
                    let y_v1 = rect.top() + 22.0 + 2.0 * row_h + 3.0;
                    let y_a1 = rect.top() + 22.0 + 3.0 * row_h + 3.0;
                    for (i, clip) in self.clips.iter().enumerate() {
                        let w = (clip.effective_duration() as f32 * self.tl_zoom).max(24.0);
                        let v_rect = egui::Rect::from_min_size(
                            egui::pos2(x_cursor, y_v1),
                            egui::vec2(w, row_h - 6.0),
                        );
                        let a_rect = egui::Rect::from_min_size(
                            egui::pos2(x_cursor, y_a1),
                            egui::vec2(w, row_h - 6.0),
                        );
                        let is_sel = self.selected_clip_idx == Some(i);
                        p.rect_filled(
                            v_rect,
                            3.0,
                            if is_sel {
                                egui::Color32::from_rgb(113, 210, 213)
                            } else {
                                egui::Color32::from_rgb(58, 167, 173)
                            },
                        );
                        if is_sel {
                            p.rect_stroke(
                                v_rect,
                                3.0,
                                egui::Stroke::new(1.5_f32, egui::Color32::WHITE),
                            );
                        }
                        p.text(
                            egui::pos2(v_rect.left() + 6.0, v_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &clip.name,
                            egui::FontId::proportional(10.5),
                            egui::Color32::BLACK,
                        );
                        p.rect_filled(a_rect, 3.0, egui::Color32::from_rgb(110, 88, 175));
                        x_cursor += w + 2.0;
                    }

                    let y_t1 = rect.top() + 22.0 + 3.0;
                    for tx in &self.text_overlays {
                        let x0 = rect.left() + header_w + tx.start_sec as f32 * self.tl_zoom;
                        let dur = if tx.end_sec > tx.start_sec {
                            (tx.end_sec - tx.start_sec) as f32
                        } else {
                            3.0
                        };
                        let r = egui::Rect::from_min_size(
                            egui::pos2(x0, y_t1),
                            egui::vec2((dur * self.tl_zoom).max(30.0), row_h - 6.0),
                        );
                        p.rect_filled(r, 3.0, egui::Color32::from_rgb(242, 210, 122));
                        p.text(
                            egui::pos2(r.left() + 5.0, r.center().y),
                            egui::Align2::LEFT_CENTER,
                            &tx.text,
                            egui::FontId::proportional(10.0),
                            egui::Color32::BLACK,
                        );
                    }

                    let ph_x = rect.left() + header_w + self.playhead_sec as f32 * self.tl_zoom;
                    p.line_segment(
                        [egui::pos2(ph_x, rect.top()), egui::pos2(ph_x, rect.bottom())],
                        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(255, 93, 82)),
                    );
                });
            });

        // 4. Center Preview Monitor & Transport
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!("Preview · {}", self.project.name))
                        .strong()
                        .color(egui::Color32::from_rgb(232, 232, 232)),
                );
                ui.add_space(8.0);

                let avail = ui.available_size();
                let mon_w = (avail.x - 32.0).clamp(320.0, 780.0);
                let mon_h = (avail.y - 68.0).clamp(180.0, 440.0);
                let (mon_rect, _) =
                    ui.allocate_exact_size(egui::vec2(mon_w, mon_h), egui::Sense::hover());
                let p = ui.painter();
                p.rect_filled(mon_rect, 3.0, egui::Color32::BLACK);
                p.rect_stroke(
                    mon_rect,
                    3.0,
                    egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(38, 42, 52)),
                );

                if let Some(idx) = self.selected_clip_idx {
                    if let Some(c) = self.clips.get(idx) {
                        p.text(
                            egui::pos2(mon_rect.center().x, mon_rect.center().y - 16.0),
                            egui::Align2::CENTER_CENTER,
                            format!("{} ({:.2}s @ {:.2}x)", c.name, c.effective_duration(), c.speed),
                            egui::FontId::proportional(14.0),
                            egui::Color32::from_rgb(60, 207, 224),
                        );
                        p.text(
                            egui::pos2(mon_rect.center().x, mon_rect.center().y + 10.0),
                            egui::Align2::CENTER_CENTER,
                            format!(
                                "Aspect: {} ({}) • Color: {} • Keyframes: {}",
                                self.project.aspect.label(),
                                self.project.framing.label(),
                                self.project.color_space,
                                self.keyframes.len()
                            ),
                            egui::FontId::proportional(11.5),
                            egui::Color32::GRAY,
                        );
                    }
                } else {
                    p.text(
                        mon_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Drop or import a video file in Media to begin",
                        egui::FontId::proportional(13.0),
                        egui::Color32::GRAY,
                    );
                }

                for tx in &self.text_overlays {
                    let active = self.playhead_sec >= tx.start_sec
                        && (tx.end_sec <= 0.0 || self.playhead_sec <= tx.end_sec);
                    if active && !tx.text.is_empty() {
                        let px = mon_rect.left() + mon_rect.width() * (tx.x_pct / 100.0);
                        let py = mon_rect.top() + mon_rect.height() * (tx.y_pct / 100.0);
                        p.text(
                            egui::pos2(px, py),
                            egui::Align2::CENTER_CENTER,
                            &tx.text,
                            egui::FontId::proportional((tx.size_pct / 100.0 * mon_rect.height()).max(12.0)),
                            egui::Color32::from_rgb(tx.color[0], tx.color[1], tx.color[2]),
                        );
                    }
                }

                ui.add_space(8.0);
                let total = self.total_timeline_duration();
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 340.0).max(0.0) * 0.5);
                    if ui.button("|<").clicked() {
                        self.playhead_sec = 0.0;
                    }
                    if ui.button("<").clicked() {
                        self.playhead_sec = (self.playhead_sec - 1.0 / 30.0).max(0.0);
                    }
                    let play_txt = if self.is_playing {
                        "❚❚ Pause"
                    } else {
                        "▶ Play"
                    };
                    let play_btn = egui::Button::new(
                        egui::RichText::new(play_txt)
                            .strong()
                            .color(egui::Color32::from_rgb(6, 32, 29)),
                    )
                    .fill(egui::Color32::from_rgb(45, 212, 191));
                    if ui.add(play_btn).clicked() {
                        self.is_playing = !self.is_playing;
                    }
                    if ui.button(">").clicked() {
                        self.playhead_sec = (self.playhead_sec + 1.0 / 30.0).min(total);
                    }
                    if ui.button(">|").clicked() {
                        self.playhead_sec = total;
                    }
                    ui.label(format!(
                        "{} / {}",
                        format_timecode(self.playhead_sec),
                        format_timecode(total)
                    ));
                });
            });
        });
    }

    fn render_lib_media(&mut self, ui: &mut egui::Ui) {
        if ui.button("📂 Import Video Clip...").clicked() {
            self.import_video_clip();
        }
        if let Some(idx) = self.selected_clip_idx {
            if let Some(c) = self.clips.get_mut(idx) {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(&c.name).strong());
                ui.add(
                    egui::Slider::new(
                        &mut c.trim_start_sec,
                        0.0..=(c.trim_end_sec - 0.1).max(0.0),
                    )
                    .text("Trim In (s)"),
                );
                ui.add(
                    egui::Slider::new(
                        &mut c.trim_end_sec,
                        (c.trim_start_sec + 0.1)..=c.duration_sec,
                    )
                    .text("Trim Out (s)"),
                );
                ui.add(egui::Slider::new(&mut c.speed, 0.25..=3.0).text("Speed (x)"));
            }
        }
        ui.separator();
        ui.heading("Tracks (V2 Overlay & A2 Music)");
        if ui.button("+ Import Overlay Video (V2)...").clicked() {
            self.import_overlay_clip();
        }
        if let Some(ov) = self.overlays.last_mut() {
            ui.add(egui::Slider::new(&mut ov.x_pct, 0.0..=100.0).text("Overlay X %"));
            ui.add(egui::Slider::new(&mut ov.y_pct, 0.0..=100.0).text("Overlay Y %"));
            ui.add(egui::Slider::new(&mut ov.size_pct, 5.0..=100.0).text("Overlay Size %"));
            ui.add(egui::Slider::new(&mut ov.opacity_pct, 0.0..=100.0).text("Overlay Opacity %"));
        }
        ui.add_space(6.0);
        if ui.button("+ Import Music Track (A2)...").clicked() {
            self.import_music_clip();
        }
        if let Some(mu) = self.music_tracks.last_mut() {
            ui.add(egui::Slider::new(&mut mu.volume_pct, 0.0..=100.0).text("Music Volume %"));
        }
    }

    fn render_lib_effects(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.effect_search);
        });
        if ui.button("Reset to none").clicked() {
            self.reset_effects_to_none();
        }
        ui.add_space(4.0);

        let Some(idx) = self.selected_clip_idx else {
            ui.label("Import or select a clip on V1 to configure effects.");
            return;
        };
        let fx = &mut self.clips[idx].fx;
        let q = self.effect_search.trim().to_lowercase();
        let show = |name: &str| q.is_empty() || name.to_lowercase().contains(&q);

        if show("effects blur vignette grain scanlines pixelate glitch posterize rgb split kaleidoscope shake trail") {
            ui.collapsing("Effects", |ui| {
                ui.add(egui::Slider::new(&mut fx.blur_px, 0.0..=12.0).text("Blur (px)"));
                ui.add(egui::Slider::new(&mut fx.vignette, 0.0..=1.0).text("Vignette"));
                ui.add(egui::Slider::new(&mut fx.film_grain, 0.0..=1.0).text("Film grain"));
                ui.add(egui::Slider::new(&mut fx.scanlines, 0.0..=1.0).text("Scanlines"));
                ui.add(egui::Slider::new(&mut fx.pixelate, 0.0..=60.0).text("Pixelate (px)"));
                ui.add(egui::Slider::new(&mut fx.glitch, 0.0..=1.0).text("Glitch"));
                ui.add(egui::Slider::new(&mut fx.posterize, 0.0..=1.0).text("Posterize"));
                ui.add(egui::Slider::new(&mut fx.rgb_split, 0.0..=40.0).text("RGB split (px)"));
                ui.add(egui::Slider::new(&mut fx.kaleidoscope, 0..=12).text("Kaleidoscope segments"));
                ui.add(egui::Slider::new(&mut fx.camera_shake, 0.0..=40.0).text("Camera shake"));
                ui.add(egui::Slider::new(&mut fx.motion_trail, 0.0..=0.9).text("Motion trail"));
            });
        }

        if show("mask ellipse rectangle linear gradient feather invert") {
            ui.collapsing("Mask", |ui| {
                egui::ComboBox::from_label("Mask type")
                    .selected_text(fx.mask_type.label())
                    .show_ui(ui, |ui| {
                        for &mt in MaskType::ALL {
                            ui.selectable_value(&mut fx.mask_type, mt, mt.label());
                        }
                    });
                ui.add(egui::Slider::new(&mut fx.mask_x, 0.0..=100.0).text("Center X %"));
                ui.add(egui::Slider::new(&mut fx.mask_y, 0.0..=100.0).text("Center Y %"));
                ui.add(egui::Slider::new(&mut fx.mask_size, 5.0..=100.0).text("Size %"));
                ui.add(egui::Slider::new(&mut fx.mask_feather, 0.0..=80.0).text("Feather (px)"));
                ui.checkbox(&mut fx.mask_invert, "Invert mask");
            });
        }

        if show("light glow light leak strobe tilt-shift") {
            ui.collapsing("Light", |ui| {
                ui.add(egui::Slider::new(&mut fx.glow, 0.0..=100.0).text("Glow %"));
                ui.add(egui::Slider::new(&mut fx.light_leak, 0.0..=100.0).text("Light leak %"));
                ui.add(egui::Slider::new(&mut fx.strobe, 0.0..=100.0).text("Strobe %"));
                ui.add(egui::Slider::new(&mut fx.tilt_shift, 0.0..=100.0).text("Tilt-shift %"));
            });
        }

        if show("stylize sharpen duotone zoom blur") {
            ui.collapsing("Stylize", |ui| {
                ui.add(egui::Slider::new(&mut fx.sharpen, 0.0..=100.0).text("Sharpen %"));
                ui.add(egui::Slider::new(&mut fx.duotone, 0.0..=100.0).text("Duotone amount %"));
                ui.add(egui::Slider::new(&mut fx.zoom_blur, 0.0..=100.0).text("Zoom blur %"));
            });
        }

        if show("transform zoom rotate flip mirror letterbox") {
            ui.collapsing("Transform", |ui| {
                ui.add(egui::Slider::new(&mut fx.zoom_pct, 100.0..=250.0).text("Zoom (push-in) %"));
                ui.add(egui::Slider::new(&mut fx.rotate_deg, -180.0..=180.0).text("Rotate (°)"));
                ui.checkbox(&mut fx.flip_h, "Flip Horizontal");
                ui.checkbox(&mut fx.flip_v, "Flip Vertical");
                ui.add(egui::Slider::new(&mut fx.letterbox, 0.0..=40.0).text("Letterbox %"));
            });
        }

        if show("print neon halftone edges ripple rings") {
            ui.collapsing("Print & neon", |ui| {
                ui.add(egui::Slider::new(&mut fx.halftone_px, 0.0..=60.0).text("Halftone cell (px)"));
                ui.add(egui::Slider::new(&mut fx.neon_edges, 0.0..=100.0).text("Neon edges %"));
                ui.add(egui::Slider::new(&mut fx.ripple_rings, 0.0..=100.0).text("Ripple rings %"));
            });
        }

        if show("color fx solarize threshold bleach bypass channel swap") {
            ui.collapsing("Color FX", |ui| {
                ui.add(egui::Slider::new(&mut fx.solarize, 0.0..=100.0).text("Solarize %"));
                ui.add(egui::Slider::new(&mut fx.threshold_bw, 0.0..=100.0).text("Threshold (B/W) %"));
                ui.add(egui::Slider::new(&mut fx.bleach_bypass, 0.0..=100.0).text("Bleach bypass %"));
            });
        }

        if show("hue cycle tile repeat spotlight") {
            ui.collapsing("Hue cycle, tile, spotlight", |ui| {
                ui.add(egui::Slider::new(&mut fx.hue_cycle_speed, 0.0..=5.0).text("Hue cycle speed"));
                ui.add(egui::Slider::new(&mut fx.tile_repeat, 1..=8).text("Tile repeat"));
                ui.add(egui::Slider::new(&mut fx.spotlight_dark, 0.0..=100.0).text("Spotlight darkness %"));
                ui.add(egui::Slider::new(&mut fx.spotlight_radius, 5.0..=100.0).text("Spotlight radius %"));
            });
        }

        // BorisFX Sapphire S_ Light & Flares
        if show("sapphire light flares s_glow s_glownoise s_glowrainbow s_gloworthicon s_halation s_glare s_glint s_sparkle s_flashbulb s_rays s_streaks s_lensflare s_edgerays s_lightleak") {
            ui.collapsing("BorisFX Sapphire: Light & Flares (S_)", |ui| {
                ui.add(egui::Slider::new(&mut fx.s_glow, 0.0..=100.0).text("S_Glow %"));
                ui.add(egui::Slider::new(&mut fx.s_glow_noise, 0.0..=100.0).text("S_GlowNoise %"));
                ui.add(egui::Slider::new(&mut fx.s_glow_rainbow, 0.0..=100.0).text("S_GlowRainbow %"));
                ui.add(egui::Slider::new(&mut fx.s_glow_orthicon, 0.0..=100.0).text("S_GlowOrthicon (dark glow) %"));
                ui.add(egui::Slider::new(&mut fx.s_halation, 0.0..=100.0).text("S_Halation %"));
                ui.add(egui::Slider::new(&mut fx.s_glare, 0.0..=100.0).text("S_Glare %"));
                ui.add(egui::Slider::new(&mut fx.s_glint, 0.0..=100.0).text("S_Glint %"));
                ui.add(egui::Slider::new(&mut fx.s_sparkle, 0.0..=100.0).text("S_Sparkle %"));
                ui.add(egui::Slider::new(&mut fx.s_flashbulb, 0.0..=100.0).text("S_Flashbulb %"));
                ui.add(egui::Slider::new(&mut fx.s_rays, 0.0..=100.0).text("S_Rays %"));
                ui.add(egui::Slider::new(&mut fx.s_streaks, 0.0..=100.0).text("S_Streaks %"));
                ui.add(egui::Slider::new(&mut fx.s_lens_flare, 0.0..=100.0).text("S_LensFlare %"));
                ui.add(egui::Slider::new(&mut fx.s_edge_rays, 0.0..=100.0).text("S_EdgeRays %"));
                ui.add(egui::Slider::new(&mut fx.s_light_leak, 0.0..=100.0).text("S_LightLeak %"));
            });
        }

        // BorisFX Sapphire S_ Blur & Distort / Warps
        if show("sapphire blur distort warp s_warpwaves2 s_warpperspective s_warpfisheye s_warpmagnify s_warppolar s_warpbubble2 s_warpvortex s_warpchroma s_warppuddle s_warptransform s_warppuff s_blurdirectional s_blurradial s_blurmotion s_rackdefocus s_zblur s_edgeblur s_beauty") {
            ui.collapsing("BorisFX Sapphire: Blur & Distort (S_)", |ui| {
                ui.checkbox(&mut fx.waves_enabled, "S_WarpWaves2 (enable)");
                ui.add(egui::Slider::new(&mut fx.wave_a_amp, -1.0..=1.0).text("S_WarpWaves2 A amplitude"));
                ui.add(egui::Slider::new(&mut fx.wave_a_freq, 0.1..=40.0).text("S_WarpWaves2 A frequency"));
                ui.add(egui::Slider::new(&mut fx.wave_b_amp, -1.0..=1.0).text("S_WarpWaves2 B amplitude"));
                ui.checkbox(&mut fx.perspective_enabled, "S_WarpPerspective (enable)");
                ui.add(egui::Slider::new(&mut fx.persp_lat, -89.0..=89.0).text("S_WarpPerspective Latitude (°)"));
                ui.add(egui::Slider::new(&mut fx.persp_swing, -360.0..=360.0).text("S_WarpPerspective Swing (°)"));
                ui.add(egui::Slider::new(&mut fx.persp_roll, -360.0..=360.0).text("S_WarpPerspective Roll (°)"));
                ui.add(egui::Slider::new(&mut fx.cube_rot_deg, 0.0..=90.0).text("Cube rotation (°)"));
                ui.add(egui::Slider::new(&mut fx.s_fisheye, -3.0..=3.0).text("S_WarpFishEye"));
                ui.add(egui::Slider::new(&mut fx.s_magnify, 0.0..=8.0).text("S_WarpMagnify"));
                ui.checkbox(&mut fx.s_polar, "S_WarpPolar");
                ui.add(egui::Slider::new(&mut fx.s_bubble2, 0.0..=2.0).text("S_WarpBubble2"));
                ui.add(egui::Slider::new(&mut fx.s_vortex, -720.0..=720.0).text("S_WarpVortex (°)"));
                ui.add(egui::Slider::new(&mut fx.s_warp_chroma, 0.0..=2.0).text("S_WarpChroma"));
                ui.add(egui::Slider::new(&mut fx.s_warp_puddle, 0.0..=100.0).text("S_WarpPuddle %"));
                ui.add(egui::Slider::new(&mut fx.s_warp_transform_rot, -180.0..=180.0).text("S_WarpTransform Rotate (°)"));
                ui.add(egui::Slider::new(&mut fx.s_warp_puff, -100.0..=100.0).text("S_WarpPuff %"));
                ui.add(egui::Slider::new(&mut fx.s_blur_dir, 0.0..=100.0).text("S_BlurDirectional %"));
                ui.add(egui::Slider::new(&mut fx.s_blur_rad, 0.0..=100.0).text("S_BlurRadial %"));
                ui.add(egui::Slider::new(&mut fx.s_blur_motion, 0.0..=100.0).text("S_BlurMotion %"));
                ui.add(egui::Slider::new(&mut fx.s_rack_defocus, 0.0..=100.0).text("S_RackDefocus %"));
                ui.add(egui::Slider::new(&mut fx.s_z_blur, 0.0..=100.0).text("S_ZBlur %"));
                ui.add(egui::Slider::new(&mut fx.s_edge_blur, 0.0..=100.0).text("S_EdgeBlur %"));
                ui.add(egui::Slider::new(&mut fx.s_beauty, 0.0..=100.0).text("S_Beauty (skin smoothing) %"));
            });
        }

        // BorisFX Sapphire S_ Stylize, Render & Composite
        if show("sapphire stylize render composite s_quadtone s_multitone s_tritone s_zebrafy s_pixelsort s_embossdistort s_embossshiny s_zap s_aurora s_caustics s_clouds s_texturecells s_autopaint s_cartoon s_etching s_scanlines s_filmdamage s_shake s_psykostripes s_psykoblobs") {
            ui.collapsing("BorisFX Sapphire: Stylize & Render (S_)", |ui| {
                ui.add(egui::Slider::new(&mut fx.s_quadtone, 0.0..=100.0).text("S_QuadTone %"));
                ui.add(egui::Slider::new(&mut fx.s_multitone, 0.0..=100.0).text("S_MultiTone %"));
                ui.add(egui::Slider::new(&mut fx.s_tritone, 0.0..=100.0).text("S_TriTone %"));
                ui.add(egui::Slider::new(&mut fx.s_zebrafy, 0.0..=100.0).text("S_Zebrafy %"));
                ui.add(egui::Slider::new(&mut fx.s_pixel_sort, 0.0..=100.0).text("S_PixelSort %"));
                ui.add(egui::Slider::new(&mut fx.s_emboss_distort, 0.0..=100.0).text("S_EmbossDistort %"));
                ui.add(egui::Slider::new(&mut fx.s_emboss_shiny, 0.0..=100.0).text("S_EmbossShiny %"));
                ui.add(egui::Slider::new(&mut fx.s_zap, 0.0..=100.0).text("S_Zap (lightning) %"));
                ui.add(egui::Slider::new(&mut fx.s_aurora, 0.0..=100.0).text("S_Aurora %"));
                ui.add(egui::Slider::new(&mut fx.s_caustics, 0.0..=100.0).text("S_Caustics %"));
                ui.add(egui::Slider::new(&mut fx.s_clouds, 0.0..=100.0).text("S_Clouds %"));
                ui.add(egui::Slider::new(&mut fx.s_texture_cells, 0.0..=100.0).text("S_TextureCells %"));
                ui.add(egui::Slider::new(&mut fx.s_auto_paint, 0.0..=100.0).text("S_AutoPaint %"));
                ui.add(egui::Slider::new(&mut fx.s_cartoon, 0.0..=100.0).text("S_Cartoon %"));
                ui.add(egui::Slider::new(&mut fx.s_etching, 0.0..=100.0).text("S_Etching %"));
                ui.add(egui::Slider::new(&mut fx.s_scanlines, 0.0..=100.0).text("S_ScanLines %"));
                ui.add(egui::Slider::new(&mut fx.s_film_damage, 0.0..=100.0).text("S_FilmDamage %"));
                ui.add(egui::Slider::new(&mut fx.s_shake, 0.0..=100.0).text("S_Shake %"));
                ui.checkbox(&mut fx.s_psyko_stripes, "S_PsykoStripes");
                ui.checkbox(&mut fx.s_psyko_blobs, "S_PsykoBlobs");
            });
        }

        if show("universe chromatic glow edge glow starglow holomatrix rgb separation heatwave glitch noir retrograde") {
            ui.collapsing("Universe: Glow, Distort & Stylize", |ui| {
                ui.add(egui::Slider::new(&mut fx.u_chromatic_glow, 0.0..=100.0).text("Chromatic Glow %"));
                ui.add(egui::Slider::new(&mut fx.u_edge_glow, 0.0..=100.0).text("Edge Glow %"));
                ui.add(egui::Slider::new(&mut fx.u_starglow, 0.0..=100.0).text("Starglow %"));
                ui.add(egui::Slider::new(&mut fx.u_holomatrix, 0.0..=100.0).text("Holomatrix %"));
                ui.add(egui::Slider::new(&mut fx.u_rgb_sep, 0.0..=100.0).text("RGB Separation %"));
                ui.add(egui::Slider::new(&mut fx.u_heatwave, 0.0..=100.0).text("Heatwave %"));
                ui.add(egui::Slider::new(&mut fx.u_glitch, 0.0..=100.0).text("Universe Glitch %"));
                ui.add(egui::Slider::new(&mut fx.u_noir, 0.0..=100.0).text("Noir %"));
                ui.add(egui::Slider::new(&mut fx.u_retrograde, 0.0..=100.0).text("RetroGrade %"));
            });
        }

        if show("continuum film grain damage vignette glow lightning laser halftone pencil mosaic median falsecolor") {
            ui.collapsing("Continuum: Film, Light & Stylize", |ui| {
                ui.add(egui::Slider::new(&mut fx.bcc_film_grain, 0.0..=100.0).text("BCC Film Grain %"));
                ui.add(egui::Slider::new(&mut fx.bcc_film_damage, 0.0..=100.0).text("BCC Film Damage %"));
                ui.add(egui::Slider::new(&mut fx.bcc_vignette, 0.0..=100.0).text("BCC Vignette %"));
                ui.add(egui::Slider::new(&mut fx.bcc_glow, 0.0..=100.0).text("BCC Glow %"));
                ui.add(egui::Slider::new(&mut fx.bcc_lightning, 0.0..=100.0).text("BCC Lightning %"));
                ui.add(egui::Slider::new(&mut fx.bcc_laser, 0.0..=100.0).text("BCC Laser %"));
                ui.add(egui::Slider::new(&mut fx.bcc_halftone, 0.0..=100.0).text("BCC Halftone %"));
                ui.add(egui::Slider::new(&mut fx.bcc_pencil, 0.0..=100.0).text("BCC Pencil %"));
                ui.add(egui::Slider::new(&mut fx.bcc_mosaic, 0.0..=100.0).text("BCC Mosaic %"));
                ui.add(egui::Slider::new(&mut fx.bcc_median, 0..=3).text("BCC Median passes"));
                ui.add(egui::Slider::new(&mut fx.bcc_falsecolor, 0.0..=100.0).text("BCC Falsecolor %"));
            });
        }

        if show("tv simulator crt curvature scanlines phosphor bloom bleed noise") {
            ui.collapsing("TV simulator (CRT)", |ui| {
                ui.checkbox(&mut fx.crt_enabled, "Enable CRT Simulator");
                ui.add(egui::Slider::new(&mut fx.crt_curvature, 0.0..=1.0).text("Screen curvature"));
                ui.add(egui::Slider::new(&mut fx.crt_scanlines, 0.0..=1.0).text("Scanline strength"));
                ui.add(egui::Slider::new(&mut fx.crt_phosphor, 0.0..=1.0).text("RGB phosphor mask"));
                ui.add(egui::Slider::new(&mut fx.crt_bloom, 0.0..=1.0).text("Bloom / glow"));
                ui.add(egui::Slider::new(&mut fx.crt_bleed, 0.0..=12.0).text("Chroma bleed (px)"));
                ui.add(egui::Slider::new(&mut fx.crt_noise, 0.0..=1.0).text("Static noise"));
            });
        }
    }

    fn render_lib_transitions(&mut self, ui: &mut egui::Ui) {
        let Some(idx) = self.selected_clip_idx else {
            ui.label("Select a clip on V1 first.");
            return;
        };
        let fx = &mut self.clips[idx].fx;
        ui.heading("Transition (Standard & Sapphire S_ Transitions)");
        egui::ComboBox::from_label("Transition type")
            .selected_text(fx.transition.label())
            .show_ui(ui, |ui| {
                for &tr in TransitionType::ALL {
                    ui.selectable_value(&mut fx.transition, tr, tr.label());
                }
            });
        ui.add(egui::Slider::new(&mut fx.transition_dur, 0.1..=3.0).text("Duration (s)"));
    }

    fn render_lib_text(&mut self, ui: &mut egui::Ui) {
        ui.heading("Text overlay");
        ui.text_edit_singleline(&mut self.draft_text.text);
        ui.add(egui::Slider::new(&mut self.draft_text.size_pct, 3.0..=25.0).text("Size (% of height)"));
        ui.add(egui::Slider::new(&mut self.draft_text.x_pct, 0.0..=100.0).text("X %"));
        ui.add(egui::Slider::new(&mut self.draft_text.y_pct, 0.0..=100.0).text("Y %"));
        ui.horizontal(|ui| {
            ui.label("Start (s):");
            ui.add(egui::DragValue::new(&mut self.draft_text.start_sec).speed(0.1));
            ui.label("End (s, 0=end):");
            ui.add(egui::DragValue::new(&mut self.draft_text.end_sec).speed(0.1));
            ui.color_edit_button_srgb(&mut self.draft_text.color);
        });
        if ui.button("+ Add text").clicked() {
            self.push_undo();
            self.text_overlays.push(self.draft_text.clone());
        }
        ui.separator();
        let mut remove_idx = None;
        for (i, tx) in self.text_overlays.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("“{}” ({:.1}s)", tx.text, tx.start_sec));
                if ui.button("✕").clicked() {
                    remove_idx = Some(i);
                }
            });
        }
        if let Some(i) = remove_idx {
            self.push_undo();
            self.text_overlays.remove(i);
        }
    }

    fn render_lib_adjust(&mut self, ui: &mut egui::Ui) {
        let Some(idx) = self.selected_clip_idx else {
            ui.label("Select a clip on V1 first.");
            return;
        };
        let fx = &mut self.clips[idx].fx;
        ui.heading("Color & Grade");
        ui.add(egui::Slider::new(&mut fx.brightness, 0.0..=200.0).text("Brightness (%)"));
        ui.add(egui::Slider::new(&mut fx.contrast, 0.0..=200.0).text("Contrast (%)"));
        ui.add(egui::Slider::new(&mut fx.saturation, 0.0..=300.0).text("Saturation (%)"));
        ui.add(egui::Slider::new(&mut fx.hue_deg, 0.0..=360.0).text("Hue (°)"));
        ui.add(egui::Slider::new(&mut fx.temperature, -100.0..=100.0).text("Temperature (cool ↔ warm)"));
        ui.add(egui::Slider::new(&mut fx.tint, -100.0..=100.0).text("Tint (green ↔ magenta)"));
        egui::ComboBox::from_label("Look")
            .selected_text(fx.look.label())
            .show_ui(ui, |ui| {
                for &lk in VideoLook::ALL {
                    ui.selectable_value(&mut fx.look, lk, lk.label());
                }
            });
        ui.separator();
        ui.heading("Color Wheels & HSL");
        ui.add(egui::Slider::new(&mut fx.wheel_shadows[0], -1.0..=1.0).text("Shadows Tint X"));
        ui.add(egui::Slider::new(&mut fx.wheel_midtones[0], -1.0..=1.0).text("Midtones Tint X"));
        ui.add(egui::Slider::new(&mut fx.wheel_highlights[0], -1.0..=1.0).text("Highlights Tint X"));
        ui.add(egui::Slider::new(&mut fx.hsl_hue_shift, -100.0..=100.0).text("HSL Hue Shift"));
        ui.add(egui::Slider::new(&mut fx.hsl_sat_shift, -100.0..=100.0).text("HSL Saturation"));
        ui.add(egui::Slider::new(&mut fx.hsl_lum_shift, -100.0..=100.0).text("HSL Luminance"));
    }

    fn render_lib_openfx(&mut self, ui: &mut egui::Ui) {
        ui.heading("OpenFX (OFX 1.4 / 1.5) Native Plugin Host");
        ui.label("Loads third-party .ofx.bundle plugins (BorisFX Sapphire OFX, Continuum OFX, RE:Vision, TuttleOFX, etc.) via the native C ABI.");
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if ui.button("🔄 Rescan OFX Paths").clicked() {
                self.ofx_plugins = scan_ofx_directories(&self.ofx_extra_dirs);
                self.set_status(
                    format!("Scanned OpenFX directories: {} plugin(s) found.", self.ofx_plugins.len()),
                    false,
                );
            }
            if ui.button("📂 Load .ofx.bundle...").clicked() {
                if let Some(bundle_dir) = rfd::FileDialog::new().pick_folder() {
                    if bundle_dir.to_string_lossy().ends_with(".ofx.bundle") {
                        let name = bundle_dir
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Custom.ofx.bundle".into());
                        let mac_bin = bundle_dir
                            .join("Contents")
                            .join("MacOS")
                            .join(format!("{}.ofx", name.trim_end_matches(".ofx.bundle")));
                        let loaded = inspect_ofx_binary(&name, &bundle_dir, &mac_bin);
                        let count = loaded.len();
                        self.ofx_plugins.extend(loaded);
                        self.set_status(format!("Loaded {} plugin(s) from {}", count, name), false);
                    } else {
                        self.ofx_extra_dirs.push(bundle_dir);
                        self.ofx_plugins = scan_ofx_directories(&self.ofx_extra_dirs);
                    }
                }
            }
        });

        ui.add_space(6.0);
        ui.collapsing("Search Paths (/Library/OFX/Plugins & OFX_PLUGIN_PATH)", |ui| {
            for p in default_ofx_search_paths() {
                ui.label(format!("• {}", p.display()));
            }
            for p in &self.ofx_extra_dirs {
                ui.label(format!("• {} (custom)", p.display()));
            }
        });

        ui.separator();
        if self.ofx_plugins.is_empty() {
            ui.label("No third-party .ofx.bundle plugins installed in /Library/OFX/Plugins yet.");
            ui.label("Install any macOS arm64 OFX bundle into /Library/OFX/Plugins or click 'Load .ofx.bundle...' above.");
        } else {
            for plug in &mut self.ofx_plugins {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut active = self.active_ofx_identifiers.contains(&plug.identifier);
                        if ui.checkbox(&mut active, &plug.identifier).changed() {
                            if active {
                                self.active_ofx_identifiers.push(plug.identifier.clone());
                            } else {
                                self.active_ofx_identifiers.retain(|id| id != &plug.identifier);
                            }
                        }
                    });
                    ui.small(format!(
                        "Bundle: {} | API: {} v{} | Plugin v{}.{}",
                        plug.bundle_name,
                        plug.api_name,
                        plug.api_version,
                        plug.version_major,
                        plug.version_minor
                    ));
                    ui.small(format!("Status: {}", plug.status));
                });
            }
        }
    }

    fn render_lib_export(&mut self, ui: &mut egui::Ui) {
        ui.heading("Output (Rust + macOS VideoToolbox)");
        ui.add_space(4.0);

        egui::ComboBox::from_label("Container")
            .selected_text(self.video_export.container.label())
            .show_ui(ui, |ui| {
                for &c in ContainerFormat::ALL {
                    ui.selectable_value(&mut self.video_export.container, c, c.label());
                }
            });

        egui::ComboBox::from_label("Video codec")
            .selected_text(self.video_export.video_codec.label())
            .show_ui(ui, |ui| {
                for &vc in VideoCodec::ALL {
                    ui.selectable_value(&mut self.video_export.video_codec, vc, vc.label());
                }
            });

        egui::ComboBox::from_label("Audio codec")
            .selected_text(self.video_export.audio_codec.label())
            .show_ui(ui, |ui| {
                for &ac in VideoAudioCodec::ALL {
                    ui.selectable_value(&mut self.video_export.audio_codec, ac, ac.label());
                }
            });

        egui::ComboBox::from_label("Resolution")
            .selected_text(self.video_export.resolution.label())
            .show_ui(ui, |ui| {
                for &res in ResolutionPreset::ALL {
                    ui.selectable_value(&mut self.video_export.resolution, res, res.label());
                }
            });

        ui.add(egui::Slider::new(&mut self.video_export.fps, 24..=120).text("Frame rate (fps)"));
        ui.checkbox(
            &mut self.video_export.auto_bitrate,
            "Auto bitrate (recommended)",
        );
        if !self.video_export.auto_bitrate {
            ui.add(
                egui::Slider::new(&mut self.video_export.video_bitrate_mbps, 1..=200)
                    .text("Bitrate (Mbps)"),
            );
        }
        egui::ComboBox::from_label("Audio bitrate")
            .selected_text(format!("{} kbps", self.video_export.audio_bitrate_kbps))
            .show_ui(ui, |ui| {
                for br in [128u32, 192, 256, 320, 512] {
                    ui.selectable_value(
                        &mut self.video_export.audio_bitrate_kbps,
                        br,
                        format!("{} kbps", br),
                    );
                }
            });
        ui.checkbox(
            &mut self.video_export.hardware_encode,
            "macOS VideoToolbox hardware acceleration",
        );
        ui.checkbox(
            &mut self.video_export.also_export_pcm_wav,
            "Also export a PCM WAV sidecar of the mix",
        );

        if let Err(err) = validate_video_export(&self.video_export) {
            ui.colored_label(egui::Color32::from_rgb(255, 180, 70), err);
        }

        ui.add_space(10.0);
        let btn_lbl = format!(
            "Export {} · {} · {} fps",
            self.video_export.container.ext().to_uppercase(),
            self.video_export.resolution.label(),
            self.video_export.fps
        );
        let exp_btn = egui::Button::new(
            egui::RichText::new(btn_lbl)
                .strong()
                .color(egui::Color32::from_rgb(6, 32, 29)),
        )
        .fill(egui::Color32::from_rgb(45, 212, 191))
        .min_size(egui::vec2(ui.available_width(), 38.0));

        if ui.add(exp_btn).clicked() {
            self.trigger_video_export();
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Grain — Pure Rust Video & Audio Studio + OpenFX")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 680.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Grain",
        options,
        Box::new(|_cc| Ok(Box::new(GrainStudioApp::default()))),
    )
}
