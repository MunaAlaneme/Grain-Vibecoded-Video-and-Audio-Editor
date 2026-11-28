//! Timeline editing engine.
//!
//! This module owns the *interaction model* of the NLE timeline: tools,
//! snapping, selection, drag state and the normalized item list that the UI
//! hit-tests against. The actual clips still live in `VideoState`; this module
//! deliberately works on a lightweight snapshot so the editor can treat V1,
//! V2..Vn, overlays, music and text as one uniform sequence — the way Premiere
//! Pro / Filmora do.

use egui::{Color32, Pos2, Rect};

/// Smallest editable clip length (seconds).
pub const MIN_CLIP: f32 = 0.05;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TrackKind {
    Video,
    Overlay,
    Audio,
    Text,
}

impl TrackKind {
    pub fn color(self) -> Color32 {
        match self {
            TrackKind::Video => Color32::from_rgb(0x2f, 0x80, 0xed),
            TrackKind::Overlay => Color32::from_rgb(0x8e, 0x5a, 0xd8),
            TrackKind::Audio => Color32::from_rgb(0x21, 0xa0, 0x6a),
            TrackKind::Text => Color32::from_rgb(0xd9, 0x8b, 0x22),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TrackKind::Video => "V",
            TrackKind::Overlay => "FX",
            TrackKind::Audio => "A",
            TrackKind::Text => "T",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimelineTool {
    Select,
    Razor,
    Slip,
    Slide,
    Zoom,
    Hand,
}

impl TimelineTool {
    pub const ALL: &'static [(Self, &'static str, &'static str)] = &[
        (Self::Select, "V", "Selection — move / trim / select clips"),
        (Self::Razor, "C", "Razor — click a clip to split it"),
        (Self::Slide, "U", "Slide — move a clip and close the gap"),
        (Self::Slip, "Y", "Slip — change in/out without moving the clip"),
        (Self::Zoom, "Z", "Zoom — click to zoom in, Alt-click to zoom out"),
        (Self::Hand, "H", "Hand — pan the timeline"),
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Razor => "Razor",
            Self::Slip => "Slip",
            Self::Slide => "Slide",
            Self::Zoom => "Zoom",
            Self::Hand => "Hand",
        }
    }

    pub fn from_key(key: egui::Key) -> Option<Self> {
        match key {
            egui::Key::V => Some(Self::Select),
            egui::Key::C => Some(Self::Razor),
            egui::Key::U => Some(Self::Slide),
            egui::Key::Y => Some(Self::Slip),
            egui::Key::Z => Some(Self::Zoom),
            egui::Key::H => Some(Self::Hand),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragMode {
    Move,
    TrimIn,
    TrimOut,
    Slip,
    Slide,
}

/// A clip as seen by the timeline UI, independent of its backing storage.
#[derive(Clone, Debug)]
pub struct TimelineItem {
    pub uid: u64,
    pub track: usize,
    pub start: f32,
    pub dur: f32,
    pub label: String,
    pub kind: TrackKind,
    pub selected: bool,
    pub locked: bool,
    pub generator: bool,
    pub compound: bool,
}

impl TimelineItem {
    pub fn end(&self) -> f32 {
        self.start + self.dur
    }

    pub fn contains(&self, t: f32) -> bool {
        t >= self.start && t < self.end()
    }
}

#[derive(Clone)]
pub struct TrackHeader {
    pub name: String,
    pub kind: TrackKind,
    pub height: f32,
    pub mute: bool,
    pub solo: bool,
    pub lock: bool,
    pub visible: bool,
    pub target: bool,
}

#[derive(Clone)]
pub struct ItemDrag {
    pub uids: Vec<u64>,
    pub from_track: usize,
    pub pointer_start: Pos2,
    pub starts: Vec<f32>,
    pub target_track: usize,
    pub target_start: f32,
    pub mode: DragMode,
}

/// Interaction state for the timeline, owned by `GrainApp`.
pub struct TimelineUi {
    pub tool: TimelineTool,
    pub snap: bool,
    pub zoom: f32,
    pub selection: Vec<u64>,
    pub drag: Option<ItemDrag>,
    pub marquee: Option<Rect>,
    pub hover: Option<u64>,
    /// Per-track lock / visibility / target-record flags (indexed by lane).
    pub locked: Vec<bool>,
    pub visible: Vec<bool>,
    pub target: Vec<bool>,
    pub audio_track_count: usize,
    pub show_waveforms: bool,
}

impl Default for TimelineUi {
    fn default() -> Self {
        Self {
            tool: TimelineTool::Select,
            snap: true,
            zoom: 60.0,
            selection: Vec::new(),
            drag: None,
            marquee: None,
            hover: None,
            locked: vec![false; 8],
            visible: vec![true; 8],
            target: vec![false; 8],
            audio_track_count: 2,
            show_waveforms: true,
        }
    }
}

impl TimelineUi {
    pub fn ensure_tracks(&mut self, n: usize) {
        if self.locked.len() < n { self.locked.resize(n, false); }
        if self.visible.len() < n { self.visible.resize(n, true); }
        if self.target.len() < n { self.target.resize(n, false); }
    }

    pub fn shrink_tracks(&mut self, n: usize) {
        self.locked.truncate(n);
        self.visible.truncate(n);
        self.target.truncate(n);
    }

    pub fn is_selected(&self, uid: u64) -> bool {
        self.selection.contains(&uid)
    }

    pub fn select(&mut self, uid: u64, additive: bool) {
        if additive {
            if let Some(i) = self.selection.iter().position(|&u| u == uid) {
                self.selection.remove(i);
            } else {
                self.selection.push(uid);
            }
        } else if !self.is_selected(uid) {
            self.selection.clear();
            self.selection.push(uid);
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    pub fn set_selection(&mut self, uids: Vec<u64>) {
        self.selection = uids;
    }

    /// Snap a time to the nearest candidate within `threshold_px / zoom`.
    pub fn snap_time(&self, t: f32, candidates: &[f32], zoom: f32, threshold_px: f32) -> f32 {
        if !self.snap || zoom <= 0.0 { return t; }
        let threshold = threshold_px / zoom;
        let mut best = t;
        let mut best_d = threshold;
        for &c in candidates {
            let d = (t - c).abs();
            if d < best_d {
                best_d = d;
                best = c;
            }
        }
        best.max(0.0)
    }

    pub fn zoom_at(&mut self, factor: f32, anchor_time: f32, anchor_px: f32) {
        let old = self.zoom;
        let new = (old * factor).clamp(8.0, 400.0);
        // Keep the time under the pointer stationary.
        if (new - old).abs() > 0.01 {
            self.zoom = new;
            let shift = (1.0 / old - 1.0 / new) * anchor_time * new;
            let _ = shift; // scroll handled by the egui ScrollArea next frame
            let _ = anchor_px;
        }
    }
}

/// Build the snap candidate set: start/end of every clip, zero, the playhead,
/// and whole-second ruler marks near the visible range.
pub fn snap_candidates(items: &[TimelineItem], playhead: f32, end: f32) -> Vec<f32> {
    let mut v = Vec::with_capacity(items.len() * 2 + 16);
    v.push(0.0);
    v.push(playhead);
    for it in items {
        v.push(it.start);
        v.push(it.end());
    }
    let mut s = 0.0_f32;
    while s <= end {
        v.push(s);
        s += 1.0;
    }
    v
}

/// Hit-test a normalized item list in timeline canvas coordinates.
pub fn hit_item(items: &[TimelineItem], p: Pos2, origin: Pos2, zoom: f32, lane_tops: &[f32], heights: &[f32]) -> Option<(usize, DragMode)> {
    let t = (p.x - origin.x) / zoom;
    let mut lane = None;
    for (i, top) in lane_tops.iter().enumerate() {
        let y0 = origin.y + *top;
        if p.y >= y0 && p.y < y0 + heights.get(i).copied().unwrap_or(30.0) {
            lane = Some(i);
            break;
        }
    }
    let lane = lane?;
    let zoom = zoom.max(1.0);
    // Topmost rendered clip wins; `items` is in painter order.
    for (i, it) in items.iter().enumerate().rev() {
        if it.locked || it.track != lane { continue; }
        let rendered_dur = (it.dur * zoom).max(8.0) / zoom;
        let hit_end = it.start + rendered_dur;
        if t < it.start || t >= hit_end { continue; }
        let edge_px = (12.0 / zoom).min(rendered_dur * 0.33).max(0.0);
        // Half-open span: an exact shared edge belongs to the next clip.
        if t - it.start < edge_px { return Some((i, DragMode::TrimIn)); }
        if hit_end - t <= edge_px { return Some((i, DragMode::TrimOut)); }
        return Some((i, DragMode::Move));
    }
    None
}

/// Timecode formatting shared by the ruler, playhead and transport.
pub fn format_tc(seconds: f32, fps: f32) -> String {
    let fps = fps.max(1.0).round() as u32;
    let total_frames = (seconds.max(0.0) * fps as f32).round() as u64;
    let fr = total_frames % fps as u64;
    let total_sec = total_frames / fps as u64;
    let s = total_sec % 60;
    let m = (total_sec / 60) % 60;
    let h = total_sec / 3600;
    format!("{:02}:{:02}:{:02}:{:02}", h, m, s, fr)
}

/// Apply a ripple delete to a snapshot: remove the selected items and pull all
/// later items left by the removed span (only on the same track, like Premiere's
/// ripple delete when "all tracks" is off).
pub fn ripple_shift(items: &mut [TimelineItem], removed: &[TimelineItem]) {
    for it in items.iter_mut() {
        if removed.iter().any(|r| r.uid == it.uid) { continue; }
        let shift: f32 = removed.iter()
            .filter(|r| r.track == it.track && r.start + r.dur <= it.start + 0.0001)
            .map(|r| r.dur)
            .sum();
        it.start -= shift;
    }
}
