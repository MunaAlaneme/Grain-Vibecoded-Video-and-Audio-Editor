// Grain desktop bridge — classic script (attaches to window).
// Desktop (Tauri): routes processing/export to Rust.
// Browser: no-op so existing WebCodecs / pure-JS paths continue to work.

(function () {
  async function isTauri() {
    return typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__;
  }

  async function invoke(cmd, args) {
    if (!(await isTauri())) throw new Error('not-tauri');
    // Dynamic import keeps the browser build free of the dependency
    const { invoke: tauriInvoke } = await import('@tauri-apps/api/core');
    return tauriInvoke(cmd, args);
  }

  window.GrainBridge = {
    isTauri,
    async engineInfo() {
      try { return await invoke('get_engine_info'); }
      catch { return 'Browser / WebCodecs engine'; }
    },
    async processAudio(req) {
      if (await isTauri()) return invoke('process_audio', { req });
      return null; // caller falls back to in-page DSP
    },
    async exportVideo(req) {
      if (await isTauri()) return invoke('export_video', { req });
      return null;
    },
    async listHardwareEncoders() {
      if (await isTauri()) return invoke('list_hardware_encoders');
      return ['browser-webcodecs'];
    }
  };
})();
