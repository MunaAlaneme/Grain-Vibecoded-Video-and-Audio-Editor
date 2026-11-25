use libloading::{Library, Symbol};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};

/// Standard OpenFX C ABI `OfxHost` struct
#[repr(C)]
pub struct OfxHost {
    pub host: *mut c_void,
    pub fetch_suite: Option<
        unsafe extern "C" fn(
            host: *mut c_void,
            suite_name: *const c_char,
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
pub struct OfxPluginInfo {
    pub bundle_name: String,
    pub bundle_path: PathBuf,
    pub binary_path: PathBuf,
    pub identifier: String,
    pub api_name: String,
    pub api_version: i32,
    pub version_major: i32,
    pub version_minor: i32,
    pub enabled: bool,
    pub status: String,
}

// Dummy function pointers for suite vtables to prevent bus errors / null dereference
unsafe extern "C" fn dummy_suite_fn() -> c_int {
    14 // kOfxStatErrUnsupported
}

#[repr(C)]
pub struct DummySuiteVtable {
    pub fn1: unsafe extern "C" fn() -> c_int,
    pub fn2: unsafe extern "C" fn() -> c_int,
    pub fn3: unsafe extern "C" fn() -> c_int,
    pub fn4: unsafe extern "C" fn() -> c_int,
    pub fn5: unsafe extern "C" fn() -> c_int,
    pub fn6: unsafe extern "C" fn() -> c_int,
    pub fn7: unsafe extern "C" fn() -> c_int,
    pub fn8: unsafe extern "C" fn() -> c_int,
}

static DUMMY_SUITE: DummySuiteVtable = DummySuiteVtable {
    fn1: dummy_suite_fn,
    fn2: dummy_suite_fn,
    fn3: dummy_suite_fn,
    fn4: dummy_suite_fn,
    fn5: dummy_suite_fn,
    fn6: dummy_suite_fn,
    fn7: dummy_suite_fn,
    fn8: dummy_suite_fn,
};

unsafe extern "C" fn host_fetch_suite(
    _host: *mut c_void,
    suite_name: *const c_char,
    _suite_version: c_int,
) -> *const c_void {
    if suite_name.is_null() {
        return std::ptr::null();
    }
    let name_c = CStr::from_ptr(suite_name);
    let name_str = name_c.to_string_lossy();

    // Return a properly aligned vtable pointer for core suites
    if name_str.contains("OfxPropertySuite")
        || name_str.contains("OfxParameterSuite")
        || name_str.contains("OfxImageEffectSuite")
        || name_str.contains("OfxMessageSuite")
        || name_str.contains("OfxInteractSuite")
    {
        &DUMMY_SUITE as *const DummySuiteVtable as *const c_void
    } else {
        std::ptr::null()
    }
}

pub fn default_ofx_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(env_paths) = std::env::var("OFX_PLUGIN_PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for p in env_paths.split(sep) {
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

fn resolve_bundle_binary(bundle_path: &Path) -> Option<PathBuf> {
    let stem = bundle_path
        .file_name()?
        .to_string_lossy()
        .trim_end_matches(".ofx.bundle")
        .to_string();
    let contents = bundle_path.join("Contents");
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
    let mut dirs = default_ofx_search_paths();
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
            if let Some(bin_path) = resolve_bundle_binary(&path) {
                let mut plugins = inspect_ofx_binary(&name, &path, &bin_path);
                out.append(&mut plugins);
            } else {
                out.push(OfxPluginInfo {
                    bundle_name: name.clone(),
                    bundle_path: path.clone(),
                    binary_path: path,
                    identifier: name,
                    api_name: "OfxImageEffectPluginAPI".into(),
                    api_version: 1,
                    version_major: 1,
                    version_minor: 0,
                    enabled: false,
                    status: "Missing platform binary inside Contents/".into(),
                });
            }
        } else if path.is_dir() && !name.starts_with('.') {
            collect_bundles_recursive(&path, depth + 1, out);
        }
    }
}

pub fn inspect_ofx_binary(
    bundle_name: &str,
    bundle_path: &Path,
    binary_path: &Path,
) -> Vec<OfxPluginInfo> {
    let mut list = Vec::new();
    unsafe {
        let lib = match Library::new(binary_path) {
            Ok(l) => l,
            Err(e) => {
                list.push(OfxPluginInfo {
                    bundle_name: bundle_name.to_string(),
                    bundle_path: bundle_path.to_path_buf(),
                    binary_path: binary_path.to_path_buf(),
                    identifier: bundle_name.to_string(),
                    api_name: "Unknown".into(),
                    api_version: 0,
                    version_major: 0,
                    version_minor: 0,
                    enabled: false,
                    status: format!("Load failed (arch mismatch or missing dylib): {}", e),
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
            let mut host = OfxHost {
                host: std::ptr::null_mut(),
                fetch_suite: Some(host_fetch_suite),
            };
            let action_load = CString::new("OfxActionLoad").unwrap();

            for i in 0..count {
                let ptr = plug_fn(i);
                if ptr.is_null() {
                    continue;
                }
                let plug = &*ptr;
                let api_name = if plug.plugin_api.is_null() {
                    "Unknown".into()
                } else {
                    CStr::from_ptr(plug.plugin_api).to_string_lossy().into_owned()
                };
                let identifier = if plug.plugin_identifier.is_null() {
                    format!("{}#{}", bundle_name, i)
                } else {
                    CStr::from_ptr(plug.plugin_identifier)
                        .to_string_lossy()
                        .into_owned()
                };

                if let Some(set_host_fn) = plug.set_host {
                    set_host_fn(&mut host as *mut OfxHost);
                }
                let status_str = if let Some(entry_fn) = plug.main_entry {
                    let rc = std::panic::catch_unwind(|| {
                        entry_fn(
                            action_load.as_ptr(),
                            std::ptr::null(),
                            std::ptr::null_mut(),
                            std::ptr::null_mut(),
                        )
                    });
                    match rc {
                        Ok(0) | Ok(14) => "Loaded & Initialized (OFX Ready)".into(),
                        Ok(code) => format!("Loaded (OfxActionLoad returned {})", code),
                        Err(_) => "Loaded with caught C++ exception/panic".into(),
                    }
                } else {
                    "Loaded (no mainEntry)".into()
                };

                list.push(OfxPluginInfo {
                    bundle_name: bundle_name.to_string(),
                    bundle_path: bundle_path.to_path_buf(),
                    binary_path: binary_path.to_path_buf(),
                    identifier,
                    api_name,
                    api_version: plug.api_version,
                    version_major: plug.plugin_version_major,
                    version_minor: plug.plugin_version_minor,
                    enabled: true,
                    status: status_str,
                });
            }
        } else {
            list.push(OfxPluginInfo {
                bundle_name: bundle_name.to_string(),
                bundle_path: bundle_path.to_path_buf(),
                binary_path: binary_path.to_path_buf(),
                identifier: bundle_name.to_string(),
                api_name: "Invalid".into(),
                api_version: 0,
                version_major: 0,
                version_minor: 0,
                enabled: false,
                status: "Missing OfxGetNumberOfPlugins / OfxGetPlugin symbols".into(),
            });
        }
    }
    list
}
