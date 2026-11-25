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

unsafe extern "C" fn host_fetch_suite(
    _host: *mut c_void,
    _suitename: *const c_char,
    _suite_version: c_int,
) -> *const c_void {
    std::ptr::null()
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
