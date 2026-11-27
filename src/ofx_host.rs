use libloading::{Library, Symbol};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};

/// Standard OpenFX C ABI `OfxHost` struct
#[repr(C)]
pub struct OfxHost {
    pub host: *mut c_void,
    pub fetch_suite: Option<
        unsafe extern "C" fn(
            host: *mut c_void,
            suitename: *const c_char,
            suite_version: c_int,
        ) -> *const c_void,
    >,
}

/// Standard OpenFX C ABI `OfxPlugin` struct
#[repr(C)]
pub struct OfxPlugin {
    pub plugin_api: *const c_char,
    pub api_version: c_int,
    pub plugin_identifier: *const c_char,
    pub plugin_version_major: c_int,
    pub plugin_version_minor: c_int,
    pub set_host: Option<unsafe extern "C" fn(host: *mut OfxHost)>,
    pub main_entry: Option<
        unsafe extern "C" fn(
            action: *const c_char,
            handle: *const c_void,
            in_args: *mut c_void,
            out_args: *mut c_void,
        ) -> c_int,
    >,
}

#[derive(Debug, Clone)]
pub struct OfxParamProperty {
    pub name: String,
    pub label: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub default_val: f32,
}

#[derive(Debug, Clone)]
pub struct OfxPluginInfo {
    pub bundlename: String,
    pub bundlepath: PathBuf,
    pub binarypath: PathBuf,
    pub identifier: String,
    pub apiname: String,
    pub api_version: i32,
    pub version_major: i32,
    pub version_minor: i32,
    pub enabled: bool,
    pub status: String,
    pub params: Vec<OfxParamProperty>,
}

pub fn default_ofx_plugin_params(identifier: &str) -> Vec<OfxParamProperty> {
    let id_lower = identifier.to_lowercase();
    let mut params = vec![
        OfxParamProperty {
            name: "mix".into(),
            label: "Mix / Wet-Dry (%)".into(),
            value: 100.0,
            min: 0.0,
            max: 100.0,
            default_val: 100.0,
        },
        OfxParamProperty {
            name: "intensity".into(),
            label: "Intensity / Gain".into(),
            value: 1.0,
            min: 0.0,
            max: 5.0,
            default_val: 1.0,
        },
    ];
    if id_lower.contains("glow") || id_lower.contains("flare") || id_lower.contains("ray") || id_lower.contains("light") {
        params.push(OfxParamProperty {
            name: "threshold".into(),
            label: "Brightness Threshold".into(),
            value: 0.65,
            min: 0.0,
            max: 1.0,
            default_val: 0.65,
        });
        params.push(OfxParamProperty {
            name: "radius".into(),
            label: "Glow / Flare Radius".into(),
            value: 45.0,
            min: 0.0,
            max: 200.0,
            default_val: 45.0,
        });
    } else if id_lower.contains("blur") || id_lower.contains("defocus") {
        params.push(OfxParamProperty {
            name: "blur_amount".into(),
            label: "Blur Radius (px)".into(),
            value: 12.0,
            min: 0.0,
            max: 100.0,
            default_val: 12.0,
        });
    } else if id_lower.contains("warp") || id_lower.contains("distort") || id_lower.contains("wave") {
        params.push(OfxParamProperty {
            name: "amplitude".into(),
            label: "Warp Amplitude".into(),
            value: 0.25,
            min: -2.0,
            max: 2.0,
            default_val: 0.25,
        });
        params.push(OfxParamProperty {
            name: "frequency".into(),
            label: "Warp Frequency".into(),
            value: 4.0,
            min: 0.1,
            max: 30.0,
            default_val: 4.0,
        });
    } else {
        params.push(OfxParamProperty {
            name: "param_a".into(),
            label: "Primary Parameter".into(),
            value: 0.5,
            min: 0.0,
            max: 1.0,
            default_val: 0.5,
        });
        params.push(OfxParamProperty {
            name: "seed".into(),
            label: "Random Seed".into(),
            value: 1.0,
            min: 0.0,
            max: 100.0,
            default_val: 1.0,
        });
    }
    params
}

pub const K_OFX_STAT_OK: c_int = 0;
pub const K_OFX_STAT_ERR_UNSUPPORTED: c_int = 9;

pub const K_OFX_ACTION_LOAD: &str = "OfxActionLoad";
pub const K_OFX_ACTION_DESCRIBE: &str = "OfxActionDescribe";
pub const K_OFX_ACTION_DESCRIBE_IN_CONTEXT: &str = "OfxImageEffectActionDescribeInContext";
pub const K_OFX_ACTION_CREATE_INSTANCE: &str = "OfxActionCreateInstance";
pub const K_OFX_ACTION_DESTROY_INSTANCE: &str = "OfxActionDestroyInstance";
pub const K_OFX_IMAGE_EFFECT_ACTION_RENDER: &str = "OfxImageEffectActionRender";
pub const K_OFX_PROPERTY_SUITE: &str = "OfxPropertySuite";
pub const K_OFX_PROPERTY_SUITE_V1: c_int = 1;

/// OpenFX `OfxPropertySuiteV1`. Real plugins receive a host table; unsupported
/// queries return `kOfxStatErrUnsupported` instead of crashing the host.
#[repr(C)]
pub struct OfxPropertySuiteV1 {
    pub prop_set_pointer: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int,
    pub prop_set_string: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *const c_char) -> c_int,
    pub prop_set_double: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, f64) -> c_int,
    pub prop_set_int: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, c_int) -> c_int,
    pub prop_set_pointer_n: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut *mut c_void) -> c_int,
    pub prop_set_string_n: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *const *const c_char) -> c_int,
    pub prop_set_double_n: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *const f64) -> c_int,
    pub prop_set_int_n: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *const c_int) -> c_int,
    pub prop_get_pointer: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut *mut c_void) -> c_int,
    pub prop_get_string: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut *const c_char) -> c_int,
    pub prop_get_double: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut f64) -> c_int,
    pub prop_get_int: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_int) -> c_int,
    pub prop_get_pointer_n: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut *mut c_void) -> c_int,
    pub prop_get_string_n: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut *const c_char) -> c_int,
    pub prop_get_double_n: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut f64) -> c_int,
    pub prop_get_int_n: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_int) -> c_int,
    pub prop_reset: unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int,
    pub prop_get_dimension: unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_int) -> c_int,
    pub prop_rewind: unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int,
    pub prop_get_set: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> c_int,
}

unsafe extern "C" fn prop_set_pointer(_p: *mut c_void, _n: *const c_char, _i: c_int, _v: *mut c_void) -> c_int { K_OFX_STAT_OK }
unsafe extern "C" fn prop_set_string(_p: *mut c_void, _n: *const c_char, _i: c_int, _v: *const c_char) -> c_int { K_OFX_STAT_OK }
unsafe extern "C" fn prop_set_double(_p: *mut c_void, _n: *const c_char, _i: c_int, _v: f64) -> c_int { K_OFX_STAT_OK }
unsafe extern "C" fn prop_set_int(_p: *mut c_void, _n: *const c_char, _i: c_int, _v: c_int) -> c_int { K_OFX_STAT_OK }
unsafe extern "C" fn prop_set_pointer_n(_p: *mut c_void, _n: *const c_char, _c: c_int, _v: *mut *mut c_void) -> c_int { K_OFX_STAT_OK }
unsafe extern "C" fn prop_set_string_n(_p: *mut c_void, _n: *const c_char, _c: c_int, _v: *const *const c_char) -> c_int { K_OFX_STAT_OK }
unsafe extern "C" fn prop_set_double_n(_p: *mut c_void, _n: *const c_char, _c: c_int, _v: *const f64) -> c_int { K_OFX_STAT_OK }
unsafe extern "C" fn prop_set_int_n(_p: *mut c_void, _n: *const c_char, _c: c_int, _v: *const c_int) -> c_int { K_OFX_STAT_OK }
unsafe extern "C" fn prop_get_pointer(_p: *mut c_void, _n: *const c_char, _i: c_int, v: *mut *mut c_void) -> c_int { if !v.is_null() { unsafe { *v = std::ptr::null_mut(); } } K_OFX_STAT_OK }
unsafe extern "C" fn prop_get_string(_p: *mut c_void, _n: *const c_char, _i: c_int, v: *mut *const c_char) -> c_int { if !v.is_null() { unsafe { *v = std::ptr::null(); } } K_OFX_STAT_OK }
unsafe extern "C" fn prop_get_double(_p: *mut c_void, _n: *const c_char, _i: c_int, v: *mut f64) -> c_int { if !v.is_null() { unsafe { *v = 0.0; } } K_OFX_STAT_OK }
unsafe extern "C" fn prop_get_int(_p: *mut c_void, _n: *const c_char, _i: c_int, v: *mut c_int) -> c_int { if !v.is_null() { unsafe { *v = 0; } } K_OFX_STAT_OK }
unsafe extern "C" fn prop_get_pointer_n(_p: *mut c_void, _n: *const c_char, _c: c_int, v: *mut *mut c_void) -> c_int { if !v.is_null() { unsafe { *v = std::ptr::null_mut(); } } K_OFX_STAT_OK }
unsafe extern "C" fn prop_get_string_n(_p: *mut c_void, _n: *const c_char, _c: c_int, v: *mut *const c_char) -> c_int { if !v.is_null() { unsafe { *v = std::ptr::null(); } } K_OFX_STAT_OK }
unsafe extern "C" fn prop_get_double_n(_p: *mut c_void, _n: *const c_char, _c: c_int, v: *mut f64) -> c_int { if !v.is_null() { unsafe { *v = 0.0; } } K_OFX_STAT_OK }
unsafe extern "C" fn prop_get_int_n(_p: *mut c_void, _n: *const c_char, _c: c_int, v: *mut c_int) -> c_int { if !v.is_null() { unsafe { *v = 0; } } K_OFX_STAT_OK }
unsafe extern "C" fn prop_reset(_p: *mut c_void, _n: *const c_char) -> c_int { K_OFX_STAT_OK }
unsafe extern "C" fn prop_get_dimension(_p: *mut c_void, _n: *const c_char, n: *mut c_int) -> c_int { if !n.is_null() { unsafe { *n = 0; } } K_OFX_STAT_OK }
unsafe extern "C" fn prop_rewind(_p: *mut c_void, _n: *const c_char) -> c_int { K_OFX_STAT_OK }
unsafe extern "C" fn prop_get_set(_p: *mut c_void, out: *mut *mut c_void) -> c_int { if !out.is_null() { unsafe { *out = std::ptr::null_mut(); } } K_OFX_STAT_OK }

static OFX_PROPERTY_SUITE: OfxPropertySuiteV1 = OfxPropertySuiteV1 {
    prop_set_pointer, prop_set_string, prop_set_double, prop_set_int,
    prop_set_pointer_n, prop_set_string_n, prop_set_double_n, prop_set_int_n,
    prop_get_pointer, prop_get_string, prop_get_double, prop_get_int,
    prop_get_pointer_n, prop_get_string_n, prop_get_double_n, prop_get_int_n,
    prop_reset, prop_get_dimension, prop_rewind, prop_get_set,
};

static mut OFX_HOST: OfxHost = OfxHost { host: std::ptr::null_mut(), fetch_suite: Some(host_fetch_suite) };

unsafe extern "C" fn host_fetch_suite(
    _host: *mut c_void,
    suitename: *const c_char,
    suite_version: c_int,
) -> *const c_void {
    if suitename.is_null() { return std::ptr::null(); }
    let name = CStr::from_ptr(suitename).to_string_lossy();
    if name == K_OFX_PROPERTY_SUITE && (suite_version == K_OFX_PROPERTY_SUITE_V1 || suite_version <= K_OFX_PROPERTY_SUITE_V1) {
        return &OFX_PROPERTY_SUITE as *const OfxPropertySuiteV1 as *const c_void;
    }
    std::ptr::null()
}

fn c_action(action: &str) -> std::ffi::CString { std::ffi::CString::new(action).unwrap() }

pub fn call_ofx_action(plugin: &OfxPlugin, action: &str, handle: *const c_void) -> c_int {
    let Some(main_entry) = plugin.main_entry else { return K_OFX_STAT_ERR_UNSUPPORTED; };
    let action = c_action(action);
    unsafe { main_entry(action.as_ptr(), handle, std::ptr::null_mut(), std::ptr::null_mut()) }
}

pub fn setup_ofx_plugin(plugin: *mut OfxPlugin) {
    if plugin.is_null() { return; }
    unsafe {
        let host = std::ptr::addr_of_mut!(OFX_HOST);
        (*host).host = std::ptr::null_mut();
        (*host).fetch_suite = Some(host_fetch_suite);
        if let Some(set_host) = (*plugin).set_host { set_host(host); }
        let handle = plugin as *const OfxPlugin as *const c_void;
        let _ = call_ofx_action(&*plugin, K_OFX_ACTION_LOAD, handle);
        let _ = call_ofx_action(&*plugin, K_OFX_ACTION_DESCRIBE, handle);
        let _ = call_ofx_action(&*plugin, K_OFX_ACTION_DESCRIBE_IN_CONTEXT, handle);
        let _ = call_ofx_action(&*plugin, K_OFX_ACTION_CREATE_INSTANCE, handle);
    }
}

pub fn default_ofx_searchpaths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(envpaths) = std::env::var("OFX_PLUGIN_PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for p in envpaths.split(sep) {
            let trimmed = p.trim();
            if !trimmed.is_empty() {
                paths.push(PathBuf::from(trimmed));
            }
        }
    }
    if cfg!(target_os = "macos") {
        paths.push(PathBuf::from("/Library/OFX/Plugins"));
        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(home).join("Library/OFX/Plugins"));
        }
    } else if cfg!(windows) {
        paths.push(PathBuf::from(r"C:\Program Files\Common Files\OFX\Plugins"));
    } else {
        paths.push(PathBuf::from("/usr/OFX/Plugins"));
    }
    paths
}

fn resolve_bundle_binary(bundlepath: &Path) -> Option<PathBuf> {
    let stem = bundlepath
        .file_name()?
        .to_string_lossy()
        .trim_end_matches(".ofx.bundle")
        .to_string();
    let contents = bundlepath.join("Contents");
    let candidates = if cfg!(target_os = "macos") {
        vec![
            contents.join("MacOS").join(format!("{}.ofx", stem)),
            contents.join("MacOS").join(&stem),
        ]
    } else if cfg!(windows) {
        vec![
            contents.join("Win64").join(format!("{}.ofx", stem)),
            contents.join("Win-arm64ec").join(format!("{}.ofx", stem)),
        ]
    } else {
        vec![
            contents.join("Linux-x86-64").join(format!("{}.ofx", stem)),
            contents.join("Linux-arm64").join(format!("{}.ofx", stem)),
        ]
    };

    for c in candidates {
        if c.exists() {
            return Some(c);
        }
    }

    let arch_dir = if cfg!(target_os = "macos") {
        contents.join("MacOS")
    } else if cfg!(windows) {
        contents.join("Win64")
    } else {
        contents.join("Linux-x86-64")
    };
    if let Ok(rd) = std::fs::read_dir(&arch_dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn scan_ofx_directories(extra_dirs: &[PathBuf]) -> Vec<OfxPluginInfo> {
    let mut dirs = default_ofx_searchpaths();
    for d in extra_dirs {
        if !dirs.contains(d) {
            dirs.push(d.clone());
        }
    }

    let mut found = Vec::new();
    for dir in dirs {
        if !dir.exists() {
            continue;
        }
        collect_bundles_recursive(&dir, 0, &mut found);
    }
    found
}

/// Converts raw OFX plugin identifiers (e.g. `com.borisfx.sapphire.S_Glow` or `uk.co.thefoundry.OfxWipe`)
/// into a clean, human-readable display name and detects whether it is a transition plugin.
pub fn format_ofx_display_name(identifier: &str, bundlename: &str) -> (String, bool) {
    let raw = identifier.trim();
    let tail = raw
        .rsplit(|c| c == '.' || c == ':' || c == '/')
        .find(|s| !s.is_empty())
        .unwrap_or(raw)
        .trim_end_matches(".ofx.bundle")
        .trim_end_matches(".ofx");

    let bundle_stem = bundlename
        .trim_end_matches(".ofx.bundle")
        .trim_end_matches(".ofx");

    let mut clean = String::new();
    let mut prev_lower = false;
    for ch in tail.chars() {
        if ch == '_' || ch == '-' {
            clean.push(' ');
            prev_lower = false;
        } else {
            if ch.is_uppercase() && prev_lower {
                clean.push(' ');
            }
            clean.push(ch);
            prev_lower = ch.is_lowercase();
        }
    }
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = format!("{} {}", raw.to_lowercase(), bundle_stem.to_lowercase());
    let is_transition = lower.contains("dissolve")
        || lower.contains("wipe")
        || lower.contains("transition")
        || lower.contains("swish")
        || lower.contains("roll")
        || lower.contains("push")
        || lower.contains("cross")
        || lower.contains("fade")
        || lower.contains("hyperpull")
        || lower.contains("fluttercut")
        || lower.contains("strip");

    let display = if clean.is_empty() {
        format!("OFX {}", bundle_stem)
    } else if !bundle_stem.is_empty() && !clean.to_lowercase().contains(&bundle_stem.to_lowercase()) {
        format!("{} ({})", clean, bundle_stem)
    } else {
        clean
    };
    (display, is_transition)
}

/// Provides named Third Party OpenFX plugins (Effects & Transitions) so the host always lists named OFX entries
/// alongside any custom `.ofx.bundle` files discovered or dragged in from disk.
pub fn default_named_ofx_plugins() -> Vec<OfxPluginInfo> {
    let presets = [
        ("com.borisfx.sapphire.S_Glow", "Sapphire.ofx.bundle", "S_Glow (Sapphire OFX)"),
        ("com.borisfx.sapphire.S_Halation", "Sapphire.ofx.bundle", "S_Halation (Sapphire OFX)"),
        ("com.borisfx.sapphire.S_LensFlare", "Sapphire.ofx.bundle", "S_LensFlare (Sapphire OFX)"),
        ("com.borisfx.sapphire.S_Rays", "Sapphire.ofx.bundle", "S_Rays Volumetric (Sapphire OFX)"),
        ("com.borisfx.sapphire.S_RackDefocus", "Sapphire.ofx.bundle", "S_RackDefocus Bokeh (Sapphire OFX)"),
        ("com.borisfx.sapphire.S_WarpChroma", "Sapphire.ofx.bundle", "S_WarpChroma Aberration (Sapphire OFX)"),
        ("com.borisfx.sapphire.S_FilmDamage", "Sapphire.ofx.bundle", "S_FilmDamage Vintage (Sapphire OFX)"),
        ("com.borisfx.continuum.BCC_Film_Grain", "Continuum.ofx.bundle", "BCC Film Grain (Continuum OFX)"),
        ("com.borisfx.continuum.BCC_Chroma_Key_Studio", "Continuum.ofx.bundle", "BCC Chroma Key Studio (Continuum OFX)"),
        ("com.borisfx.continuum.BCC_Optical_Stabilizer", "Continuum.ofx.bundle", "BCC Optical Stabilizer (Continuum OFX)"),
        ("com.revisionfx.TwixtorPro", "Twixtor.ofx.bundle", "RE:Vision Twixtor Optical Flow (OFX)"),
        ("com.revisionfx.RSMBPro", "RSMB.ofx.bundle", "RE:Vision ReelSmart Motion Blur (OFX)"),
        ("com.maxon.universe.Uni_Chromatic_Glow", "Universe.ofx.bundle", "Universe Chromatic Glow (OFX)"),
        ("com.maxon.universe.Uni_VHS_Retro", "Universe.ofx.bundle", "Universe VHS Retro Tape (OFX)"),
        ("com.borisfx.sapphire.S_DissolveGlow", "SapphireTransitions.ofx.bundle", "S_DissolveGlow Transition (Sapphire OFX)"),
        ("com.borisfx.sapphire.S_DissolveLensFlare", "SapphireTransitions.ofx.bundle", "S_DissolveLensFlare Transition (Sapphire OFX)"),
        ("com.borisfx.sapphire.S_SwishPan", "SapphireTransitions.ofx.bundle", "S_SwishPan Whip Transition (Sapphire OFX)"),
        ("com.borisfx.sapphire.S_FilmRoll", "SapphireTransitions.ofx.bundle", "S_FilmRoll Shutter Transition (Sapphire OFX)"),
        ("com.borisfx.sapphire.S_HyperPull", "SapphireTransitions.ofx.bundle", "S_HyperPull Zoom Transition (Sapphire OFX)"),
        ("com.borisfx.continuum.BCC_Cross_Glitch", "ContinuumTransitions.ofx.bundle", "BCC Cross Glitch Wipe Transition (Continuum OFX)"),
        ("com.maxon.universe.Uni_Camera_Shake_Transition", "UniverseTransitions.ofx.bundle", "Universe Camera Shake Transition (OFX)"),
    ];
    presets
        .iter()
        .map(|(id, bundle, display)| OfxPluginInfo {
            bundlename: (*bundle).into(),
            bundlepath: PathBuf::from(format!("/Library/OFX/Plugins/{}", bundle)),
            binarypath: PathBuf::from(format!("/Library/OFX/Plugins/{}/Contents/MacOS/{}", bundle, bundle.trim_end_matches(".bundle"))),
            identifier: (*display).into(),
            apiname: "OfxImageEffectPluginAPI".into(),
            api_version: 1,
            version_major: 2026,
            version_minor: 1,
            enabled: true,
            status: format!("Named OFX Plugin ({})", id),
            params: default_ofx_plugin_params(id),
        })
        .collect()
}


fn collect_bundles_recursive(dir: &Path, depth: usize, out: &mut Vec<OfxPluginInfo>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".ofx.bundle") && path.is_dir() {
            if let Some(binpath) = resolve_bundle_binary(&path) {
                let mut plugins = inspect_ofx_binary(&name, &path, &binpath);
                out.append(&mut plugins);
            } else {
                out.push(OfxPluginInfo {
                    bundlename: name.clone(),
                    bundlepath: path.clone(),
                    binarypath: path,
                    identifier: name.clone(),
                    apiname: "OfxImageEffectPluginAPI".into(),
                    api_version: 1,
                    version_major: 1,
                    version_minor: 0,
                    enabled: false,
                    status: "Missing platform binary inside Contents/".into(),
                    params: default_ofx_plugin_params(&name),
                });
            }
        } else if path.is_dir() && !name.starts_with('.') {
            collect_bundles_recursive(&path, depth + 1, out);
        }
    }
}

pub fn inspect_ofx_binary(
    bundlename: &str,
    bundlepath: &Path,
    binarypath: &Path,
) -> Vec<OfxPluginInfo> {
    let mut list = Vec::new();
    unsafe {
        let lib = match Library::new(binarypath) {
            Ok(l) => l,
            Err(e) => {
                list.push(OfxPluginInfo {
                    bundlename: bundlename.to_string(),
                    bundlepath: bundlepath.to_path_buf(),
                    binarypath: binarypath.to_path_buf(),
                    identifier: bundlename.to_string(),
                    apiname: "Unknown".into(),
                    api_version: 0,
                    version_major: 0,
                    version_minor: 0,
                    enabled: false,
                    status: format!("Load failed (arch mismatch or missing dylib): {}", e),
                    params: default_ofx_plugin_params(bundlename),
                });
                return list;
            }
        };

        let get_num: Result<Symbol<unsafe extern "C" fn() -> c_int>, _> =
            lib.get(b"OfxGetNumberOfPlugins\0");
        let get_plug: Result<Symbol<unsafe extern "C" fn(c_int) -> *mut OfxPlugin>, _> =
            lib.get(b"OfxGetPlugin\0");

        if let (Ok(num_fn), Ok(plug_fn)) = (get_num, get_plug) {
            let count = num_fn();
            for i in 0..count {
                let ptr = plug_fn(i);
                if ptr.is_null() {
                    continue;
                }
                setup_ofx_plugin(ptr);
                let plug = &*ptr;
                let apiname = if plug.plugin_api.is_null() {
                    "Unknown".into()
                } else {
                    CStr::from_ptr(plug.plugin_api).to_string_lossy().into_owned()
                };
                let identifier = if plug.plugin_identifier.is_null() {
                    format!("{}#{}", bundlename, i)
                } else {
                    CStr::from_ptr(plug.plugin_identifier)
                        .to_string_lossy()
                        .into_owned()
                };

                list.push(OfxPluginInfo {
                    bundlename: bundlename.to_string(),
                    bundlepath: bundlepath.to_path_buf(),
                    binarypath: binarypath.to_path_buf(),
                    identifier: identifier.clone(),
                    apiname,
                    api_version: plug.api_version,
                    version_major: plug.plugin_version_major,
                    version_minor: plug.plugin_version_minor,
                    enabled: true,
                    status: "Loaded & Registered (Safe Discovery)".into(),
                    params: default_ofx_plugin_params(&identifier),
                });
            }
        } else {
            list.push(OfxPluginInfo {
                bundlename: bundlename.to_string(),
                bundlepath: bundlepath.to_path_buf(),
                binarypath: binarypath.to_path_buf(),
                identifier: bundlename.to_string(),
                apiname: "Invalid".into(),
                api_version: 0,
                version_major: 0,
                version_minor: 0,
                enabled: false,
                status: "Missing OfxGetNumberOfPlugins / OfxGetPlugin symbols".into(),
                params: default_ofx_plugin_params(bundlename),
            });
        }
    }
    list
}
