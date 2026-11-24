// Grain bridge: Tauri/Rust on desktop, WebCodecs or ffmpeg.wasm in browser.
(function () {
  async function isTauri() {
    return !!window.__TAURI_INTERNALS__;
  }
  async function invoke(cmd, args) {
    if (!(await isTauri())) throw new Error('not-tauri');
    const { invoke: t } = await import('@tauri-apps/api/core');
    return t(cmd, args);
  }
  window.GrainBridge = {
    isTauri,
    async processAudio(req) { return (await isTauri()) ? invoke('process_audio', { req }) : null; },
    async exportVideo(req) {
      if (await isTauri()) return invoke('export_video', { req });
      // Browser: fall through to existing WebCodecs or (future) ffmpeg.wasm
      console.info('Browser export — WebCodecs / ffmpeg.wasm path');
      return null;
    },
    async listHardwareEncoders() {
      return (await isTauri()) ? invoke('list_hardware_encoders') : ['webcodecs', 'ffmpeg.wasm'];
    },
    async engineInfo() {
      try { return await invoke('get_engine_info'); } catch { return 'Browser (WebCodecs / ffmpeg.wasm)'; }
    }
  };
})();
