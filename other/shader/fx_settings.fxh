//******************************************************************************************
// Spellforce 2 - Demons of the Past
//
// File:         fx_settings.fxh
// Description:  Global User Configuration & Scene-Linear HDR Post-Processing Pipeline
// Architecture: Direct3D 9 / Shader Model 3.0
//******************************************************************************************

#ifndef FX_SETTINGS_H
#define FX_SETTINGS_H

// PIPELINE TARGET CONFIGURATION
// 1 = Patched Executable (D3DFMT_A16B16G16R16F - 16-bit Float HDR enabled via binary patch)
// 0 = Vanilla Executable (D3DFMT_A8R8G8B8 - Original unpatched 8-bit SDR binary)
#define FX_ENABLE_FP16_PATCH          1

// AUTOMATIC TUNING BASED ON PIPELINE PRECISION
#if FX_ENABLE_FP16_PATCH == 1
#define FX_ITM_HDR_PEAK               10000.0f                       // Target HDR peak in nits (Physical range)
#define FX_MASKHIGHLIGHTS_THRESHOLD   0.86f                          // Bloom extraction threshold
#define FX_MASKHIGHLIGHTS_KNEE        0.25f                          // Soft knee starting at 0.70
#define FX_MASKHIGHLIGHTS_INTENSITY   1.40f                          // Physical bloom extraction scale
#define FX_BLOOM_ROLLOFF              0.95f                          // Headroom for specular highlights
#else
#define FX_ITM_HDR_PEAK               260.0f                         // Conservative peak for 8-bit SDR fallback
#define FX_MASKHIGHLIGHTS_THRESHOLD   0.72f                          // Adapted SDR threshold
#define FX_MASKHIGHLIGHTS_KNEE        0.20f                          // Soft knee starting at 0.52
#define FX_MASKHIGHLIGHTS_INTENSITY   1.15f                          // Adapted intensity for SDR
#define FX_BLOOM_ROLLOFF              0.85f                          // Early roll-off to avoid hard clipping
#endif

// TONEMAPPING & DISPLAY CALIBRATION
// 0 = Off
// 1 = HDR Pass-through
// 2 = False Color (Debug Heatmap)
// 3 = Khronos PBR Neutral
// 4 = ACES 2.0 Fit
// 5 = AgX
// 6 = GT7
// 7 = CineTone
// 8 = Lumina
#define FX_TONEMAP_MODE               8

#define FX_GAMMA_CORRECTION           1                              // 1 = Scene-Referred Linear Workflow

// Output EOTF (Electro-Optical Transfer Function) for Display Presentation:
// 0 = Pure Gamma 2.2 (Recommended: prevents milky lifted shadows on 95% of consumer monitors)
// 1 = IEC 61966-2-1 Piecewise sRGB (Strict standard with linear toe below 0.04045)
#define FX_OUTPUT_EOTF                0

// Global Manual Exposure Multiplier (Used when Auto-Exposure is disabled)
#define FX_EXPOSURE                   1.25f

// Instantaneous Grid Auto-Exposure without Frame History (16-tap sparse grid)
#define ENABLE_AUTO_EXPOSURE          0                              // 1 = Adapt exposure dynamically based on scene brightness
#define FX_AUTO_EXP_TARGET            0.18f                          // Standard 18% middle gray reflectance target
#define FX_AUTO_EXP_MIN               0.70f                          // Minimum exposure floor (prevents solar washout)
#define FX_AUTO_EXP_MAX               1.85f                          // Maximum exposure ceiling in dark dungeons

// INVERSE TONE MAPPING (SCENE-LINEAR EXPANDER)
#define ENABLE_ITM                    1                              // 1 = Reconstruct physical scene light energy
#define FX_ITM_MAX_GAIN               8.0f                           // Strict maximum multiplier cap over SDR white

// BLACK LEVEL RESTORATION (ANTI-FOG DE-HAZING)
#define ENABLE_TRUE_BLACK             1                              // Removes lifted milky ambient haze
#define FX_TRUE_BLACK_STRENGTH        0.35f                          // De-hazing intensity (0.0 to 1.0)

// SPATIAL DETAIL & LOCAL CONTRAST (LCE)
#define ENABLE_LCE                    1                              // High-frequency local contrast enhancement
#define FX_LCE_STRENGTH               0.20f                          // Local contrast intensity
#define FX_LCE_SAMPLES                2.00f                          // Kernel radius multiplier (auto-scaled by resolution)
#define FX_LCE_SHADOW_THRESHOLD       0.05f                          // Protect pure blacks from noise amplification
#define FX_LCE_MAX_LIMIT              0.40f                          // Halo and ringing clamp limit

// ZERO-COST MICRO-AO (CAVITY SHADOWS)
#define ENABLE_MICRO_AO               1                              // Synthesizes contact micro-shadows from 8-tap LCE
#define FX_MICRO_AO_STRENGTH          0.35f                          // Crevice/crease ambient occlusion intensity

// SCREEN-Y PSEUDO-DEPTH HEURISTICS (ISOMETRIC CAMERA TILT ~50 DEG)
#define ENABLE_SCREEN_Y_DEPTH         1                              // 1 = Top of screen is treated as farther away
#define FX_AERIAL_HAZE_STRENGTH       0.16f                          // Atmospheric aerial haze toward the horizon
#define FX_AERIAL_HAZE_COLOR          float3(0.55f, 0.70f, 0.95f)    // Cool skylight haze tint

// PROCEDURAL ENVIRONMENTAL PROBE (IBL)
#define ENABLE_ENV_PROBE              1                              // 1 = Enable ambient sky/ground probe
#define FX_ENV_BUMP_SCALE             2.50f                          // Relief sensitivity to sky bounce
#define FX_ENV_AMBIENT_INTENSITY      0.25f                          // Sky ambient fill light strength in shadows
#define FX_ENV_SPECULAR_INTENSITY     0.40f                          // Environmental specular sheen reflection strength

// Environmental Probe Color Palette (Physical sky/ground irradiance model)
#define FX_ENV_SKY_COLOR              float3(0.40f, 0.65f, 1.00f)    // Cold zenith skylight
#define FX_ENV_HORIZON_COLOR          float3(0.85f, 0.75f, 0.60f)    // Warm solar horizon bounce
#define FX_ENV_GROUND_COLOR           float3(0.30f, 0.25f, 0.15f)    // Soil / vegetation nadir bounce

// CINEMATIC COLOR GRADING (ASC-CDL)
#define ENABLE_LOG_GRADING            1                              // Process color grading in ACEScct Log space
#define ENABLE_ASC_CDL                0                              // American Society of Cinematographers CDL
#define FX_CDL_SLOPE                  float3( 1.08f,  1.04f,  0.95f) // Gain
#define FX_CDL_OFFSET                 float3(-0.02f, -0.01f,  0.03f) // Lift
#define FX_CDL_POWER                  float3( 1.05f,  1.05f,  1.05f) // Gamma

// INTELLIGENT COLOR HANDLING
#define ENABLE_SMART_VIBRANCE         0                              // Perceptual saturation boost for muted tones
#define FX_VIBRANCE_AMOUNT            0.35f

#define ENABLE_PURKINJE               1                              // Scotopic human eye blue-shift in darkness
#define FX_PURKINJE_STRENGTH          0.75f
#define FX_PURKINJE_DARK_LEVEL        0.10f                          // Luminance threshold triggering Purkinje effect

// COLOR VOLUMES & GAMUT PRESERVATION
#define ENABLE_GAMUT_COMPRESSION      0                              // Compresses out-of-gamut specular spikes
#define FX_GAMUT_THRESHOLD            1.20f
#define FX_GAMUT_STRENGTH             0.50f

#define FX_WIDE_GAMUT_EMULATION       0                              // Simulates ACEScg / AP1 wide color gamut primaries
#define FX_GAMUT_EXPANSION            0.25f

// 3D LOOKUP TABLE (LUT) - RATIO-PRESERVING HDR TRANSFER
#define ENABLE_LUT                    1                              // 1 = Enable External 3D Color Grade
#define LUT_SIZE                      32.0f                          // 16.0 for original SF2 LUTs, 32.0 for custom LUTs
#define FX_LUT_PRE_EXPOSURE           1                              // 1 = LUT evaluates pre-exposure SDR baseline and applies true ratio transfer

// SPLIT TONING (Applied Post-Tonemap in Display Space)
#define ENABLE_SPLIT_TONING           0                              // 1 = Enable, 0 = Disable
#define FX_SPLIT_TONING_AMOUNT        0.15f
#define FX_SPLIT_HIGH_COLOR           float3( 1.00f,  0.93f,  0.85f) // Warm highlight tint
#define FX_SPLIT_LOW_COLOR            float3( 0.85f,  0.90f,  1.00f) // Cool shadow tint

// AMD FIDELITYFX RCAS (ROBUST CONTRAST ADAPTIVE SHARPENING)
#define FX_RCAS_SHARPNESS             0.40f                          // Sharpening intensity
#define FX_RCAS_LIMIT                 0.1875f                        // Safety ringing limit
#define FX_RCAS_DENOISE               1                              // Suppress sharpening on flat/low-contrast noise

// BLOOM, GLOW & OPTICAL COMPOSITE
#define ENABLE_KARIS_DOWNSAMPLE       1                              // Anti-flicker luma weighted 13-tap downsampling
#define FX_BLUR_DESATURATE_INTENSITY  0.35f                          // Desaturates bloom core to mimic overexposure

#define FX_BLOOM_INTENSITY            0.70f                          // Master bloom multiplier in final composite
#define FX_BLOOM_ENERGY_CONSERVING    1                              // 1 = Soft photographic rolloff blend; 0 = Pure additive

// ATMOSPHERIC PSEUDO-DEPTH (DARK CHANNEL PRIOR)
#define ENABLE_PSEUDO_DEPTH           1                              // Reconstructs depth from aerial haze without Z-buffer
#define FX_PSEUDO_DEPTH_SCALE         1.80f                          // Prevents near weapons/faces from blinding camera

// VOLUMETRIC SUN SHAFTS / GOD RAYS
#define ENABLE_GOD_RAYS               1                              // Light shafts cast through silhouettes and foliage
#define FX_SUN_POSITION               float2(0.50f, 0.08f)           // Screen-space Sun origin
#define FX_GODRAYS_INTENSITY          0.35f                          // Ray brilliance multiplier
#define FX_GODRAYS_DENSITY            0.90f                          // Ray projection distance scale
#define FX_GODRAYS_DECAY              0.86f                          // Step attenuation factor
#define FX_GODRAYS_COLOR              float3(1.0f, 0.88f, 0.65f)     // Warm golden sunlight tint

// SCREEN-SPACE SUBSURFACE SCATTERING
#define ENABLE_SSS                    1                              // Simulates light bleeding through skin and leaves
#define FX_SSS_INTENSITY              0.45f                          // Subsurface scatter bleed strength
#define FX_SSS_SKIN_COLOR             float3(1.0f, 0.28f, 0.06f)     // Subdermal capillary/blood transmission tint
#define FX_SSS_FOLIAGE_COLOR          float3(0.4f, 0.95f, 0.12f)     // Chlorophyll back-light transmission tint

// ANAMORPHIC STREAKS & OPTICAL ARTIFACTS
#define ENABLE_ANAMORPHIC_STREAKS     0                              // Horizontal cinema light flares
#define FX_STREAK_INTENSITY           0.30f
#define FX_STREAK_LENGTH              1.50f
#define FX_STREAK_COLOR               float3(0.4f, 0.7f, 1.0f)       // Cyan/blue flare tint

#define ENABLE_FILM_HALATION          0                              // Red boundary bleed around high-energy highlights
#define FX_HALATION_INTENSITY         0.75f

#define ENABLE_LENS_GHOSTING          0                              // Internal multi-element lens reflections
#define FX_GHOSTING_INTENSITY         0.15f

#define ENABLE_LENS_DIRT              0                              // Organic lens smudges revealed by bloom
#define FX_DIRT_INTENSITY             0.25f

// CHROMATIC ABERRATION
#define ENABLE_CHROMATIC_ABERRATION   1                              // Master chromatic aberration switch
#define ENABLE_PRISMATIC_CA           1                              // 1 = Continuous 3-band rainbow spectrum
#define FX_GLOW_CHROMATIC_ABERRATION  1.50f                          // Radial pixel fringing width at screen corners

// LENS & CAMERA EMULATION
#define ENABLE_LENS_DISTORTION        0                              // Barrel curvature / spherize distortion
#define FX_LENS_DISTORTION            0.03f

#define ENABLE_VIGNETTE               0                              // Optical corner light falloff
#define FX_VIGNETTE_STRENGTH          0.70f

// FILM GRAIN
#define ENABLE_FILM_GRAIN             1                              // Organic film emulsion grain
#define ENABLE_COLOR_CODED_GRAIN      1                              // Chromatic grain variation
#define FX_NOISE_SEED_CHANNEL         r                              // Channel of NoiseTexture written into alpha seed bus
#define FX_NOISE_INTENSITY            0.85f                          // Overall grain amplitude
#define FX_GRAIN_LUMA_WEIGHT          1.00f                          // Suppresses grain in deep shadows and highlights
#define FX_GRAIN_COLOR_SENSITIVITY    0.40f

// ANTI-ALIASING & POST-AA RESTORATION
#define ENABLE_AA                     1
#define FXAA_QUALITY_EDGETHRESHOLD    0.200f                         // Contrast threshold required to trigger smoothing
#define FXAA_QUALITY_EDGETHRESHOLDMIN 0.0625f                        // Minimum absolute luma for edge detection
#define FXAA_QUALITY_SUBPIX           0.50f                          // Sub-pixel point alias smoothing
#define FXAA_DEBUG_MODE               0                              // 0 = Off, 1 = Edge Orientation, 2 = Offset Heatmap

#define ENABLE_POST_FXAA_SHARPEN      1                              // 1 = Restores fine texture micro-contrast lost to FXAA
#define FX_POST_SHARPEN_STRENGTH      0.20f                          // Unsharp mask weight using cached orthogonal taps

// DISPLAY QUANTIZATION & DITHERING
#define ENABLE_IGN_DITHERING          0                              // Static Interleaved Gradient Noise
#define ENABLE_FRC_DITHERING          1                              // Frame Rate Control temporal dithering
#define FX_DITHER_BITS                8.0f                           // Display bit depth (8-bit monitor swapchain)
#define FX_FRC_MODE                   1                              // 0 = Bayer 2x2 with random phase, 1 = IGN with random phase
#define FX_IGN_STRENGTH               1.0f                           // Dithering intensity

// DIAGNOSTIC MODES (PP_DEBUG.FX)
#define FX_DEBUG_VIEW                 0                              // 0 = Normal Game View, 1 = Alpha Channel, 2 = Gradient Ramp, 3 = False-Color Luminance, 4 = Clip / NaN / Inf Detector

#endif // FX_SETTINGS_H
