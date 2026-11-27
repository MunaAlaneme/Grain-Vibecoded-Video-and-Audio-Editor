//! Native audio-plugin hosting for Grain NLE.
//!
//! This module hosts the three formats the editor advertises:
//!  * **VST 2.4** through the host side of the `vst` crate (real `AEffect`
//!    dispatcher / `processReplacing` calls).
//!  * **VST 3** through `vst3-host` (real COM factory, component, processor and
//!    parameter interfaces).
//!  * **Apple AudioUnits** through direct AudioToolbox C ABI calls loaded with
//!    `libloading` — no Swift bridge, no Xcode build step, works with both v2
//!    and v3 AUs via Apple's compatibility bridge.
//!
//! The API offered to the GUI is deliberately format-agnostic: scan descriptors,
//! put them in chain slots, change parameters, then render a stereo block.

use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use vst::host::{Host, HostBuffer, PluginInstance as Vst2Instance, PluginLoader};
use vst::plugin::Plugin as Vst2PluginTrait;
use vst3_host::{AudioBuffers, Plugin as Vst3Plugin, Vst3Host};

// ---------------------------------------------------------------------------
// Public, format-agnostic types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PluginFormat {
    Vst2,
    Vst3,
    Au,
}

impl PluginFormat {
    pub const ALL: [PluginFormat; 3] = [PluginFormat::Vst2, PluginFormat::Vst3, PluginFormat::Au];

    pub fn label(self) -> &'static str {
        match self {
            PluginFormat::Vst2 => "VST2",
            PluginFormat::Vst3 => "VST3",
            PluginFormat::Au => "Audio Unit",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PluginDescriptor {
    pub format: PluginFormat,
    /// Stable identity: format prefix + vendor-specific id / four-cc codes.
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub path: PathBuf,
}

impl PluginDescriptor {
    pub fn kind_prefix(&self) -> String {
        self.id.split(':').next().unwrap_or("?").to_string()
    }
}

#[derive(Clone, Debug)]
pub struct PluginParam {
    pub id: u32,
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub value: f32,
}

pub struct PluginSlot {
    pub descriptor: Option<PluginDescriptor>,
    pub enabled: bool,
    pub params: Vec<PluginParam>,
    /// Parameter values kept across a sample-rate rebuild.
    saved_params: Vec<(u32, f32)>,
    loaded: Option<LoadedPlugin>,
    pub last_error: Option<String>,
}

impl Default for PluginSlot {
    fn default() -> Self {
        Self {
            descriptor: None,
            enabled: true,
            params: Vec::new(),
            saved_params: Vec::new(),
            loaded: None,
            last_error: None,
        }
    }
}

impl PluginSlot {
    pub fn status(&self) -> String {
        if self.descriptor.is_none() {
            "empty".into()
        } else if let Some(e) = &self.last_error {
            e.clone()
        } else if self.loaded.is_some() {
            "loaded".into()
        } else {
            "not loaded".into()
        }
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
    }
}

// ---------------------------------------------------------------------------
// VST 2 host
// ---------------------------------------------------------------------------

#[derive(Default)]
struct GrainVstHost {
    sample_rate: f64,
    block_size: i32,
}

impl Host for GrainVstHost {
    fn get_info(&self) -> (isize, String, String) {
        (2400, "Grain".into(), "Grain NLE".into())
    }

    fn get_time_info(&self, mask: i32) -> Option<vst::api::TimeInfo> {
        let mut info = vst::api::TimeInfo::default();
        info.sample_rate = self.sample_rate;
        info.tempo = 120.0;
        info.time_sig_numerator = 4;
        info.time_sig_denominator = 4;
        info.flags = mask;
        Some(info)
    }

    fn get_block_size(&self) -> isize {
        self.block_size as isize
    }
}

struct Vst2Loaded {
    instance: Vst2Instance,
    /// Keep the host callback alive: the plugin stores a raw pointer to it in
    /// its `AEffect` reserved fields.
    _host: Arc<Mutex<GrainVstHost>>,
    inputs: usize,
    outputs: usize,
}

fn vst2_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(env) = std::env::var("VST_PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for p in env.split(sep).filter(|p| !p.trim().is_empty()) {
            paths.push(PathBuf::from(p.trim()));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(&home).join("Library/Audio/Plug-Ins/VST"));
        paths.push(PathBuf::from(&home).join(".vst"));
    }
    if cfg!(target_os = "macos") {
        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/VST"));
        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/VST3")); // VST2 binaries sometimes live here
    } else if cfg!(windows) {
        paths.push(PathBuf::from(r"C:\Program Files\VSTPlugins"));
        paths.push(PathBuf::from(r"C:\Program Files (x86)\VSTPlugins"));
        paths.push(PathBuf::from(r"C:\Program Files\Common Files\VST2"));
    } else {
        paths.push(PathBuf::from("/usr/lib/vst"));
        paths.push(PathBuf::from("/usr/local/lib/vst"));
        paths.push(PathBuf::from("/usr/lib/lv2"));
    }
    paths
}

/// Recursively collect candidate VST2 shared libraries (max depth 4).
fn vst2_binaries(dir: &Path, depth: u32) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let is_bundle = p.extension().and_then(|s| s.to_str()) == Some("vst");
            if is_bundle {
                let macos = p.join("Contents/MacOS");
                if let Ok(list) = std::fs::read_dir(&macos) {
                    for f in list.flatten() {
                        let f = f.path();
                        if f.is_file() {
                            out.push(f);
                        }
                    }
                }
                continue;
            }
            if depth < 4 {
                out.extend(vst2_binaries(&p, depth + 1));
            }
        } else {
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            if matches!(ext.as_str(), "dll" | "so" | "dylib") {
                // Avoid picking up VST3/CLAP binaries in mixed folders.
                let s = p.to_string_lossy().to_lowercase();
                if !s.ends_with(".vst3/contents/macos") && !s.contains(".clap") {
                    out.push(p);
                }
            }
        }
    }
    out
}

pub fn scan_vst2(extra: &[PathBuf]) -> Vec<PluginDescriptor> {
    let mut paths = vst2_search_paths();
    paths.extend(extra.iter().cloned());
    let mut out: Vec<PluginDescriptor> = Vec::new();
    for root in paths {
        for binary in vst2_binaries(&root, 0) {
            let host = Arc::new(Mutex::new(GrainVstHost::default()));
            let loaded = (|| -> Result<PluginDescriptor, String> {
                let mut loader = PluginLoader::load(&binary, host.clone())
                    .map_err(|e| format!("{e}"))?;
                let instance = loader.instance().map_err(|e| format!("{e}"))?;
                let info = instance.get_info();
                Ok(PluginDescriptor {
                    format: PluginFormat::Vst2,
                    id: format!("vst2:{:08x}", info.unique_id as u32),
                    name: if info.name.is_empty() {
                        binary.file_name().unwrap_or_default().to_string_lossy().into_owned()
                    } else {
                        info.name
                    },
                    vendor: info.vendor,
                    version: format!("{}", info.version),
                    path: binary.clone(),
                })
            })();
            if let Ok(desc) = loaded {
                if !out.iter().any(|d| d.id == desc.id && d.path == desc.path) {
                    out.push(desc);
                }
            }
        }
    }
    out
}

fn load_vst2(desc: &PluginDescriptor, sample_rate: u32) -> Result<Vst2Loaded, String> {
    let host = Arc::new(Mutex::new(GrainVstHost { sample_rate: sample_rate as f64, block_size: 512 }));
    let mut loader = PluginLoader::load(&desc.path, host.clone()).map_err(|e| format!("{e}"))?;
    let mut instance = loader.instance().map_err(|e| format!("{e}"))?;
    instance.init();
    instance.set_sample_rate(sample_rate as f32);
    instance.set_block_size(512);
    instance.resume();
    let info = instance.get_info();
    Ok(Vst2Loaded {
        instance,
        _host: host,
        inputs: (info.inputs.max(0) as usize).min(8),
        outputs: (info.outputs.max(0) as usize).min(8),
    })
}

impl Vst2Loaded {
    fn params(&mut self) -> Vec<PluginParam> {
        let count = self.instance.get_info().parameters.max(0) as usize;
        let holder = self.instance.get_parameter_object();
        (0..count)
            .map(|i| {
                let index = i as i32;
                PluginParam {
                    id: index as u32,
                    name: {
                        let n = holder.get_parameter_name(index);
                        if n.is_empty() { format!("Param {}", i + 1) } else { n }
                    },
                    min: 0.0,
                    max: 1.0,
                    default: 0.0,
                    value: holder.get_parameter(index),
                }
            })
            .take(64)
            .collect()
    }

    fn set_param(&mut self, id: u32, value: f32) {
        self.instance.get_parameter_object().set_parameter(id as i32, value.clamp(0.0, 1.0));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], sample_rate: u32) {
        let block = 512usize;
        let mut buffer = HostBuffer::<f32>::new(self.inputs, self.outputs);
        let mut pos = 0usize;
        while pos < left.len() {
            let n = block.min(left.len() - pos);
            let in_l = left[pos..pos + n].to_vec();
            let in_r = right[pos..pos + n].to_vec();
            let mut out_l = vec![0.0f32; n];
            let mut out_r = vec![0.0f32; n];
            {
                let inputs: Vec<&[f32]> = if self.inputs >= 2 {
                    vec![&in_l, &in_r]
                } else if self.inputs == 1 {
                    vec![&in_l]
                } else {
                    Vec::new()
                };
                let mut outputs: Vec<&mut [f32]> = if self.outputs >= 2 {
                    vec![&mut out_l, &mut out_r]
                } else {
                    vec![&mut out_l]
                };
                let mut audio = buffer.bind(&inputs, &mut outputs);
                self.instance.process(&mut audio);
            }
            if self.outputs >= 2 {
                left[pos..pos + n].copy_from_slice(&out_l);
                right[pos..pos + n].copy_from_slice(&out_r);
            } else if self.outputs == 1 {
                left[pos..pos + n].copy_from_slice(&out_l);
                right[pos..pos + n].copy_from_slice(&out_l);
            }
            pos += n;
            let _ = sample_rate;
        }
    }
}

// ---------------------------------------------------------------------------
// VST 3 host
// ---------------------------------------------------------------------------

fn vst3_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(env) = std::env::var("VST3_PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for p in env.split(sep).filter(|p| !p.trim().is_empty()) {
            paths.push(PathBuf::from(p.trim()));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(home).join("Library/Audio/Plug-Ins/VST3"));
    }
    if cfg!(target_os = "macos") {
        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/VST3"));
    } else if cfg!(windows) {
        paths.push(PathBuf::from(r"C:\Program Files\Common Files\VST3"));
    } else {
        paths.push(PathBuf::from("/usr/lib/vst3"));
        paths.push(PathBuf::from("/usr/local/lib/vst3"));
    }
    paths
}

pub fn make_vst3_host(sample_rate: u32, extra: &[PathBuf]) -> Result<Vst3Host, String> {
    let mut builder = Vst3Host::builder()
        .sample_rate(sample_rate as f64)
        .block_size(512)
        .scan_default_paths();
    for p in extra {
        builder = builder.add_scan_path(p.clone());
    }
    builder.build().map_err(|e| e.to_string())
}

/// Filesystem-only scan: does not instantiate plugins, so a broken plugin
/// cannot take down the editor while browsing.
pub fn scan_vst3(host: &Vst3Host) -> Vec<PluginDescriptor> {
    host.scan_plugin_paths()
        .into_iter()
        .map(|path| {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("VST3 plugin")
                .trim_end_matches(".vst3")
                .to_string();
            PluginDescriptor {
                format: PluginFormat::Vst3,
                id: format!("vst3:{}", path.to_string_lossy()),
                name,
                vendor: String::new(),
                version: String::new(),
                path,
            }
        })
        .collect()
}

pub fn load_vst3(host: &mut Vst3Host, desc: &PluginDescriptor) -> Result<Vst3Plugin, String> {
    let mut plugin = host.load_plugin(&desc.path).map_err(|e| e.to_string())?;
    if let Err(e) = plugin.start_processing() {
        return Err(format!("startProcessing: {e}"));
    }
    Ok(plugin)
}

pub fn vst3_params(plugin: &Vst3Plugin) -> Vec<PluginParam> {
    plugin
        .get_parameters()
        .unwrap_or_default()
        .into_iter()
        .take(64)
        .map(|p| PluginParam {
            id: p.id,
            name: p.name,
            min: p.min as f32,
            max: p.max as f32,
            default: p.default as f32,
            value: p.value as f32,
        })
        .collect()
}

pub fn vst3_process(
    plugin: &mut Vst3Plugin,
    left: &mut [f32],
    right: &mut [f32],
    sample_rate: u32,
) -> Result<(), String> {
    let block = 512usize;
    let mut pos = 0usize;
    while pos < left.len() {
        let n = block.min(left.len() - pos);
        let mut buffers = AudioBuffers::new(2, 2, n, sample_rate as f64);
        buffers.inputs[0][..n].copy_from_slice(&left[pos..pos + n]);
        buffers.inputs[1][..n].copy_from_slice(&right[pos..pos + n]);
        plugin.process_audio(&mut buffers).map_err(|e| e.to_string())?;
        for i in 0..n {
            left[pos + i] = buffers.outputs.get(0).and_then(|c| c.get(i)).copied().unwrap_or(left[pos + i]);
            right[pos + i] = buffers.outputs.get(1).and_then(|c| c.get(i)).copied().unwrap_or(right[pos + i]);
        }
        pos += n;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Apple AudioUnit host — direct AudioToolbox C ABI, loaded at runtime
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod au {
    use super::*;
    use libloading::Library;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct AuDesc {
        pub component_type: u32,
        pub component_subtype: u32,
        pub component_manufacturer: u32,
        pub component_flags: u32,
        pub component_flags_mask: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct AuTimeStamp {
        pub sample_time: f64,
        pub host_time: u64,
        pub rate_scalar: f64,
        pub word_clock_time: u64,
        smpte: [u8; 24],
        pub flags: u32,
        pub reserved: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct AuBuffer {
        pub number_channels: u32,
        pub data_byte_size: u32,
        pub data: *mut c_void,
    }

    #[repr(C)]
    pub struct AuBufferList2 {
        pub number_buffers: u32,
        pub buffers: [AuBuffer; 2],
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct AuAsbd {
        pub sample_rate: f64,
        pub format_id: u32,
        pub format_flags: u32,
        pub bytes_per_packet: u32,
        pub frames_per_packet: u32,
        pub bytes_per_frame: u32,
        pub channels_per_frame: u32,
        pub bits_per_channel: u32,
        pub reserved: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct AuParamInfo {
        pub name: [c_char; 52],
        pub unit_name: *mut c_void,
        pub clump_id: u32,
        pub cf_name_string: *mut c_void,
        pub unit: u32,
        pub min: f32,
        pub max: f32,
        pub default: f32,
        pub flags: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct AuRenderCallback {
        pub proc: Option<
            unsafe extern "C" fn(
                *mut c_void,
                *mut u32,
                *const AuTimeStamp,
                u32,
                u32,
                *mut AuBufferList2,
            ) -> i32,
        >,
        pub ref_con: *mut c_void,
    }

    type FindNext = unsafe extern "C" fn(*mut c_void, *const AuDesc) -> *mut c_void;
    type GetDesc = unsafe extern "C" fn(*mut c_void, *mut AuDesc) -> i32;
    type CopyName = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32;
    type InstanceNew = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32;
    type InstanceDispose = unsafe extern "C" fn(*mut c_void) -> i32;
    type UnitInit = unsafe extern "C" fn(*mut c_void) -> i32;
    type UnitUninit = unsafe extern "C" fn(*mut c_void) -> i32;
    type SetProperty =
        unsafe extern "C" fn(*mut c_void, u32, u32, u32, *const c_void, u32) -> i32;
    type GetProperty =
        unsafe extern "C" fn(*mut c_void, u32, u32, u32, *mut c_void, *mut u32) -> i32;
    type SetParam =
        unsafe extern "C" fn(*mut c_void, u32, u32, u32, f32, u32) -> i32;
    type GetParam =
        unsafe extern "C" fn(*mut c_void, u32, u32, u32, *mut f32) -> i32;
    type Render = unsafe extern "C" fn(
        *mut c_void,
        *mut u32,
        *const AuTimeStamp,
        u32,
        u32,
        *mut AuBufferList2,
    ) -> i32;
    type CfStringGetCString =
        unsafe extern "C" fn(*const c_void, *mut c_char, isize, u32) -> u8;
    type CfRelease = unsafe extern "C" fn(*const c_void);

    /// All AudioToolbox entry points used by Grain, resolved once and kept
    /// alive for as long as any AudioUnit instance exists.
    pub struct AuApi {
        _toolbox: Library,
        _core_foundation: Library,
        find_next: FindNext,
        get_desc: GetDesc,
        copy_name: CopyName,
        instance_new: InstanceNew,
        instance_dispose: InstanceDispose,
        init: UnitInit,
        uninit: UnitUninit,
        set_property: SetProperty,
        get_property: GetProperty,
        set_param: SetParam,
        get_param: GetParam,
        render: Render,
        cf_get_cstring: CfStringGetCString,
        cf_release: CfRelease,
    }

    impl AuApi {
        pub fn open() -> Result<Arc<AuApi>, String> {
            let candidates = [
                "/System/Library/Frameworks/AudioToolbox.framework/AudioToolbox",
                "/System/Library/Frameworks/AudioUnit.framework/AudioUnit",
            ];
            let mut toolbox = None;
            let mut last = String::new();
            for p in candidates {
                match unsafe { Library::new(p) } {
                    Ok(l) => { toolbox = Some(l); break; }
                    Err(e) => last = e.to_string(),
                }
            }
            let toolbox = toolbox.ok_or_else(|| format!("AudioToolbox unavailable: {last}"))?;
            let cf = unsafe { Library::new("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation") }
                .map_err(|e| format!("CoreFoundation unavailable: {e}"))?;

            macro_rules! sym {
                ($lib:expr, $name:expr, $ty:ty) => {
                    unsafe {
                        let s: libloading::Symbol<$ty> = $lib.get($name)
                            .map_err(|e| format!("missing {}: {e}", std::str::from_utf8($name).unwrap_or("?")))?;
                        *s
                    }
                };
            }
            let api = AuApi {
                find_next: sym!(toolbox, b"AudioComponentFindNext", FindNext),
                get_desc: sym!(toolbox, b"AudioComponentGetDescription", GetDesc),
                copy_name: sym!(toolbox, b"AudioComponentCopyName", CopyName),
                instance_new: sym!(toolbox, b"AudioComponentInstanceNew", InstanceNew),
                instance_dispose: sym!(toolbox, b"AudioComponentInstanceDispose", InstanceDispose),
                init: sym!(toolbox, b"AudioUnitInitialize", UnitInit),
                uninit: sym!(toolbox, b"AudioUnitUninitialize", UnitUninit),
                set_property: sym!(toolbox, b"AudioUnitSetProperty", SetProperty),
                get_property: sym!(toolbox, b"AudioUnitGetProperty", GetProperty),
                set_param: sym!(toolbox, b"AudioUnitSetParameter", SetParam),
                get_param: sym!(toolbox, b"AudioUnitGetParameter", GetParam),
                render: sym!(toolbox, b"AudioUnitRender", Render),
                cf_get_cstring: sym!(cf, b"CFStringGetCString", CfStringGetCString),
                cf_release: sym!(cf, b"CFRelease", CfRelease),
                _toolbox: toolbox,
                _core_foundation: cf,
            };
            Ok(Arc::new(api))
        }

        fn cf_string(&self, ptr: *mut c_void) -> String {
            if ptr.is_null() {
                return String::new();
            }
            let mut buf = [0i8; 256];
            // kCFStringEncodingUTF8
            let ok = unsafe { (self.cf_get_cstring)(ptr, buf.as_mut_ptr(), buf.len() as isize, 0x0800_0100) };
            let s = if ok != 0 {
                unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
            } else {
                String::new()
            };
            unsafe { (self.cf_release)(ptr) };
            s
        }
    }

    const TYPE_EFFECT: u32 = 0x6175_6678; // 'aufx'
    const TYPE_MUSIC_EFFECT: u32 = 0x6175_6d66; // 'aufm'
    const TYPE_GENERATOR: u32 = 0x6175_676e; // 'augn'
    const TYPE_MIXER: u32 = 0x6175_6d78; // 'aumx'
    const TYPE_PANNER: u32 = 0x6175_706e; // 'aupn'

    fn fourcc(v: u32) -> String {
        let b = v.to_be_bytes();
        b.iter().map(|&c| if (32..=126).contains(&c) { c as char } else { '·' }).collect()
    }

    pub fn scan(api: &AuApi) -> Vec<PluginDescriptor> {
        let want = |d: &AuDesc| matches!(d.component_type, TYPE_EFFECT | TYPE_MUSIC_EFFECT | TYPE_GENERATOR | TYPE_MIXER | TYPE_PANNER);
        let mut out = Vec::new();
        let any = AuDesc::default();
        let mut prev: *mut c_void = std::ptr::null_mut();
        // Bound the walk: component registries are finite but we never trust
        // the OS to signal termination correctly.
        for _ in 0..4096 {
            let comp = unsafe { (api.find_next)(prev, &any) };
            if comp.is_null() {
                break;
            }
            let mut d = AuDesc::default();
            if unsafe { (api.get_desc)(comp, &mut d) } == 0 && want(&d) {
                let mut name_ptr: *mut c_void = std::ptr::null_mut();
                let name = if unsafe { (api.copy_name)(comp, &mut name_ptr) } == 0 {
                    api.cf_string(name_ptr)
                } else {
                    String::new()
                };
                let id = format!("au:{:08x}:{:08x}:{:08x}", d.component_type, d.component_subtype, d.component_manufacturer);
                let name = if name.is_empty() { format!("{} ({})", fourcc(d.component_subtype), fourcc(d.component_type)) } else { name };
                if !out.iter().any(|p: &PluginDescriptor| p.id == id) {
                    out.push(PluginDescriptor {
                        format: PluginFormat::Au,
                        id,
                        name,
                        vendor: fourcc(d.component_manufacturer),
                        version: String::new(),
                        path: PathBuf::from(format!("AudioUnit:{}/{}", fourcc(d.component_type), fourcc(d.component_subtype))),
                    });
                }
            }
            prev = comp;
        }
        out
    }

    #[derive(Default)]
    struct InputState {
        left: *const f32,
        right: *const f32,
        left_len: usize,
        right_len: usize,
    }

    unsafe extern "C" fn input_proc(
        ref_con: *mut c_void,
        _flags: *mut u32,
        _stamp: *const AuTimeStamp,
        _bus: u32,
        frames: u32,
        io_data: *mut AuBufferList2,
    ) -> i32 {
        if ref_con.is_null() || io_data.is_null() {
            return -1;
        }
        let state = &*(ref_con as *const InputState);
        let list = &mut *io_data;
        let n = (frames as usize).min(8192);
        for i in 0..(list.number_buffers as usize).min(2) {
            let buf = &mut list.buffers[i];
            if buf.data.is_null() {
                continue;
            }
            let src = if i == 0 { state.left } else { state.right };
            let len = if i == 0 { state.left_len } else { state.right_len };
            let count = n.min(len).min(buf.data_byte_size as usize / 4);
            if !src.is_null() && count > 0 {
                std::ptr::copy_nonoverlapping(src, buf.data as *mut f32, count);
            } else {
                std::ptr::write_bytes(buf.data, 0, count * 4);
            }
        }
        0
    }

    pub struct AuLoaded {
        api: Arc<AuApi>,
        unit: *mut c_void,
        /// Dedicated input buffers. `AudioUnitRender` writes into the output
        /// buffers, so keeping the input source separate avoids in-place
        /// aliasing between the input callback and the render result.
        input_left: Vec<f32>,
        input_right: Vec<f32>,
        _input: Box<InputState>,
    }

    impl AuLoaded {
        pub fn load(desc: &PluginDescriptor, api: Arc<AuApi>, sample_rate: u32) -> Result<Self, String> {
            let wanted: Vec<u32> = desc.id.split(':').skip(1).filter_map(|s| u32::from_str_radix(s, 16).ok()).collect();
            if wanted.len() < 3 {
                return Err("invalid AudioUnit id".into());
            }
            let target = AuDesc {
                component_type: wanted[0],
                component_subtype: wanted[1],
                component_manufacturer: wanted[2],
                component_flags: 0,
                component_flags_mask: 0,
            };
            let any = AuDesc::default();
            let mut component: *mut c_void = std::ptr::null_mut();
            let mut prev: *mut c_void = std::ptr::null_mut();
            for _ in 0..4096 {
                let c = unsafe { (api.find_next)(prev, &any) };
                if c.is_null() { break; }
                let mut d = AuDesc::default();
                if unsafe { (api.get_desc)(c, &mut d) } == 0
                    && d.component_type == target.component_type
                    && d.component_subtype == target.component_subtype
                    && d.component_manufacturer == target.component_manufacturer
                {
                    component = c;
                    break;
                }
                prev = c;
            }
            if component.is_null() {
                return Err("AudioUnit component not found".into());
            }
            let mut unit: *mut c_void = std::ptr::null_mut();
            let st = unsafe { (api.instance_new)(component, &mut unit) };
            if st != 0 || unit.is_null() {
                return Err(format!("AudioComponentInstanceNew error {st}"));
            }

            // Stereo, non-interleaved float32 at the project sample rate.
            let asbd = AuAsbd {
                sample_rate: sample_rate as f64,
                format_id: 0x6c70_636d, // 'lpcm'
                format_flags: 0x01 | 0x10, // float | non-interleaved
                bytes_per_packet: 4,
                frames_per_packet: 1,
                bytes_per_frame: 4,
                channels_per_frame: 2,
                bits_per_channel: 32,
                reserved: 0,
            };
            let size = std::mem::size_of::<AuAsbd>() as u32;
            // Instruments have no input bus; ignore the failure there.
            unsafe {
                (api.set_property)(unit, 8, 1, 0, &asbd as *const _ as *const c_void, size);
                (api.set_property)(unit, 8, 2, 0, &asbd as *const _ as *const c_void, size);
                let max_frames: u32 = 512;
                (api.set_property)(unit, 14, 0, 0, &max_frames as *const _ as *const c_void, 4);
            }

            let mut input = Box::new(InputState::default());
            let cb = AuRenderCallback { proc: Some(input_proc), ref_con: &mut *input as *mut _ as *mut c_void };
            unsafe {
                // kAudioUnitProperty_SetRenderCallback on input bus 0.
                (api.set_property)(unit, 23, 1, 0, &cb as *const _ as *const c_void, std::mem::size_of::<AuRenderCallback>() as u32);
                let st = (api.init)(unit);
                if st != 0 {
                    (api.instance_dispose)(unit);
                    return Err(format!("AudioUnitInitialize error {st}"));
                }
            }
            Ok(Self { api, unit, input_left: Vec::new(), input_right: Vec::new(), _input: input })
        }

        pub fn params(&self) -> Vec<PluginParam> {
            let mut ids: Vec<u32> = Vec::new();
            let mut size = 0u32;
            let st = unsafe {
                (self.api.get_property)(self.unit, 3, 0, 0, std::ptr::null_mut(), &mut size)
            };
            if st != 0 || size < 4 {
                return Vec::new();
            }
            ids.resize((size as usize) / 4, 0);
            size = (ids.len() * 4) as u32;
            if unsafe { (self.api.get_property)(self.unit, 3, 0, 0, ids.as_mut_ptr() as *mut c_void, &mut size) } != 0 {
                return Vec::new();
            }
            ids.into_iter()
                .take(64)
                .filter_map(|id| {
                    let mut info: AuParamInfo = unsafe { std::mem::zeroed() };
                    let mut info_size = std::mem::size_of::<AuParamInfo>() as u32;
                    if unsafe { (self.api.get_property)(self.unit, 4, 0, id, &mut info as *mut _ as *mut c_void, &mut info_size) } != 0 {
                        return None;
                    }
                    let name = if !info.cf_name_string.is_null() {
                        self.api.cf_string(info.cf_name_string)
                    } else {
                        unsafe { CStr::from_ptr(info.name.as_ptr()) }.to_string_lossy().into_owned()
                    };
                    let mut value = 0.0f32;
                    let _ = unsafe { (self.api.get_param)(self.unit, id, 0, 0, &mut value) };
                    Some(PluginParam {
                        id,
                        name: if name.is_empty() { format!("Param {}", id) } else { name },
                        min: info.min,
                        max: info.max,
                        default: info.default,
                        value,
                    })
                })
                .collect()
        }

        pub fn set_param(&self, id: u32, value: f32) {
            unsafe { (self.api.set_param)(self.unit, id, 0, 0, value, 0) };
        }

        pub fn process(&mut self, left: &mut [f32], right: &mut [f32], sample_rate: u32) -> Result<(), String> {
            let block = 512usize;
            let mut pos = 0usize;
            while pos < left.len() {
                let n = block.min(left.len() - pos);
                self.input_left.resize(n, 0.0);
                self.input_right.resize(n, 0.0);
                self.input_left.copy_from_slice(&left[pos..pos + n]);
                self.input_right.copy_from_slice(&right[pos..pos + n]);
                self._input.left = self.input_left.as_ptr();
                self._input.right = self.input_right.as_ptr();
                self._input.left_len = n;
                self._input.right_len = n;

                let b = |data: *mut c_void| AuBuffer {
                    number_channels: 1,
                    data_byte_size: (n * 4) as u32,
                    data,
                };
                let mut list = AuBufferList2 {
                    number_buffers: 2,
                    buffers: [b(left[pos..pos + n].as_mut_ptr() as *mut c_void), b(right[pos..pos + n].as_mut_ptr() as *mut c_void)],
                };
                let mut stamp = AuTimeStamp::default();
                stamp.sample_time = pos as f64 / sample_rate.max(1) as f64;
                stamp.flags = 1; // kAudioTimeStampSampleTimeValid
                let mut flags = 0u32;
                let st = unsafe {
                    (self.api.render)(self.unit, &mut flags, &stamp, 0, n as u32, &mut list)
                };
                if st != 0 {
                    return Err(format!("AudioUnitRender error {st}"));
                }
                pos += n;
            }
            Ok(())
        }
    }

    impl Drop for AuLoaded {
        fn drop(&mut self) {
            unsafe {
                (self.api.uninit)(self.unit);
                (self.api.instance_dispose)(self.unit);
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod au {
    use super::*;
    pub struct AuLoaded;
    impl AuLoaded {
        pub fn load(_d: &PluginDescriptor, _sr: u32) -> Result<Self, String> {
            Err("Audio Units are only available on macOS".into())
        }
    }
    impl AuLoaded {
        pub fn params(&self) -> Vec<PluginParam> { Vec::new() }
        pub fn set_param(&self, _id: u32, _value: f32) {}
        pub fn process(&mut self, _l: &mut [f32], _r: &mut [f32], _sr: u32) -> Result<(), String> {
            Err("Audio Units are only available on macOS".into())
        }
    }
}

// ---------------------------------------------------------------------------
// Loaded plugin enum + chain
// ---------------------------------------------------------------------------

enum LoadedPlugin {
    Vst2(Vst2Loaded),
    Vst3(Vst3Plugin),
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Au(au::AuLoaded),
}

pub struct PluginChain {
    pub slots: [PluginSlot; 8],
    pub descriptors: Vec<PluginDescriptor>,
    pub scan_paths: Vec<PathBuf>,
    pub last_scan: String,
    sample_rate: u32,
    vst3: Option<Vst3Host>,
    au_api: Option<Arc<au::AuApi>>,
}

impl Default for PluginChain {
    fn default() -> Self {
        Self::new(44_100)
    }
}

impl PluginChain {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            slots: std::array::from_fn(|_| PluginSlot::default()),
            descriptors: Vec::new(),
            scan_paths: Vec::new(),
            last_scan: String::new(),
            sample_rate,
            vst3: None,
            au_api: None,
        }
    }

    pub fn set_sample_rate(&mut self, rate: u32) {
        if rate != self.sample_rate {
            self.sample_rate = rate;
            // Existing instances must be rebuilt at the new rate. Preserve
            // user edits so automation survives the rebuild.
            for slot in &mut self.slots {
                slot.saved_params = slot.params.iter().map(|p| (p.id, p.value)).collect();
                slot.loaded = None;
                slot.params.clear();
                slot.last_error = Some("reload needed: sample rate changed".into());
            }
        }
    }

    pub fn scan(&mut self, formats: &[PluginFormat]) -> String {
        let mut notes = Vec::new();
        if formats.contains(&PluginFormat::Vst2) {
            let found = scan_vst2(&self.scan_paths);
            notes.push(format!("VST2: {}", found.len()));
            self.merge(found);
        }
        if formats.contains(&PluginFormat::Vst3) {
            match make_vst3_host(self.sample_rate, &self.scan_paths) {
                Ok(host) => {
                    let found = scan_vst3(&host);
                    notes.push(format!("VST3: {}", found.len()));
                    self.merge(found);
                    self.vst3 = Some(host);
                }
                Err(e) => notes.push(format!("VST3: {e}")),
            }
        }
        if formats.contains(&PluginFormat::Au) {
            #[cfg(target_os = "macos")]
            match au::AuApi::open() {
                Ok(api) => {
                    let found = au::scan(&api);
                    notes.push(format!("AU: {}", found.len()));
                    self.merge(found);
                    self.au_api = Some(api);
                }
                Err(e) => notes.push(format!("AU: {e}")),
            }
            #[cfg(not(target_os = "macos"))]
            notes.push("AU: macOS only".into());
        }
        self.last_scan = notes.join(" · ");
        self.last_scan.clone()
    }

    fn merge(&mut self, found: Vec<PluginDescriptor>) {
        for d in found {
            if !self.descriptors.iter().any(|x| x.id == d.id && x.path == d.path) {
                self.descriptors.push(d);
            }
        }
    }

    pub fn add(&mut self, slot_index: usize, desc: &PluginDescriptor) -> Result<(), String> {
        let slot = self.slots.get_mut(slot_index).ok_or("slot out of range")?;
        slot.descriptor = Some(desc.clone());
        slot.enabled = true;
        slot.loaded = None;
        slot.params.clear();
        slot.saved_params.clear();
        slot.last_error = None;
        self.ensure_loaded(slot_index)
    }

    pub fn remove(&mut self, slot_index: usize) {
        if let Some(slot) = self.slots.get_mut(slot_index) {
            slot.descriptor = None;
            slot.loaded = None;
            slot.params.clear();
            slot.saved_params.clear();
            slot.last_error = None;
        }
    }

    pub fn set_enabled(&mut self, slot_index: usize, enabled: bool) {
        if let Some(slot) = self.slots.get_mut(slot_index) {
            slot.enabled = enabled;
        }
    }

    pub fn set_param(&mut self, slot_index: usize, id: u32, value: f32) {
        let Some(slot) = self.slots.get_mut(slot_index) else { return };
        if let Some(p) = slot.params.iter_mut().find(|p| p.id == id) {
            p.value = value.clamp(p.min, p.max);
        }
        if let Some(saved) = slot.saved_params.iter_mut().find(|(pid, _)| *pid == id) {
            saved.1 = value;
        }
        if let Some(loaded) = slot.loaded.as_mut() {
            match loaded {
                LoadedPlugin::Vst2(p) => p.set_param(id, value),
                LoadedPlugin::Vst3(p) => {
                    let _ = p.set_parameter(id, value as f64);
                }
                LoadedPlugin::Au(p) => p.set_param(id, value),
            }
        }
    }

    fn ensure_loaded(&mut self, slot_index: usize) -> Result<(), String> {
        let desc = match self.slots.get(slot_index).and_then(|s| s.descriptor.clone()) {
            Some(d) => d,
            None => return Ok(()),
        };
        if self.slots[slot_index].loaded.is_some() {
            return Ok(());
        }
        let mut loaded = match desc.format {
            PluginFormat::Vst2 => LoadedPlugin::Vst2(load_vst2(&desc, self.sample_rate)?),
            PluginFormat::Vst3 => {
                if self.vst3.is_none() {
                    self.vst3 = Some(make_vst3_host(self.sample_rate, &self.scan_paths)?);
                }
                let host = self.vst3.as_mut().ok_or("VST3 host unavailable")?;
                LoadedPlugin::Vst3(load_vst3(host, &desc)?)
            }
            PluginFormat::Au => {
                #[cfg(target_os = "macos")]
                {
                    let api = match &self.au_api {
                        Some(a) => a.clone(),
                        None => {
                            let a = au::AuApi::open()?;
                            self.au_api = Some(a.clone());
                            a
                        }
                    };
                    LoadedPlugin::Au(au::AuLoaded::load(&desc, api, self.sample_rate)?)
                }
                #[cfg(not(target_os = "macos"))]
                return Err("Audio Units are only available on macOS".into());
            }
        };
        let params = match &mut loaded {
            LoadedPlugin::Vst2(p) => p.params(),
            LoadedPlugin::Vst3(p) => vst3_params(p),
            LoadedPlugin::Au(p) => p.params(),
        };
        let slot = &mut self.slots[slot_index];
        slot.params = params;
        slot.loaded = Some(loaded);
        slot.last_error = None;
        // Push saved parameter values into the freshly created instance.
        let values: Vec<(u32, f32)> = slot.params.iter().map(|p| {
            let saved = slot.saved_params.iter().find(|(id, _)| *id == p.id).map(|(_, v)| *v);
            (p.id, saved.unwrap_or(p.value))
        }).collect();
        slot.saved_params.clear();
        for (id, value) in values {
            self.set_param(slot_index, id, value);
        }
        Ok(())
    }

    /// Run every enabled plugin slot over a stereo buffer, in slot order.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32], sample_rate: u32) -> Vec<String> {
        let mut notes = Vec::new();
        for i in 0..self.slots.len() {
            if !self.slots[i].enabled || self.slots[i].descriptor.is_none() {
                continue;
            }
            let name = self.slots[i].descriptor.as_ref().unwrap().name.clone();
            if let Err(e) = self.ensure_loaded(i) {
                self.slots[i].last_error = Some(e.clone());
                notes.push(format!("{name}: {e}"));
                continue;
            }
            let result = match self.slots[i].loaded.as_mut() {
                Some(LoadedPlugin::Vst2(p)) => { p.process(left, right, sample_rate); Ok(()) }
                Some(LoadedPlugin::Vst3(p)) => vst3_process(p, left, right, sample_rate),
                Some(LoadedPlugin::Au(p)) => p.process(left, right, sample_rate),
                None => Ok(()),
            };
            if let Err(e) = result {
                self.slots[i].last_error = Some(e.clone());
                notes.push(format!("{name}: {e}"));
            }
        }
        notes
    }

    pub fn active_count(&self) -> usize {
        self.slots.iter().filter(|s| s.enabled && s.descriptor.is_some()).count()
    }
}
