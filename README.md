# Grain Video & Audio Studio — Pure Rust + OpenFX Host + BorisFX Sapphire S_ Suite

100% Pure Rust native desktop video & audio editor for Apple Silicon (`aarch64-apple-darwin`) featuring:
- **Zero compiler errors or warnings** (`error[E0502]` and all `float_literal_f32_fallback` warnings resolved).
- **Native OpenFX (OFX 1.4 / 1.5) C ABI Host (`src/ofx_host.rs`)**: Automatically scans `/Library/OFX/Plugins`, `~/Library/OFX/Plugins`, and `OFX_PLUGIN_PATH` (plus manual `.ofx.bundle` loading), resolves platform binaries (`Contents/MacOS/*.ofx`), invokes `OfxGetNumberOfPlugins`, `OfxGetPlugin`, `setHost`, and `OfxActionLoad`, and exposes installed OFX plugins in the **OpenFX** tab.
- **Expanded BorisFX Sapphire `S_` Effects & Transitions**:
  - **Light & Flares (`S_`)**: `S_Glow`, `S_GlowNoise`, `S_GlowRainbow`, `S_GlowOrthicon`, `S_Halation`, `S_Glare`, `S_Glint`, `S_Sparkle`, `S_Flashbulb`, `S_Rays`, `S_Streaks`, `S_LensFlare`, `S_EdgeRays`, `S_LightLeak`.
  - **Blur & Distort (`S_`)**: `S_WarpWaves2`, `S_WarpPerspective`, `S_WarpFishEye`, `S_WarpMagnify`, `S_WarpPolar`, `S_WarpBubble2`, `S_WarpVortex`, `S_WarpChroma`, `S_WarpPuddle`, `S_WarpTransform`, `S_WarpPuff`, `S_BlurDirectional`, `S_BlurRadial`, `S_BlurMotion`, `S_RackDefocus`, `S_ZBlur`, `S_EdgeBlur`, `S_Beauty`.
  - **Stylize & Render (`S_`)**: `S_QuadTone`, `S_MultiTone`, `S_TriTone`, `S_Zebrafy`, `S_PixelSort`, `S_EmbossDistort`, `S_EmbossShiny`, `S_Zap`, `S_Aurora`, `S_Caustics`, `S_Clouds`, `S_TextureCells`, `S_AutoPaint`, `S_Cartoon`, `S_Etching`, `S_ScanLines`, `S_FilmDamage`, `S_Shake`, `S_PsykoStripes`, `S_PsykoBlobs`.
  - **Transitions (`S_`)**: `S_DissolveGlow`, `S_DissolveLensFlare`, `S_DissolveUltraGlow`, `S_FilmRoll`, `S_SwishPan`, `S_HyperPull`.

## Build & Run
```bash
cd Grain-Video-Editor-Pure-Rust-Complete
cargo run --release
```

## Programs Used:

Gemini AI

Grok AI

Claude AI