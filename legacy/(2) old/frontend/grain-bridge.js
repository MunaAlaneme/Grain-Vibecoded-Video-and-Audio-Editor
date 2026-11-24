// Grain bridge: Tauri/Rust on desktop (macOS VideoToolbox / FFmpeg / DSP), WebCodecs fallback in browser.
(function () {
  function isTauriSync() {
    return !!(window.__TAURI_INTERNALS__ || window.__TAURI__);
  }

  async function invoke(cmd, args) {
    if (!isTauriSync()) throw new Error('not-tauri');
    const tauri = window.__TAURI__;
    if (tauri && tauri.core && typeof tauri.core.invoke === 'function') {
      return tauri.core.invoke(cmd, args);
    }
    if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
      return window.__TAURI_INTERNALS__.invoke(cmd, args);
    }
    throw new Error('Tauri invoke API unavailable');
  }

  async function saveDialog(defaultPath, filters) {
    const tauri = window.__TAURI__;
    if (tauri && tauri.dialog && typeof tauri.dialog.save === 'function') {
      return tauri.dialog.save({ defaultPath, filters });
    }
    return defaultPath;
  }

  window.GrainBridge = {
    async isTauri() { return isTauriSync(); },
    saveDialog,
    async processAudio(req) { return isTauriSync() ? invoke('process_audio', { req }) : null; },
    async exportVideo(req) { return isTauriSync() ? invoke('export_video', { req }) : null; },
    async listHardwareEncoders() {
      return isTauriSync() ? invoke('list_hardware_encoders') : ['webcodecs', 'ffmpeg.wasm'];
    },
    async engineInfo() {
      try { return await invoke('get_engine_info'); }
      catch { return 'Browser (WebCodecs / ffmpeg.wasm)'; }
    }
  };
})();
