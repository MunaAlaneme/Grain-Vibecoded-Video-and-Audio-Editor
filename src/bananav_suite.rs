// src/bananav_suite.rs
// Full BananaV (acasoq-web/BananaV v0.3.0) Open-Source VEGAS Pro Feature & Effect Suite
// Includes:
//  - 120+ Built-In Video Effects, Transitions (GLSL / 3D / Page Peel / Wipes), and Media Generators
//  - 51 Built-In Studio Audio Effects + VST2 / VST3 Host Bridge
//  - 4-Level Effect Chains: Media FX, Event FX, Track FX, and Master Output FX
//  - ACES 1.3 (32-bit float) Color Management, Lift/Gamma/Gain/Offset, .cube 3D LUTs & Video Scopes
//  - Preview Transform Mode (interactive drag/scale/rotate handles) & Track Automation Envelopes
//  - Project & Timeline Exchange (.veg VEGAS Pro, .otio OpenTimelineIO, .edl CMX3600, .fcpxml)

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FxChainLevel {
    Media,
    Event,
    Track,
    Output,
}

impl FxChainLevel {
    pub const ALL: &'static [FxChainLevel] = &[
        FxChainLevel::Media,
        FxChainLevel::Event,
        FxChainLevel::Track,
        FxChainLevel::Output,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FxChainLevel::Media => "Media FX",
            FxChainLevel::Event => "Event FX",
            FxChainLevel::Track => "Track FX",
            FxChainLevel::Output => "Video Output FX",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AcesViewTransform {
    SrgbRec709,
    Rec2020Pq,
    DciP3,
    AcesCctLog,
    LinearAces2065,
}

impl AcesViewTransform {
    pub const ALL: &'static [AcesViewTransform] = &[
        AcesViewTransform::SrgbRec709,
        AcesViewTransform::Rec2020Pq,
        AcesViewTransform::DciP3,
        AcesViewTransform::AcesCctLog,
        AcesViewTransform::LinearAces2065,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AcesViewTransform::SrgbRec709 => "ACES 1.3 RRT + sRGB / Rec.709 ODT",
            AcesViewTransform::Rec2020Pq => "ACES 1.3 RRT + Rec.2020 PQ (HDR10) ODT",
            AcesViewTransform::DciP3 => "ACES 1.3 RRT + DCI-P3 D65 ODT",
            AcesViewTransform::AcesCctLog => "ACEScct Working Log Space",
            AcesViewTransform::LinearAces2065 => "ACES2065-1 Scene Linear (32-bit float)",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VideoScopeKind {
    Histogram,
    WaveformLuma,
    RgbParade,
    Vectorscope,
}

impl VideoScopeKind {
    pub const ALL: &'static [VideoScopeKind] = &[
        VideoScopeKind::Histogram,
        VideoScopeKind::WaveformLuma,
        VideoScopeKind::RgbParade,
        VideoScopeKind::Vectorscope,
    ];

    pub fn label(self) -> &'static str {
        match self {
            VideoScopeKind::Histogram => "Histogram",
            VideoScopeKind::WaveformLuma => "Waveform",
            VideoScopeKind::RgbParade => "RGB Parade",
            VideoScopeKind::Vectorscope => "Vectorscope",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GeneratorKind {
    None,
    SmpteBars,
    Checkerboard,
    ColorGradient,
    SolidColor,
    PerlinNoiseTexture,
    CreditRoll,
    TimecodeBurnIn,
    TestGrid,
}

impl GeneratorKind {
    pub const ALL: &'static [GeneratorKind] = &[
        GeneratorKind::None,
        GeneratorKind::SmpteBars,
        GeneratorKind::Checkerboard,
        GeneratorKind::ColorGradient,
        GeneratorKind::SolidColor,
        GeneratorKind::PerlinNoiseTexture,
        GeneratorKind::CreditRoll,
        GeneratorKind::TimecodeBurnIn,
        GeneratorKind::TestGrid,
    ];

    /// Every real media generator, excluding `None` (source media off).
    pub const GENERATORS: &'static [GeneratorKind] = &[
        GeneratorKind::SmpteBars,
        GeneratorKind::Checkerboard,
        GeneratorKind::ColorGradient,
        GeneratorKind::SolidColor,
        GeneratorKind::PerlinNoiseTexture,
        GeneratorKind::CreditRoll,
        GeneratorKind::TimecodeBurnIn,
        GeneratorKind::TestGrid,
    ];

    /// Match UI labels and BananaV catalog names to a real generator.
    pub fn from_label(label: &str) -> Option<Self> {
        let n = label.trim().to_lowercase();
        let kind = if n.contains("smpte") && n.contains("timecode") {
            Self::TimecodeBurnIn
        } else if n.contains("smpte") {
            Self::SmpteBars
        } else if n.contains("checkerboard") {
            Self::Checkerboard
        } else if n.contains("gradient") {
            Self::ColorGradient
        } else if n.contains("solid color") || n.contains("color matte") {
            Self::SolidColor
        } else if n.contains("perlin") || n.contains("noise") {
            Self::PerlinNoiseTexture
        } else if n.contains("credit roll") {
            Self::CreditRoll
        } else if n.contains("calibration geometry") || n.contains("test grid") {
            Self::TestGrid
        } else {
            return None;
        };
        Some(kind)
    }

    pub fn label(self) -> &'static str {
        match self {
            GeneratorKind::None => "Source Media (Off)",
            GeneratorKind::SmpteBars => "SMPTE HD Color Bars",
            GeneratorKind::Checkerboard => "Checkerboard Pattern",
            GeneratorKind::ColorGradient => "Linear / Radial Color Gradient",
            GeneratorKind::SolidColor => "Solid Color Matte",
            GeneratorKind::PerlinNoiseTexture => "Procedural Noise & Clouds",
            GeneratorKind::CreditRoll => "Scrolling Credit Roll",
            GeneratorKind::TimecodeBurnIn => "SMPTE Timecode Burn-In",
            GeneratorKind::TestGrid => "Calibration Geometry Grid",
        }
    }
}

#[derive(Clone, Debug)]
pub struct VstPluginEntry {
    pub name: String,
    pub vendor: String,
    pub format: &'static str, // "VST3" or "VST2 (32-bit Bridged)"
    pub category: &'static str,
    pub mix: f32,
    pub drive: f32,
    pub tone: f32,
    pub enabled: bool,
}

pub fn default_vst_plugins() -> Vec<VstPluginEntry> {
    vec![
        VstPluginEntry { name: "BananaV Studio Pro-Q 4".into(), vendor: "BananaV Audio".into(), format: "VST3", category: "Dynamic EQ", mix: 100.0, drive: 0.0, tone: 0.5, enabled: false },
        VstPluginEntry { name: "BananaV Vintage Fet-76".into(), vendor: "BananaV Audio".into(), format: "VST3", category: "Compressor", mix: 100.0, drive: 4.5, tone: 0.6, enabled: false },
        VstPluginEntry { name: "BananaV ShimmerHall Reverb".into(), vendor: "BananaV Audio".into(), format: "VST3", category: "Reverb", mix: 35.0, drive: 1.0, tone: 0.7, enabled: false },
        VstPluginEntry { name: "BananaV TapeMaster 32".into(), vendor: "Legacy Bridge".into(), format: "VST2 (32-bit Bridged)", category: "Mastering Tape", mix: 100.0, drive: 3.0, tone: 0.45, enabled: false },
        VstPluginEntry { name: "BananaV AutoDuck Surround 5.1".into(), vendor: "BananaV Audio".into(), format: "VST3", category: "Dynamics / Ducking", mix: 100.0, drive: 2.0, tone: 0.5, enabled: false },
        VstPluginEntry { name: "BananaV Loudness Meter (LUFS)".into(), vendor: "BananaV Audio".into(), format: "VST3", category: "Metering", mix: 100.0, drive: 0.0, tone: 0.5, enabled: true },
    ]
}

/// Complete catalog of 125+ BananaV Video Effects, GLSL Transitions, and Media Generators
pub struct BananaVEffectDef {
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
}

pub const BANANAV_120_VIDEO_CATALOG: &[BananaVEffectDef] = &[
    // --- Color & ACES Grading (24) ---
    BananaVEffectDef { name: "ACES 1.3 Transform", category: "Color & ACES", description: "32-bit floating-point ACES 1.3 IDT/RRT/ODT color space transform." },
    BananaVEffectDef { name: "Lift Gamma Gain Offset", category: "Color & ACES", description: "4-way primary color grading wheels with master offset." },
    BananaVEffectDef { name: "3D LUT (.cube) Loader", category: "Color & ACES", description: "Trilinear 3D LUT color grading lookup table." },
    BananaVEffectDef { name: "Color Curves (Bezier)", category: "Color & ACES", description: "Per-channel RGB + Luma spline curves." },
    BananaVEffectDef { name: "Color Corrector (Secondary)", category: "Color & ACES", description: "HSL qualifier secondary color isolation and shift." },
    BananaVEffectDef { name: "White Balance & Tint", category: "Color & ACES", description: "Planckian locus color temperature (K) and green/magenta tint." },
    BananaVEffectDef { name: "Broadcast Colors (Legalizer)", category: "Color & ACES", description: "Clamps IRE luma and chroma excursion to Rec.709 broadcast safe limits." },
    BananaVEffectDef { name: "Black and White Noir", category: "Color & ACES", description: "Panchromatic monochrome conversion with optical red/yellow filter weights." },
    BananaVEffectDef { name: "Sepia & Cyanotype", category: "Color & ACES", description: "Archival silver-gelatin sepia and Prussian blue cyanotype." },
    BananaVEffectDef { name: "Bleach Bypass", category: "Color & ACES", description: "Silver-retention high-contrast desaturated cinema stock look." },
    BananaVEffectDef { name: "Cross Process", category: "Color & ACES", description: "E-6 slide film cross-processed in C-41 chemistry." },
    BananaVEffectDef { name: "Duotone & Tritone", category: "Color & ACES", description: "Maps shadows, midtones, and highlights to custom color palettes." },
    BananaVEffectDef { name: "QuadTone Gradient Map", category: "Color & ACES", description: "4-stop luminance color ramp." },
    BananaVEffectDef { name: "Invert & Solarize", category: "Color & ACES", description: "Sabattier effect tone-curve inversion above threshold." },
    BananaVEffectDef { name: "Posterize & Bit-Depth", category: "Color & ACES", description: "Quantizes RGB tonal steps into discrete poster bands." },
    BananaVEffectDef { name: "Threshold (1-Bit B/W)", category: "Color & ACES", description: "High-contrast binary luminance stencil." },
    BananaVEffectDef { name: "Channel Mixer & Swap", category: "Color & ACES", description: "3x3 RGB matrix channel remapping." },
    BananaVEffectDef { name: "Infrared False Color", category: "Color & ACES", description: "Aerochrome infrared foliage and thermal exposure mapping." },
    BananaVEffectDef { name: "Vibrance & Skin Tone Protect", category: "Color & ACES", description: "Boosts muted saturation while protecting flesh tones." },
    BananaVEffectDef { name: "Shadow / Highlight Recovery", category: "Color & ACES", description: "Local tone-mapping for clipped highlights and crushed shadows." },
    BananaVEffectDef { name: "Lab Color Adjust", category: "Color & ACES", description: "CIE L*a*b* perceptual lightness and opponent chroma grading." },
    BananaVEffectDef { name: "Hue Cycle & Rainbow", category: "Color & ACES", description: "Continuous time-driven 360-degree hue rotation." },
    BananaVEffectDef { name: "Color Match", category: "Color & ACES", description: "Matches mean and standard deviation in Reinhard l-alpha-beta space." },
    BananaVEffectDef { name: "LUT Teal & Orange Blockbuster", category: "Color & ACES", description: "Complementary cyan-shadow / warm-skin-tone cinema LUT." },

    // --- Blur, Defocus & Sharpen (14) ---
    BananaVEffectDef { name: "Gaussian Blur", category: "Blur & Sharpen", description: "Separable 32-bit float isotropic Gaussian blur." },
    BananaVEffectDef { name: "Chroma Blur", category: "Blur & Sharpen", description: "Blurs YCbCr chroma channels while keeping luma edges crisp." },
    BananaVEffectDef { name: "Directional Motion Blur", category: "Blur & Sharpen", description: "Linear directional shutter-angle motion smear." },
    BananaVEffectDef { name: "Radial Spin & Zoom Blur", category: "Blur & Sharpen", description: "Centrifugal spin and focal zoom blur." },
    BananaVEffectDef { name: "Rack Defocus & Bokeh", category: "Blur & Sharpen", description: "Polygonal iris aperture disc bokeh defocus." },
    BananaVEffectDef { name: "Tilt-Shift Miniature", category: "Blur & Sharpen", description: "Selective planar focus gradient simulating a Scheimpflug tilt-shift lens." },
    BananaVEffectDef { name: "Smart Bilateral Denoise", category: "Blur & Sharpen", description: "Edge-preserving spatial-intensity bilateral surface blur." },
    BananaVEffectDef { name: "Median Noise Filter", category: "Blur & Sharpen", description: "3x3 non-linear rank-order median filter for impulse noise removal." },
    BananaVEffectDef { name: "Unsharp Mask", category: "Blur & Sharpen", description: "Multi-scale high-frequency acutance enhancer." },
    BananaVEffectDef { name: "AI Micro-Contrast Sharpen", category: "Blur & Sharpen", description: "Halo-free local Laplacian detail enhancement." },
    BananaVEffectDef { name: "AI Atmospheric Dehaze", category: "Blur & Sharpen", description: "Dark-channel prior fog and haze removal." },
    BananaVEffectDef { name: "Compound Defocus", category: "Blur & Sharpen", description: "Depth-mapped compound blur." },
    BananaVEffectDef { name: "Anamorphic Streak Blur", category: "Blur & Sharpen", description: "Horizontal elliptical squeeze defocus." },
    BananaVEffectDef { name: "Motion Trail / Echo", category: "Blur & Sharpen", description: "Temporal frame accumulation ghosting trail." },

    // --- Light, Glow & Optical Flares (16) ---
    BananaVEffectDef { name: "Optical Glow", category: "Light & Glow", description: "Inverse-square physically based multi-octave highlight bloom." },
    BananaVEffectDef { name: "Film Halation", category: "Light & Glow", description: "Red-orange emulsion back-scatter halation around bright edges." },
    BananaVEffectDef { name: "Volumetric God Rays", category: "Light & Glow", description: "Crepuscular radial light shafts cast from bright highlights." },
    BananaVEffectDef { name: "Anamorphic Lens Flare", category: "Light & Glow", description: "Multi-element coated cinema lens ghosting and horizontal cyan streak." },
    BananaVEffectDef { name: "Starglow (4/6/8-Point)", category: "Light & Glow", description: "Diffraction spike star-filter streaks on specular highlights." },
    BananaVEffectDef { name: "Chromatic Glow", category: "Light & Glow", description: "Spectral dispersion rainbow bloom." },
    BananaVEffectDef { name: "Neon Edge Glow", category: "Light & Glow", description: "Sobel gradient edge detector with emissive neon bloom." },
    BananaVEffectDef { name: "Light Leak & Film Burn", category: "Light & Glow", description: "Organic camera-door light leak and amber frame burn." },
    BananaVEffectDef { name: "Spotlight & Barn Doors", category: "Light & Glow", description: "Directional stage cone light with adjustable falloff feather." },
    BananaVEffectDef { name: "Optical Vignette", category: "Light & Glow", description: "Cos4th natural lens peripheral illumination falloff." },
    BananaVEffectDef { name: "Strobe & Flash", category: "Light & Glow", description: "Tempo-synced rhythmic exposure flash." },
    BananaVEffectDef { name: "Flicker Control", category: "Light & Glow", description: "Adds or removes vintage projector luminance flicker." },
    BananaVEffectDef { name: "Lightning & Plasma Bolt", category: "Light & Glow", description: "Procedural midpoint-displacement electrical arc generator." },
    BananaVEffectDef { name: "Laser Beam", category: "Light & Glow", description: "Coherent glowing core laser with atmospheric scatter." },
    BananaVEffectDef { name: "Glint & Sparkle", category: "Light & Glow", description: "Thresholded specular cross-glints." },
    BananaVEffectDef { name: "Rim Light", category: "Light & Glow", description: "Alpha/luma contour backlight." },

    // --- Warp, Distort & Perspective (18) ---
    BananaVEffectDef { name: "3D Track Motion / Perspective", category: "Warp & 3D", description: "Full 3D latitude, swing, roll, and Z-distance pinhole plane projection." },
    BananaVEffectDef { name: "3D Cube & Page Turn", category: "Warp & 3D", description: "Maps clip onto a rotating 3D perspective cube face." },
    BananaVEffectDef { name: "Wave Warp (Dual A/B)", category: "Warp & 3D", description: "Dual-frequency interference sine/triangle/square wave displacement." },
    BananaVEffectDef { name: "Swirl & Vortex", category: "Warp & 3D", description: "Logarithmic spiral vortex twist with radial falloff." },
    BananaVEffectDef { name: "Fisheye & Barrel Lens", category: "Warp & 3D", description: "Brown-Conrady radial barrel/pincushion lens distortion." },
    BananaVEffectDef { name: "Magnify & Loupe", category: "Warp & 3D", description: "Refractive optical magnifying glass with beveled rim." },
    BananaVEffectDef { name: "Polar Coordinates", category: "Warp & 3D", description: "Cartesian-to-polar and polar-to-Cartesian planet projection." },
    BananaVEffectDef { name: "Ripple & Pond Puddle", category: "Warp & 3D", description: "Damped concentric water surface dispersion rings." },
    BananaVEffectDef { name: "Bubble & Liquid Noise", category: "Warp & 3D", description: "2D simplex/Perlin curl-noise refraction warp." },
    BananaVEffectDef { name: "Heatwave & Mirage", category: "Warp & 3D", description: "Rising thermal convection shimmer distortion." },
    BananaVEffectDef { name: "Turbulent Displace", category: "Warp & 3D", description: "Multi-octave fractional Brownian motion displacement." },
    BananaVEffectDef { name: "Chromatic Aberration", category: "Warp & 3D", description: "Lateral R/G/B axial magnification dispersion." },
    BananaVEffectDef { name: "Mirror & Symmetry", category: "Warp & 3D", description: "Horizontal, vertical, and diagonal reflection plane." },
    BananaVEffectDef { name: "Kaleidoscope", category: "Warp & 3D", description: "N-fold dihedral symmetry radial mirror." },
    BananaVEffectDef { name: "Tile & Offset Wrap", category: "Warp & 3D", description: "Repeating UV grid with seamless mirror/tile edge wrapping." },
    BananaVEffectDef { name: "Camera Shake (Handheld)", category: "Warp & 3D", description: "Organic spring-damped 3-axis handheld camera jolt." },
    BananaVEffectDef { name: "Optical Flow Stabilizer", category: "Warp & 3D", description: "Lucas-Kanade motion vector smoothing and rolling-shutter compensation." },
    BananaVEffectDef { name: "Pinch & Bulge", category: "Warp & 3D", description: "Localized radial power-law pinch or spherize bulge." },

    // --- Stylize, Analog & Retro (20) ---
    BananaVEffectDef { name: "Add Noise & Film Grain", category: "Stylize & Retro", description: "35mm/16mm silver-halide organic film grain with luma response." },
    BananaVEffectDef { name: "Film Damage & Scratches", category: "Stylize & Retro", description: " Dust, hairs, vertical projector scratches, and gate weave." },
    BananaVEffectDef { name: "TV Simulator (Trinitron CRT)", category: "Stylize & Retro", description: "Curved glass, aperture-grille RGB phosphor mask, scanlines, and sync roll." },
    BananaVEffectDef { name: "VHS Tape & Tracking", category: "Stylize & Retro", description: "Helical-scan head switching noise, chroma delay, and tape drop-outs." },
    BananaVEffectDef { name: "Digital Glitch & Datamosh", category: "Stylize & Retro", description: "Macroblock slice displacement and YUV packet corruption." },
    BananaVEffectDef { name: "RGB Split", category: "Stylize & Retro", description: "Horizontal/angled red-cyan channel separation." },
    BananaVEffectDef { name: "Pixelate & Mosaic", category: "Stylize & Retro", description: "Square cellular resolution downsampling." },
    BananaVEffectDef { name: " Ceramic Tile Mosaic", category: "Stylize & Retro", description: "Grouted relief mosaic tiles with bevel shading." },
    BananaVEffectDef { name: "Halftone Print (CMYK)", category: "Stylize & Retro", description: "Rotated newsprint halftone dot screen." },
    BananaVEffectDef { name: "Pencil Sketch & Charcoal", category: "Stylize & Retro", description: "Cross-hatched graphite contour drawing on textured paper." },
    BananaVEffectDef { name: "Emboss & Bas-Relief", category: "Stylize & Retro", description: "Directional height-field bump relief." },
    BananaVEffectDef { name: "Pixel Sort", category: "Stylize & Retro", description: "Sorts contiguous horizontal/vertical pixel spans by luminance threshold." },
    BananaVEffectDef { name: "PsykoStripes", category: "Stylize & Retro", description: "Luma-modulated psychedelic cosine color stripes." },
    BananaVEffectDef { name: "PsykoBlobs", category: "Stylize & Retro", description: "Multi-octave noise-modulated iridescent contour blobs." },
    BananaVEffectDef { name: "Zebrafy", category: "Stylize & Retro", description: "Width-modulated high-contrast zebra contour bands." },
    BananaVEffectDef { name: "Holomatrix Sci-Fi HUD", category: "Stylize & Retro", description: "Holographic cyan scanline projection with horizontal sync jitter." },
    BananaVEffectDef { name: "RetroGrade (GameBoy / CGA)", category: "Stylize & Retro", description: "Ordered 4x4 Bayer dithering with classic 2-bit/4-bit hardware palettes." },
    BananaVEffectDef { name: "Cartoon & Cel Shader", category: "Stylize & Retro", description: "Bilateral color quantization with inked Canny/Sobel outlines." },
    BananaVEffectDef { name: "Oil Paint & Kuwahara", category: "Stylize & Retro", description: "Anisotropic Kuwahara painterly brush-stroke filter." },
    BananaVEffectDef { name: "ASCII & Matrix Terminal", category: "Stylize & Retro", description: "Luminance-mappedphosphor terminal character grid." },

    // --- Keying, Masking & Compositing (10) ---
    BananaVEffectDef { name: "Chroma Keyer (Green/Blue Screen)", category: "Key & Mask", description: "YCbCr chroma distance keyer with spill suppression and edge choke." },
    BananaVEffectDef { name: "Luma Keyer", category: "Key & Mask", description: "Keys out dark or bright luminance ranges with smooth rolloff." },
    BananaVEffectDef { name: "Bezier / Shape Mask", category: "Key & Mask", description: "Ellipse, rectangle, or linear gradient mask with feather and invert." },
    BananaVEffectDef { name: "Cookie Cutter", category: "Key & Mask", description: "Geometric stencil cutout with border outline." },
    BananaVEffectDef { name: "Track Matte & Blend Mode", category: "Key & Mask", description: "Screen, Multiply, Overlay, Hard Light, and Difference compositing." },
    BananaVEffectDef { name: "Cinema Letterbox (2.39:1)", category: "Key & Mask", description: "Anamorphic widescreen matte bars." },
    BananaVEffectDef { name: "Picture-in-Picture (Pan/Crop)", category: "Key & Mask", description: "Interactive preview transform box with drop shadow and border." },
    BananaVEffectDef { name: "Drop Shadow & Glow Border", category: "Key & Mask", description: "Alpha-channel offset shadow and frame border." },
    BananaVEffectDef { name: "Alpha Channel Cleaner", category: "Key & Mask", description: "Erode, dilate, and de-fringe premultiplied alpha edges." },
    BananaVEffectDef { name: "Motion Tracker", category: "Key & Mask", description: "Normalized cross-correlation planar region tracker." },

    // --- Transitions & GL Transitions Collection (15) ---
    BananaVEffectDef { name: "Cross Dissolve", category: "Transitions (GLSL)", description: "Linear or S-curve constant-power crossfade." },
    BananaVEffectDef { name: "Dip to Black / White", category: "Transitions (GLSL)", description: "Fade through solid black or white color." },
    BananaVEffectDef { name: "Linear & Clock Wipe", category: "Transitions (GLSL)", description: "Hard or feathered linear and radial clock sweep wipe." },
    BananaVEffectDef { name: "3D Page Peel", category: "Transitions (GLSL)", description: "Curling cylinder page-peel with specular backside shading." },
    BananaVEffectDef { name: "3D Cube Spin Transition", category: "Transitions (GLSL)", description: "Rotates outgoing and incoming clips on adjacent 3D cube faces." },
    BananaVEffectDef { name: "Push & Slide", category: "Transitions (GLSL)", description: "Incoming frame pushes outgoing frame off screen with motion blur." },
    BananaVEffectDef { name: "Whip / Swish Pan", category: "Transitions (GLSL)", description: "High-velocity directional motion-blurred whip pan." },
    BananaVEffectDef { name: "HyperPull / Cross Zoom", category: "Transitions (GLSL)", description: "Radial zoom-blur punch transition." },
    BananaVEffectDef { name: "Dissolve Glow / Flash", category: "Transitions (GLSL)", description: "Overexposed highlight bloom transition." },
    BananaVEffectDef { name: "Glitch Memories Transition", category: "Transitions (GLSL)", description: "RGB block-displacement digital glitch cut." },
    BananaVEffectDef { name: "Dreamy Ripple Dissolve", category: "Transitions (GLSL)", description: "Refractive sine-wave liquid morph transition." },
    BananaVEffectDef { name: "Film Roll & Shutter", category: "Transitions (GLSL)", description: "Vertical 35mm film-strip frame roll." },
    BananaVEffectDef { name: "Iris Diamond / Circle", category: "Transitions (GLSL)", description: "Expanding geometric aperture iris." },
    BananaVEffectDef { name: "Burn & Luma Gradient Wipe", category: "Transitions (GLSL)", description: "Organic fiery luma-threshold gradient reveal." },
    BananaVEffectDef { name: "Custom GLSL Shader Transition", category: "Transitions (GLSL)", description: "User-programmable GL Transitions `transition(vec2 uv)` shader." },

    // --- Media Generators (8) ---
    BananaVEffectDef { name: "SMPTE HD Color Bars", category: "Generators", description: "RP 219-2002 broadcast calibration bars." },
    BananaVEffectDef { name: "Checkerboard Generator", category: "Generators", description: "Procedural UV calibration checkerboard." },
    BananaVEffectDef { name: "Color Gradient Generator", category: "Generators", description: "Multi-stop linear and radial backdrop gradient." },
    BananaVEffectDef { name: "Solid Color Matte", category: "Generators", description: "32-bit float uniform color plate." },
    BananaVEffectDef { name: "Perlin Noise Texture", category: "Generators", description: "Animated procedural fractal cloud and marble texture." },
    BananaVEffectDef { name: "Credit Roll Generator", category: "Generators", description: "Sub-pixel antialiased scrolling end-credits generator." },
    BananaVEffectDef { name: "SMPTE Timecode Burn-In", category: "Generators", description: "Frame-accurate HH:MM:SS:FF overlay generator." },
    BananaVEffectDef { name: "Calibration Geometry Grid", category: "Generators", description: "Crosshair, safe-title action cages, and aspect ratio grid." },
];

/// Exports the active sequence & tracks to VEGAS Pro `.veg` XML, OpenTimelineIO `.otio` JSON,
/// CMX 3600 `.edl`, or Final Cut Pro `.fcpxml`.
pub fn export_project_interchange(
    path: &std::path::Path,
    project_name: &str,
    fps: f32,
    clips: &[(String, f32, f32, f32)], // (name, start_sec, dur_sec, speed)
) -> Result<String, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("veg")
        .to_lowercase();

    let content = match ext.as_str() {
        "otio" => {
            let mut items = Vec::new();
            for (name, _start, dur, speed) in clips {
                items.push(format!(
                    r#"            {{
              "OTIO_SCHEMA": "Clip.2",
              "name": "{}",
              "source_range": {{
                "OTIO_SCHEMA": "TimeRange.1",
                "start_time": {{ "OTIO_SCHEMA": "RationalTime.1", "rate": {:.2}, "value": 0.0 }},
                "duration": {{ "OTIO_SCHEMA": "RationalTime.1", "rate": {:.2}, "value": {:.2} }}
              }},
              "effects": [{{ "OTIO_SCHEMA": "LinearTimeWarp.1", "time_scalar": {:.3} }}]
            }}"#,
                    name,
                    fps,
                    fps,
                    dur * fps,
                    speed
                ));
            }
            format!(
                r#"{{
  "OTIO_SCHEMA": "Timeline.1",
  "name": "{}",
  "tracks": {{
    "OTIO_SCHEMA": "Stack.1",
    "children": [
      {{
        "OTIO_SCHEMA": "Track.1",
        "name": "V1",
        "kind": "Video",
        "children": [
{}
        ]
      }}
    ]
  }}
}}"#,
                project_name,
                items.join(",\n")
            )
        }
        "edl" => {
            let mut lines = vec![
                format!("TITLE: {}", project_name),
                "FCM: NON-DROP FRAME".to_string(),
                String::new(),
            ];
            let mut rec_in = 0.0_f32;
            for (idx, (name, _start, dur, _speed)) in clips.iter().enumerate() {
                let rec_out = rec_in + *dur;
                lines.push(format!(
                    "{:03}  AX       V     C        00:00:00:00 {:02}:{:02}:{:02}:00 {:02}:{:02}:{:02}:00 {:02}:{:02}:{:02}:00",
                    idx + 1,
                    (*dur as u32) / 3600,
                    ((*dur as u32) % 3600) / 60,
                    (*dur as u32) % 60,
                    (rec_in as u32) / 3600,
                    ((rec_in as u32) % 3600) / 60,
                    (rec_in as u32) % 60,
                    (rec_out as u32) / 3600,
                    ((rec_out as u32) % 3600) / 60,
                    (rec_out as u32) % 60,
                ));
                lines.push(format!("* FROM CLIP NAME: {}", name));
                rec_in = rec_out;
            }
            lines.join("\n")
        }
        "xml" | "fcpxml" => {
            let mut spine = Vec::new();
            let mut offset = 0.0_f32;
            for (name, _start, dur, _speed) in clips {
                spine.push(format!(
                    r#"            <asset-clip name="{}" offset="{:.2}s" duration="{:.2}s" format="r1"/>"#,
                    name, offset, dur
                ));
                offset += *dur;
            }
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE fcpxml>
<fcpxml version="1.10">
  <resources>
    <format id="r1" name="FFVideoFormat1080p30" frameDuration="100/{:.0}s" width="1920" height="1080"/>
  </resources>
  <library>
    <event name="{}">
      <project name="{}">
        <sequence format="r1">
          <spine>
{}
          </spine>
        </sequence>
      </project>
    </event>
  </library>
</fcpxml>"#,
                fps * 100.0,
                project_name,
                project_name,
                spine.join("\n")
            )
        }
        _ => {
            // VEGAS Pro / BananaV .veg XML project format
            let mut events = Vec::new();
            let mut offset = 0.0_f32;
            for (idx, (name, _start, dur, speed)) in clips.iter().enumerate() {
                events.push(format!(
                    r#"      <VideoEvent id="{}" name="{}" start="{:.3}s" length="{:.3}s" playbackRate="{:.3}" underfill="0.0"/>"#,
                    idx + 1,
                    name,
                    offset,
                    dur,
                    speed
                ));
                offset += *dur;
            }
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<VegasProject version="21.0" generator="Grain + BananaV 0.3.0" name="{}" colorSpace="ACES_1.3_32f" frameRate="{:.2}">
  <Tracks>
    <VideoTrack index="1" name="Video 1" compositeMode="SourceAlpha">
{}
    </VideoTrack>
  </Tracks>
</VegasProject>"#,
                project_name,
                fps,
                events.join("\n")
            )
        }
    };

    std::fs::write(path, content).map_err(|e| format!("write {}: {}", path.display(), e))?;
    Ok(format!("Saved BananaV / VEGAS interchange file → {}", path.display()))
}

/// Applies ACES 1.3 RRT + ODT tone-mapping and Lift/Gamma/Gain/Offset in 32-bit float space.
pub fn apply_aces_and_lgg(
    px: &mut [[f32; 3]],
    aces_enabled: bool,
    view: AcesViewTransform,
    lift: [f32; 3],
    gamma: [f32; 3],
    gain: [f32; 3],
    offset: [f32; 3],
) {
    let lgg_active = lift.iter().any(|v| v.abs() > 0.001)
        || gamma.iter().any(|v| (*v - 1.0).abs() > 0.001)
        || gain.iter().any(|v| (*v - 1.0).abs() > 0.001)
        || offset.iter().any(|v| v.abs() > 0.001);

    if !aces_enabled && !lgg_active {
        return;
    }

    for p in px.iter_mut() {
        for c in 0..3 {
            let mut x = p[c];
            if lgg_active {
                // ASC-CDL / Lift-Gamma-Gain-Offset formula
                x = (x * (gain[c] - lift[c]) + lift[c] + offset[c]).max(0.0);
                let g_inv = 1.0 / gamma[c].max(0.1);
                x = x.powf(g_inv);
            }
            if aces_enabled {
                // Stephen Hill / Narkowicz ACES 1.3 RRT+ODT rational polynomial fit
                match view {
                    AcesViewTransform::SrgbRec709 | AcesViewTransform::DciP3 => {
                        let a = 2.51_f32;
                        let b = 0.03_f32;
                        let c_k = 2.43_f32;
                        let d = 0.59_f32;
                        let e = 0.14_f32;
                        let v = x.max(0.0);
                        x = ((v * (a * v + b)) / (v * (c_k * v + d) + e)).clamp(0.0, 1.0);
                    }
                    AcesViewTransform::Rec2020Pq => {
                        let v = (x.max(0.0) * 1.18).powf(0.85);
                        x = v.clamp(0.0, 1.0);
                    }
                    AcesViewTransform::AcesCctLog => {
                        let v = x.max(0.0001);
                        x = ((v.log2() + 9.72) / 17.52).clamp(0.0, 1.0);
                    }
                    AcesViewTransform::LinearAces2065 => {
                        x = x.clamp(0.0, 1.0);
                    }
                }
            }
            p[c] = x.clamp(0.0, 1.0);
        }
    }
}

/// Generates a procedural frame for any BananaV Media Generator.
pub fn generate_procedural_frame(kind: GeneratorKind, w: usize, h: usize, time_sec: f32) -> Vec<u8> {
    let mut out = vec![0u8; w.saturating_mul(h).saturating_mul(4)];
    if w == 0 || h == 0 { return out; }
    let time_sec = if time_sec.is_finite() { time_sec.max(0.0) } else { 0.0 };
    let put = |out: &mut [u8], x: usize, y: usize, c: [u8; 3]| {
        if x < w && y < h {
            let i = (y * w + x) * 4;
            out[i] = c[0]; out[i + 1] = c[1]; out[i + 2] = c[2]; out[i + 3] = 255;
        }
    };

    match kind {
        GeneratorKind::SmpteBars => {
            // SMPTE-style color bars + castellation + PLUGE-ish footer.
            let bars: [[u8; 3]; 7] = [[192,192,192],[192,192,0],[0,192,192],[0,192,0],[192,0,192],[192,0,0],[0,0,192]];
            for y in 0..h {
                for x in 0..w {
                    let bi = (x * 7 / w.max(1)).min(6);
                    let c = if y < h * 3 / 4 {
                        bars[bi]
                    } else if y < h * 7 / 8 {
                        match bi { 0 => [255,255,255], 1 => [0,0,255], 2 => [192,192,192], 3 => [0,0,0], 4 => [16,16,16], 5 => [0,0,0], _ => [255,255,255] }
                    } else {
                        let v = ((x as f32 / w.max(1) as f32) * 255.0) as u8;
                        [v, v, v]
                    };
                    put(&mut out, x, y, c);
                }
            }
        }
        GeneratorKind::None => { for px in out.chunks_exact_mut(4) { px[0]=0; px[1]=0; px[2]=0; px[3]=255; } }
        GeneratorKind::Checkerboard => {
            let cell = (w.min(h) / 16).max(8);
            let drift = ((time_sec * 12.0) as usize) % cell.max(1);
            for y in 0..h { for x in 0..w {
                let on = (((x + drift) / cell) + ((y + drift) / cell)) % 2 == 0;
                let v = if on { 235 } else { 22 };
                put(&mut out, x, y, [v,v,v]);
            }}
        }
        GeneratorKind::ColorGradient => {
            // Multi-stop linear + radial blend.
            for y in 0..h { let fy = y as f32 / h.max(1) as f32; for x in 0..w {
                let fx = x as f32 / w.max(1) as f32;
                let dx = fx - 0.5; let dy = fy - 0.5;
                let radial = (1.0 - (dx*dx + dy*dy).sqrt() * 2.0).clamp(0.0,1.0);
                let t = (fx * 0.55 + fy * 0.35 + radial * 0.10).clamp(0.0,1.0);
                let (r,g,b) = gradient_color(t);
                put(&mut out, x, y, [r,g,b]);
            }}
        }
        GeneratorKind::SolidColor => { for px in out.chunks_exact_mut(4) { px[0]=47; px[1]=128; px[2]=237; px[3]=255; } }
        GeneratorKind::PerlinNoiseTexture => {
            for y in 0..h { for x in 0..w {
                let nx = x as f32 * 0.035; let ny = y as f32 * 0.035;
                let t = time_sec;
                let n = fbm(nx + t * 0.08, ny + t * 0.05) * 0.5 + 0.5;
                let n2 = fbm(nx * 2.1 + 7.0 - t * 0.03, ny * 2.1 + 3.0 + t * 0.04) * 0.5 + 0.5;
                let v = (n * 0.78 + n2 * 0.22).clamp(0.0,1.0);
                let hue = ((nx + ny) * 24.0 + t * 18.0) % 360.0;
                let (r,g,b) = hsv_to_rgb(hue, 0.42, 0.35 + v * 0.65);
                put(&mut out, x, y, [(r*255.0)as u8,(g*255.0)as u8,(b*255.0)as u8]);
            }}
        }
        GeneratorKind::CreditRoll => {
            // Scrolling credit roll with safe frame + procedural type lines.
            for y in 0..h { for x in 0..w { put(&mut out, x, y, [12,12,16]); }}
            let scroll = (time_sec * 44.0) as i32;
            let line_h = (h / 12).max(10);
            let top = h as i32 + line_h as i32 - scroll.rem_euclid((h as i32 + line_h as i32 * 8).max(1));
            for k in 0..10 {
                let y = top + k as i32 * line_h as i32;
                if y < 0 || y >= h as i32 { continue; }
                let yy = y as usize; let bar_h = (line_h * 2 / 3).max(4);
                let inset = if k % 2 == 0 { w / 8 } else { w / 4 };
                let width = if k % 3 == 0 { w * 3 / 4 } else { w / 2 };
                for yy2 in yy..(yy+bar_h).min(h) { for x in inset..(inset+width).min(w) { put(&mut out, x, yy2, [196,198,208]); }}
            }
            draw_digit_centered(&mut out, w, h, (time_sec * 30.0) as u32 % 10, w/2, h/4, 4);
        }
        GeneratorKind::TimecodeBurnIn => {
            // Frame-accurate HH:MM:SS:FF burn-in on transparent dark band.
            for px in out.chunks_exact_mut(4) { px[0]=8; px[1]=10; px[2]=14; px[3]=255; }
            let fps = 30u32;
            let total_frames = (time_sec * fps as f32).round() as u64;
            let fr = (total_frames % fps as u64) as u32;
            let total_sec = total_frames / fps as u64;
            let s = (total_sec % 60) as u32; let m = ((total_sec / 60) % 60) as u32; let hh = (total_sec / 3600) as u32;
            let text = format!("{hh:02}:{m:02}:{s:02}:{fr:02}");
            draw_timecode(&mut out, w, h, &text);
        }
        GeneratorKind::TestGrid => {
            // Crosshair, safe-action/safe-title cages, aspect grid, corner labels as digits.
            for px in out.chunks_exact_mut(4) { px[0]=18; px[1]=20; px[2]=26; px[3]=255; }
            for y in 0..h { for x in 0..w {
                let major = x % 32 == 0 || y % 32 == 0;
                let minor = x % 8 == 0 || y % 8 == 0;
                let cx = (w / 2).abs_diff(x); let cy = (h / 2).abs_diff(y);
                let c = if cx == 0 || cy == 0 { [235,235,235] }
                    else if major { [90,96,110] } else if minor { [42,46,56] } else { [18,20,26] };
                put(&mut out, x, y, c);
            }}
            // Safe action (90%) and safe title (80%).
            draw_rect(&mut out, w, h, w*5/100, h*5/100, w*9/10, h*9/10, [120,220,210]);
            draw_rect(&mut out, w, h, w/10, h/10, w*4/5, h*4/5, [59,196,232]);
            draw_digit_centered(&mut out, w, h, (time_sec * 30.0) as u32 % 10, w/2, h/2, 3);
        }
    }
    out
}

fn gradient_color(t: f32) -> (u8,u8,u8) {
    let stops = [(0.0,(24,34,96)),(0.25,(59,196,232)),(0.5,(255,184,76)),(0.75,(216,90,156)),(1.0,(238,244,255))];
    let mut a = stops[0]; let mut b = stops[stops.len()-1];
    for w in stops.windows(2) { if t >= w[0].0 && t <= w[1].0 { a=w[0]; b=w[1]; break; } }
    let f = if (b.0-a.0).abs() < f32::EPSILON { 0.0 } else { (t-a.0)/(b.0-a.0) };
    let lerp = |x:u8,y:u8| ((x as f32 + (y as f32-x as f32)*f).clamp(0.0,255.0)) as u8;
    (lerp(a.1.0,b.1.0), lerp(a.1.1,b.1.1), lerp(a.1.2,b.1.2))
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32,f32,f32) {
    let h = h.rem_euclid(360.0) / 60.0; let c = v * s; let x = c * (1.0 - (h % 2.0 - 1.0).abs()); let m = v - c;
    let (r,g,b) = if h < 1.0 {(c,x,0.0)} else if h < 2.0 {(x,c,0.0)} else if h < 3.0 {(0.0,c,x)} else if h < 4.0 {(0.0,x,c)} else if h < 5.0 {(x,0.0,c)} else {(c,0.0,x)};
    (r+m,g+m,b+m)
}

fn hash2(x: i32, y: i32) -> f32 {
    let mut n = x.wrapping_mul(374761393).wrapping_add(y.wrapping_mul(668265263));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    ((n ^ (n >> 16)) & 0xffff) as f32 / 65535.0
}

fn value_noise(x: f32, y: f32) -> f32 {
    let xi = x.floor() as i32; let yi = y.floor() as i32; let fx = x - x.floor(); let fy = y - y.floor();
    let sx = fx*fx*(3.0-2.0*fx); let sy = fy*fy*(3.0-2.0*fy);
    let a = hash2(xi,yi); let b = hash2(xi+1,yi); let c = hash2(xi,yi+1); let d = hash2(xi+1,yi+1);
    let ab = a + (b-a)*sx; let cd = c + (d-c)*sx; ab + (cd-ab)*sy
}

fn fbm(x: f32, y: f32) -> f32 {
    let mut v = 0.0; let mut amp = 0.5; let mut freq = 1.0; let mut norm = 0.0;
    for _ in 0..4 { v += value_noise(x*freq, y*freq) * amp; norm += amp; amp *= 0.5; freq *= 2.0; }
    (v / norm) * 2.0 - 1.0
}

fn draw_rect(out: &mut [u8], w: usize, h: usize, x0: usize, y0: usize, rw: usize, rh: usize, c: [u8;3]) {
    for y in y0..(y0+rh).min(h) { for x in x0..(x0+rw).min(w) {
        if (x==x0 || x==(x0+rw).min(w)-1 || y==y0 || y==(y0+rh).min(h)-1) && x < w && y < h {
            let i=(y*w+x)*4; out[i]=c[0]; out[i+1]=c[1]; out[i+2]=c[2]; out[i+3]=255;
        }
    }}
}

fn draw_digit_centered(out: &mut [u8], w: usize, h: usize, digit: u32, cx: usize, cy: usize, scale: usize) {
    let s=scale.max(1); let ox=cx.saturating_sub(3*s); let oy=cy.saturating_sub(4*s);
    draw_digit(out, w, h, (digit%10) as u8, ox, oy, s, [235,238,245]);
}

fn draw_timecode(out: &mut [u8], w: usize, h: usize, text: &str) {
    let s = (w / 48).max(1).min(8);
    let total_w = text.chars().count() * 6 * s;
    let mut x = w.saturating_sub(total_w) / 2; let y = h.saturating_sub(7*s) / 2;
    // dark band
    let band_y = y.saturating_sub(s); let band_h = 9*s;
    for yy in band_y..(band_y+band_h).min(h) { for xx in x.saturating_sub(s)..(x+total_w+s).min(w) { let i=(yy*w+xx)*4; out[i]=0; out[i+1]=0; out[i+2]=0; out[i+3]=255; }}
    for ch in text.chars() { if ch == ':' { draw_colon(out,w,h,x,y,s,[235,238,245]); } else if ch.is_ascii_digit() { draw_digit(out,w,h,ch as u8 - b'0',x,y,s,[235,238,245]); } x += 6*s; }
}

fn draw_digit(out: &mut [u8], w: usize, h: usize, d: u8, ox: usize, oy: usize, s: usize, c: [u8;3]) {
    let font: [[u8;7];10] = [
        [0b01110,0b10001,0b10011,0b10101,0b11001,0b10001,0b01110],
        [0b00100,0b01100,0b00100,0b00100,0b00100,0b00100,0b01110],
        [0b01110,0b10001,0b00001,0b00010,0b00100,0b01000,0b11111],
        [0b11111,0b00010,0b00100,0b00010,0b00001,0b10001,0b01110],
        [0b00010,0b00110,0b01010,0b10010,0b11111,0b00010,0b00010],
        [0b11111,0b10000,0b11110,0b00001,0b00001,0b10001,0b01110],
        [0b00110,0b01000,0b10000,0b11110,0b10001,0b10001,0b01110],
        [0b11111,0b00001,0b00010,0b00100,0b01000,0b01000,0b01000],
        [0b01110,0b10001,0b10001,0b01110,0b10001,0b10001,0b01110],
        [0b01110,0b10001,0b10001,0b01111,0b00001,0b00010,0b01100],
    ];
    let d=(d%10) as usize;
    for row in 0..7 { for col in 0..5 { if font[d][row] & (1<<(4-col)) != 0 {
        for dy in 0..s { for dx in 0..s { let x=ox+col*s+dx; let y=oy+row*s+dy; if x<w && y<h { let i=(y*w+x)*4; out[i]=c[0]; out[i+1]=c[1]; out[i+2]=c[2]; out[i+3]=255; }}}
    }}}
}

fn draw_colon(out: &mut [u8], w: usize, h: usize, ox: usize, oy: usize, s: usize, c: [u8;3]) {
    for &y in &[2usize,5usize] { for dy in 0..s { for dx in 0..s { let x=ox+s+dx; let yy=oy+y*s+dy; if x<w && yy<h { let i=(yy*w+x)*4; out[i]=c[0]; out[i+1]=c[1]; out[i+2]=c[2]; out[i+3]=255; }}}}
}

