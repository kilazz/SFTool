//******************************************************************************************
// File:            pp_fxaa.fx
// Description:     FXAA 3.11, Post-AA Sharpness, Procedural Film Grain & Diagnostics
// Pipeline Status: Executed LAST (Final display presentation to swapchain)
// Architecture:    Direct3D 9 / Shader Model 3.0
//******************************************************************************************

#ifndef WIN32

// COMMON LIBRARIES & STRUCTURES
#include "fx_common.fxh"

// INTERNAL FXAA 3.11 QUALITY PRESET CONSTANTS
#define FXAA_QUALITY_P0 1.0f
#define FXAA_QUALITY_P1 1.5f
#define FXAA_QUALITY_P2 2.0f
#define FXAA_QUALITY_P3 2.0f
#define FXAA_QUALITY_P4 2.0f
#define FXAA_QUALITY_P5 2.0f
#define FXAA_QUALITY_P6 4.0f
#define FXAA_QUALITY_P7 8.0f

#define FxaaBool   bool
#define FxaaFloat  float
#define FxaaFloat2 float2
#define FxaaFloat3 float3
#define FxaaFloat4 float4
#define FxaaSat(a) saturate((a))
#define FxaaTexTop(t, p) tex2Dlod(t, float4(p, 0.0f, 0.0f))

// Standard Rec.601 luma for gamma/sRGB display-encoded edge detection
#define FxaaLuma(rgba) dot((rgba).rgb, float3(0.299f, 0.587f, 0.114f))

// EXTERNAL ENGINE PARAMETERS
float4 Offset;
texture SceneTexture;

sampler2D SceneImage = sampler_state
{
    Texture = <SceneTexture>;
    MinFilter = Linear;
    MagFilter = Linear;
    MipFilter = None;
    AddressU = Clamp;
    AddressV = Clamp;
};

// CUSTOM I/O STRUCTURE
struct VS_OUTPUT_FXAA
{
    float4 Position : POSITION;
    float4 TexCoord[5] : TEXCOORD0;
};

// VERTEX SHADER
VS_OUTPUT_FXAA VS_FXAA(const VS_INPUT input)
{
    VS_OUTPUT_FXAA o = (VS_OUTPUT_FXAA) 0;

    o.Position = input.Pos;
    o.TexCoord[0].xyzw = input.TexCoord0.xyzw;

    // Orthogonal neighbors (S, E, N, W)
    o.TexCoord[1].xy = input.TexCoord0.xy + (float2(0.0f, 1.0f) * Offset.xy);
    o.TexCoord[1].zw = input.TexCoord0.xy + (float2(1.0f, 0.0f) * Offset.xy);
    o.TexCoord[2].xy = input.TexCoord0.xy + (float2(0.0f, -1.0f) * Offset.xy);
    o.TexCoord[2].zw = input.TexCoord0.xy + (float2(-1.0f, 0.0f) * Offset.xy);

    // Diagonal neighbors (NW, SE, NE, SW)
    o.TexCoord[3].xy = input.TexCoord0.xy + (float2(-1.0f, -1.0f) * Offset.xy);
    o.TexCoord[3].zw = input.TexCoord0.xy + (float2(1.0f, 1.0f) * Offset.xy);
    o.TexCoord[4].xy = input.TexCoord0.xy + (float2(1.0f, -1.0f) * Offset.xy);
    o.TexCoord[4].zw = input.TexCoord0.xy + (float2(-1.0f, 1.0f) * Offset.xy);

    return o;
}

// CORE FXAA 3.11 ALGORITHM
void FXAAFX(inout float3 cOut, in float4 TexCoord[5])
{
    FxaaFloat2 posM = TexCoord[0].xy;
    FxaaFloat4 rgbyM = FxaaTexTop(SceneImage, posM);
    FxaaFloat lumaM = FxaaLuma(rgbyM);

    // Sample orthogonal neighbors
    FxaaFloat lumaS = FxaaLuma(FxaaTexTop(SceneImage, TexCoord[1].xy));
    FxaaFloat lumaE = FxaaLuma(FxaaTexTop(SceneImage, TexCoord[1].zw));
    FxaaFloat lumaN = FxaaLuma(FxaaTexTop(SceneImage, TexCoord[2].xy));
    FxaaFloat lumaW = FxaaLuma(FxaaTexTop(SceneImage, TexCoord[2].zw));

    // Contrast range determination
    FxaaFloat rangeMax = max(max(lumaN, lumaW), max(max(lumaS, lumaM), lumaE));
    FxaaFloat rangeMin = min(min(lumaN, lumaW), min(min(lumaS, lumaM), lumaE));
    FxaaFloat range = rangeMax - rangeMin;

    // Early exit threshold check
    FxaaFloat rangeMaxClamped = max(FXAA_QUALITY_EDGETHRESHOLDMIN, rangeMax * FXAA_QUALITY_EDGETHRESHOLD);

    if (range < rangeMaxClamped)
    {
#if defined(FXAA_DEBUG_MODE) && (FXAA_DEBUG_MODE != 0)
        cOut = GetLuma(cOut).xxx * 0.20f;
#endif
        return;
    }

    // Sample diagonal neighbors
    FxaaFloat lumaNW = FxaaLuma(FxaaTexTop(SceneImage, TexCoord[3].xy));
    FxaaFloat lumaSE = FxaaLuma(FxaaTexTop(SceneImage, TexCoord[3].zw));
    FxaaFloat lumaNE = FxaaLuma(FxaaTexTop(SceneImage, TexCoord[4].xy));
    FxaaFloat lumaSW = FxaaLuma(FxaaTexTop(SceneImage, TexCoord[4].zw));

    FxaaFloat lumaNS = lumaN + lumaS;
    FxaaFloat lumaWE = lumaW + lumaE;

    // Edge orientation detection
    FxaaFloat edgeHorz = abs(-2.0f * lumaM + lumaNS) * 2.0f + abs(-2.0f * lumaE + lumaNE + lumaSE) + abs(-2.0f * lumaW + lumaNW + lumaSW);
    FxaaFloat edgeVert = abs(-2.0f * lumaM + lumaWE) * 2.0f + abs(-2.0f * lumaN + lumaNW + lumaNE) + abs(-2.0f * lumaS + lumaSW + lumaSE);

    FxaaBool horzSpan = edgeHorz >= edgeVert;
    FxaaFloat lengthSign = horzSpan ? Offset.y : Offset.x;

    if (!horzSpan)
    {
        lumaN = lumaW;
        lumaS = lumaE;
    }

    // Sub-pixel lowpass average
    FxaaFloat subpixA = (lumaNS + lumaWE) * 2.0f + (lumaNW + lumaNE + lumaSW + lumaSE);
    FxaaFloat subpixB = (subpixA * (1.0f / 12.0f)) - lumaM;

    // Gradient steepness and pair selection
    FxaaFloat gradientN = lumaN - lumaM;
    FxaaFloat gradientS = lumaS - lumaM;
    FxaaFloat lumaNN = lumaN + lumaM;
    FxaaFloat lumaSS = lumaS + lumaM;
    FxaaBool pairN = abs(gradientN) >= abs(gradientS);
    FxaaFloat gradient = max(abs(gradientN), abs(gradientS));
    if (pairN)
        lengthSign = -lengthSign;

    // Normalized subpixel contrast
    FxaaFloat subpixC = FxaaSat(abs(subpixB) * (1.0f / range));

    // Edge search starting coordinates
    FxaaFloat2 posB = posM;
    if (!horzSpan)
        posB.x += lengthSign * 0.5f;
    else
        posB.y += lengthSign * 0.5f;

    FxaaFloat2 offNP = horzSpan ? float2(Offset.x, 0.0f) : float2(0.0f, Offset.y);
    FxaaFloat2 posN = posB - (offNP * FXAA_QUALITY_P0);
    FxaaFloat2 posP = posB + (offNP * FXAA_QUALITY_P0);

    FxaaFloat lumaEndN = FxaaLuma(FxaaTexTop(SceneImage, posN));
    FxaaFloat lumaEndP = FxaaLuma(FxaaTexTop(SceneImage, posP));

    if (!pairN)
        lumaNN = lumaSS;
    FxaaFloat gradientScaled = gradient * 0.25f;
    FxaaFloat lumaMM = lumaM - lumaNN * 0.5f;
    FxaaBool lumaMLTZero = lumaMM < 0.0f;

    lumaEndN -= lumaNN * 0.5f;
    lumaEndP -= lumaNN * 0.5f;
    FxaaBool doneN = abs(lumaEndN) >= gradientScaled;
    FxaaBool doneP = abs(lumaEndP) >= gradientScaled;

    FxaaFloat qualitySteps[7] =
    {
        FXAA_QUALITY_P1, FXAA_QUALITY_P2, FXAA_QUALITY_P3,
        FXAA_QUALITY_P4, FXAA_QUALITY_P5, FXAA_QUALITY_P6, FXAA_QUALITY_P7
    };

    for (int i = 0; i < 7; i++)
    {
        if (doneN && doneP)
            break;

        if (!doneN)
        {
            posN -= offNP * qualitySteps[i];
            lumaEndN = FxaaLuma(FxaaTexTop(SceneImage, posN)) - lumaNN * 0.5f;
            doneN = abs(lumaEndN) >= gradientScaled;
        }
        if (!doneP)
        {
            posP += offNP * qualitySteps[i];
            lumaEndP = FxaaLuma(FxaaTexTop(SceneImage, posP)) - lumaNN * 0.5f;
            doneP = abs(lumaEndP) >= gradientScaled;
        }
    }

    FxaaFloat dstN = horzSpan ? (posM.x - posN.x) : (posM.y - posN.y);
    FxaaFloat dstP = horzSpan ? (posP.x - posM.x) : (posP.y - posM.y);

    FxaaBool goodSpanN = (lumaEndN < 0.0f) != lumaMLTZero;
    FxaaBool goodSpanP = (lumaEndP < 0.0f) != lumaMLTZero;
    FxaaBool directionN = dstN < dstP;
    FxaaBool goodSpan = directionN ? goodSpanN : goodSpanP;

    FxaaFloat spanLengthRcp = 1.0f / (dstN + dstP);
    FxaaFloat pixelOffset = (min(dstN, dstP) * (-spanLengthRcp)) + 0.5f;
    FxaaFloat pixelOffsetGood = goodSpan ? pixelOffset : 0.0f;

    FxaaFloat subpixF = (-2.0f * subpixC + 3.0f) * (subpixC * subpixC);
    FxaaFloat subpixH = (subpixF * subpixF) * FXAA_QUALITY_SUBPIX;

    FxaaFloat finalOffset = max(pixelOffsetGood, subpixH) * lengthSign;

#if defined(FXAA_DEBUG_MODE) && (FXAA_DEBUG_MODE == 1)
    cOut = horzSpan ? float3(1.0f, 0.1f, 0.1f) : float3(0.1f, 0.4f, 1.0f);
    return;
#elif defined(FXAA_DEBUG_MODE) && (FXAA_DEBUG_MODE == 2)
    float heat = saturate(abs(finalOffset) * (horzSpan ? (1.0f / Offset.y) : (1.0f / Offset.x)));
    cOut = lerp(float3(0.0f, 1.0f, 0.2f), float3(1.0f, 0.05f, 0.0f), heat);
    return;
#elif defined(FXAA_DEBUG_MODE) && (FXAA_DEBUG_MODE == 3)
    cOut = lerp(float3(0.1f, 0.1f, 0.3f), float3(1.0f, 1.0f, 0.0f), saturate(subpixH * 2.0f));
    return;
#endif

    if (!horzSpan)
        posM.x += finalOffset;
    else
        posM.y += finalOffset;

    cOut.rgb = FxaaTexTop(SceneImage, posM).xyz;
}

// PIXEL SHADER: FINAL DISPLAY PRESENTATION
float4 PS_FXAA(const VS_OUTPUT_FXAA input) : COLOR
{
    float2 uv = input.TexCoord[0].xy;
    float4 centerTex = FxaaTexTop(SceneImage, uv);
    float3 cOut = centerTex.rgb;
    float engineAlpha = centerTex.a; // Engine native 3D material/geometry mask

    // FAST APPROXIMATE ANTI-ALIASING (FXAA 3.11)
#if ENABLE_AA == 1
    FXAAFX(cOut, input.TexCoord);

    // POST-AA MICRO-SHARPENING
    // Reuses the four orthogonal taps already loaded in L1 cache
#if ENABLE_POST_FXAA_SHARPEN == 1
    float3 lumaNeighbors = (
        FxaaTexTop(SceneImage, input.TexCoord[1].xy).rgb + // S
        FxaaTexTop(SceneImage, input.TexCoord[1].zw).rgb + // E
        FxaaTexTop(SceneImage, input.TexCoord[2].xy).rgb + // N
        FxaaTexTop(SceneImage, input.TexCoord[2].zw).rgb   // W
    ) * 0.25f;

    float3 sharpDiff = cOut - lumaNeighbors;
    cOut += clamp(sharpDiff * FX_POST_SHARPEN_STRENGTH, -0.05f, 0.05f);
#endif
#endif

#if defined(FXAA_DEBUG_MODE) && (FXAA_DEBUG_MODE != 0)
    return float4(saturate(cOut), 1.0f);
#endif

    // GLOBAL PIPELINE DIAGNOSTIC SUITE (FX_DEBUG_VIEW)
#if defined(FX_DEBUG_VIEW) && (FX_DEBUG_VIEW != 0)

#if FX_DEBUG_VIEW == 1
        // MODE 1: Inspect Engine 3D Alpha Mask (Silhouettes, Units, Terrain Mask)
    return float4(engineAlpha.xxx, 1.0f);

#elif FX_DEBUG_VIEW == 2
        // MODE 2: 5-Step Grayscale SDR Calibration Bar (0%, 25%, 50%, 75%, 100%)
        if (uv.y > 0.88f && uv.x > 0.20f && uv.x < 0.80f)
        {
            float normX    = (uv.x - 0.20f) / 0.60f;
            float stepVal  = floor(normX * 5.0f) / 4.0f; // Discrete 0.0, 0.25, 0.50, 0.75, 1.0
            float gridTick = step(0.03f, frac(normX * 5.0f));
            return float4(LinearTosRGB(stepVal.xxx * gridTick), 1.0f);
        }
        return float4(saturate(cOut), 1.0f);

#elif FX_DEBUG_VIEW == 3
        // MODE 3: False-Color Exposure Zones (IRE-like Heatmap)
        return float4(Tonemap_FalseColor(cOut), 1.0f);

#elif FX_DEBUG_VIEW == 4
        // MODE 4: Hard Clipping & True Black Detector
        // Magenta = Blown out speculars (>= 0.995)
        // Cyan    = True Black crushed to zero (Luma <= 0.005)
        float dbgLuma = GetLuma(cOut);
        if (any(cOut >= 0.995f)) return float4(1.0f, 0.0f, 1.0f, 1.0f);
        if (dbgLuma <= 0.005f)   return float4(0.0f, 1.0f, 1.0f, 1.0f);
        return float4(dbgLuma.xxx * 0.35f, 1.0f);

#endif

#endif

    // DISPLAY SPACE COLOR SHAPING
#if ENABLE_SPLIT_TONING == 1
    float st_luma = GetLuma(cOut);
    cOut *= lerp(float3(1.0f, 1.0f, 1.0f), FX_SPLIT_HIGH_COLOR, smoothstep(0.5f, 1.0f, st_luma) * FX_SPLIT_TONING_AMOUNT);
    cOut *= lerp(float3(1.0f, 1.0f, 1.0f), FX_SPLIT_LOW_COLOR, (1.0f - smoothstep(0.0f, 0.5f, st_luma)) * FX_SPLIT_TONING_AMOUNT);
#endif

#if ENABLE_TRUE_BLACK == 1
    cOut = ApplySoftTrueBlack(cOut, FX_TRUE_BLACK_STRENGTH);
#endif

    // HARDWARE RESOLUTION RECONSTRUCTION & PROCEDURAL GRAIN
    // Universal screen-space resolution reconstruction:
    // Derives hardware pixel count from partial derivatives or engine Offset
    float2 screenRes = float2(1.0f / max(abs(ddx(uv.x)), 1e-6f), 1.0f / max(abs(ddy(uv.y)), 1e-6f));
    float2 pixelPos = floor(uv * screenRes);

#if ENABLE_FILM_GRAIN == 1
    float luma = GetLuma(cOut);

    // Procedural animated 35mm film grain (Zero-texture ALU generation)
    // Temporal phase scrolled by Offset.x provided by engine
    float temporalPhase = frac(Offset.x * 43.758f);
    float grainSeed = frac(52.9829189f * frac(dot(pixelPos, float2(0.06711056f, 0.00583715f)) + temporalPhase));

    // Optical film shoulder: grain dissolves in deep blacks and extreme highlights
    float grainWeight = saturate(1.0f - abs(luma - 0.5f) * 2.0f);
    grainWeight *= saturate(1.0f - (luma * FX_GRAIN_LUMA_WEIGHT * 0.5f));

#if ENABLE_COLOR_CODED_GRAIN == 1
    float3 colorMask = abs(cOut - 0.5f) * FX_GRAIN_COLOR_SENSITIVITY;
    float3 finalWeight = grainWeight * (1.0f - saturate(colorMask));
#else
    float3 finalWeight = grainWeight;
#endif

    cOut += (grainSeed - 0.5f) * (FX_NOISE_INTENSITY * 0.035f) * finalWeight;
#endif

    // VIGNETTE
#if ENABLE_VIGNETTE == 1
    cOut = ApplyVignette(cOut, uv);
#endif

    // SWAPCHAIN QUANTIZATION & TEMPORAL DITHERING
#if ENABLE_IGN_DITHERING == 1
    cOut += (GetIGN(uv, screenRes) - 0.5f) * (1.0f / 255.0f) * FX_IGN_STRENGTH;
#endif

#if ENABLE_FRC_DITHERING == 1
    float ditherSeed = frac(Offset.x * 17.135f);
#if FX_FRC_MODE == 0
    float frc = GetFRC_Bayer(uv, screenRes, ditherSeed);
#else
    float frc = GetFRC_IGN(uv, screenRes, ditherSeed);
#endif
    cOut += frc * (1.0f / (exp2(FX_DITHER_BITS) - 1.0f)) * FX_IGN_STRENGTH;
#endif

    return float4(saturate(cOut), 1.0f);
}

// ENGINE DISPATCH TECHNIQUES
technique FXAA
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_FXAA();
        PixelShader = compile ps_3_0 PS_FXAA();
    }
}

#endif // WIN32
