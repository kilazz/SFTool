//******************************************************************************************
// File:            pp_glow.fx
// Description:     Physical Linear Bloom Composite, Optics & Master Tonemapping
// Pipeline Status: Executed at the junction of Scene + Bloom
// Architecture:    Direct3D 9 / Shader Model 3.0
//******************************************************************************************

#ifndef WIN32

// COMMON LIBRARIES & STRUCTURES
#include "fx_common.fxh"

// EXTERNAL ENGINE PARAMETERS
float4 Offset;

// TEXTURES & SAMPLERS
texture SceneTexture;
texture GlowTexture;

sampler2D SceneImage = sampler_state
{
    Texture = <SceneTexture>;
    MinFilter = Linear;
    MagFilter = Linear;
    MipFilter = None;
    AddressU = Clamp;
    AddressV = Clamp;
};

sampler2D GlowImage = sampler_state
{
    Texture = <GlowTexture>;
    MinFilter = Linear;
    MagFilter = Linear;
    MipFilter = None;
    AddressU = Clamp;
    AddressV = Clamp;
};

// VERTEX SHADER
VS_OUTPUT VS_Glow(const VS_INPUT input)
{
    VS_OUTPUT o = (VS_OUTPUT) 0;

    o.Position = input.Pos;
    o.TexCoord0.xy = input.TexCoord0.xy;

    // Preserve engine-native aberration parameter vector passed by C++
    o.TexCoord0.zw = Offset.zw * FX_GLOW_CHROMATIC_ABERRATION;

    return o;
}

// HIGHLIGHT EXTRACTION (Directly from Scene-Linear Light)
float4 PS_MaskHighLights(const VS_OUTPUT input) : COLOR
{
    float2 uv = input.TexCoord0.xy;

    // SceneImage is in pure Scene-Linear HDR
    float3 linearCol = max(_TexTop(SceneImage, uv).rgb, 0.0f);
    float lum = GetLuma(linearCol);

    float softKnee = smoothstep(
        FX_MASKHIGHLIGHTS_THRESHOLD - FX_MASKHIGHLIGHTS_KNEE,
        FX_MASKHIGHLIGHTS_THRESHOLD + FX_MASKHIGHLIGHTS_KNEE,
        lum
    );

    float bloomAmount = max(lum - (FX_MASKHIGHLIGHTS_THRESHOLD - FX_MASKHIGHLIGHTS_KNEE), 0.0f);
    float3 masked = linearCol * bloomAmount * softKnee * FX_MASKHIGHLIGHTS_INTENSITY;

    // ATMOSPHERIC PSEUDO-DEPTH (DARK CHANNEL PRIOR)
#if ENABLE_PSEUDO_DEPTH == 1
    float darkChannel = min(linearCol.r, min(linearCol.g, linearCol.b));
    float pseudoDepth = saturate(darkChannel * FX_PSEUDO_DEPTH_SCALE);
    float depthWeight = lerp(0.45f, 1.35f, pseudoDepth);

    // Strongly saturated pixels (fire, magic, glowing crystals) always have a LOW dark channel and
    // would be treated as "near" and dimmed the most, the opposite of what ITM does to them.
    // Exempt true emitters; skin and diffuse colors (chroma < ~0.55) keep the depth weighting.
    float maxCh = max(linearCol.r, max(linearCol.g, linearCol.b));
    float chroma = (maxCh - darkChannel) / max(maxCh, 1e-4f);
    depthWeight = lerp(depthWeight, 1.0f, smoothstep(0.55f, 0.90f, chroma));

    masked *= depthWeight;
#endif

    return float4(FX_RT_CLAMP(masked), 1.0f);
}

// PHYSICAL OPTICAL COMPOSITE & MASTER TONEMAPPING
float4 PS_Glow(const VS_OUTPUT input) : COLOR
{
    float2 uv = input.TexCoord0.xy;
    float4 sceneTex = _TexTop(SceneImage, uv);
    float3 sceneLin = max(sceneTex.rgb, 0.0f);
    float noiseAlpha = sceneTex.a;

    float3 glowBase = max(_TexTop(GlowImage, uv).rgb, 0.0f);
    float3 finalGlow = glowBase;

    // Volumetric God Rays
#if ENABLE_GOD_RAYS == 1
    float2 rayDelta = (FX_SUN_POSITION - uv) * (FX_GODRAYS_DENSITY * 0.125f);
    float rayJitter = GetIGN(uv, 1.0f / max(Offset.xy, 1e-6f));
    float2 marchCoord = uv + rayDelta * rayJitter;
    float3 accumulatedRays = 0.0f;
    float rayWeight = 1.0f;
    float totalWeight = 0.0f;

    for (int r = 0; r < 8; r++)
    {
        marchCoord += rayDelta;
        accumulatedRays += max(_TexTop(GlowImage, marchCoord).rgb, 0.0f) * rayWeight;
        totalWeight += rayWeight;
        rayWeight *= FX_GODRAYS_DECAY;
    }

    accumulatedRays *= (1.0f / totalWeight);
    float rayEdgeFade = saturate(1.0f - distance(uv, float2(0.5f, 0.5f)) * 0.75f);
    finalGlow += accumulatedRays * FX_GODRAYS_COLOR * (FX_GODRAYS_INTENSITY * rayEdgeFade);
#endif

    // Screen-Space Subsurface Scattering
#if ENABLE_SSS == 1
    float sceneLuma = GetLuma(sceneLin);

    float isSkin = saturate((sceneLin.r - sceneLin.g) * 5.0f) *
                   saturate((sceneLin.g - sceneLin.b) * 6.0f) *
                   smoothstep(0.04f, 0.35f, sceneLuma) *
                   (1.0f - smoothstep(0.70f, 1.0f, sceneLuma));

    float isFoliage = saturate((sceneLin.g - sceneLin.r) * 5.0f) *
                      saturate((sceneLin.g - sceneLin.b) * 5.0f) *
                      smoothstep(0.04f, 0.40f, sceneLuma);

    float3 sssProfile = (isSkin * FX_SSS_SKIN_COLOR) + (isFoliage * FX_SSS_FOLIAGE_COLOR);
    finalGlow += glowBase * sssProfile * FX_SSS_INTENSITY;
#endif

    // Anamorphic Streaks
#if ENABLE_ANAMORPHIC_STREAKS == 1
    float3 streaks      = 0.0f;
    float streakWeight  = 0.0f;
    float2 streakOffset = float2(FX_STREAK_LENGTH * 0.01f * FX_RES_SCALE, 0.0f);

    for (int i = -6; i <= 6; i++)
    {
        float fi = (float)i;
        float weight = exp(-abs(fi) * 0.4f);
        streaks += max(_TexTop(GlowImage, uv + streakOffset * fi).rgb, 0.0f) * weight;
        streakWeight += weight;
    }

    finalGlow += (streaks / streakWeight) * FX_STREAK_COLOR * FX_STREAK_INTENSITY;
#endif

    // Chromatic Aberration
#if ENABLE_CHROMATIC_ABERRATION == 1
    float2 dirToCenter = uv - 0.5f;
    float distToCenter = length(dirToCenter);
    float2 caDir = (distToCenter > 1e-4f) ? (dirToCenter / distToCenter) : 0.0f;
    float caDistMask = distToCenter * distToCenter;
    float2 caShift = caDir * Offset.xy * (FX_GLOW_CHROMATIC_ABERRATION * 1.5f * FX_RES_SCALE) * caDistMask;

#if ENABLE_PRISMATIC_CA == 1
    float3 tapR = max(_TexTop(SceneImage, uv + caShift * 1.00f).rgb, 0.0f);
    float3 tapG = max(_TexTop(SceneImage, uv + caShift * 0.25f).rgb, 0.0f);
    float3 tapB = max(_TexTop(SceneImage, uv - caShift * 0.75f).rgb, 0.0f);

    sceneLin.r = tapR.r;
    sceneLin.g = (tapR.g * 0.15f + tapG.g * 0.70f + tapB.g * 0.15f);
    sceneLin.b = tapB.b;

    // Shift only the bloom base; god rays / SSS / streaks added above must survive in R and B
    finalGlow.r += max(_TexTop(GlowImage, uv + caShift * 2.0f).rgb, 0.0f).r - glowBase.r;
    finalGlow.b += max(_TexTop(GlowImage, uv - caShift * 1.5f).rgb, 0.0f).b - glowBase.b;
#else
    sceneLin.r = max(_TexTop(SceneImage, uv + caShift).rgb, 0.0f).r;
    sceneLin.b = max(_TexTop(SceneImage, uv - caShift).rgb, 0.0f).b;

    finalGlow.r += max(_TexTop(GlowImage, uv + caShift * 2.0f).rgb, 0.0f).r - glowBase.r;
    finalGlow.b += max(_TexTop(GlowImage, uv - caShift * 2.0f).rgb, 0.0f).b - glowBase.b;
#endif
#endif

    // Film Halation
#if ENABLE_FILM_HALATION == 1
    float halationMask = smoothstep(0.4f, 1.0f, GetLuma(glowBase));
    finalGlow += glowBase * float3(1.0f, 0.15f, 0.05f) * halationMask * FX_HALATION_INTENSITY;
#endif

    // Procedural Organic Lens Dirt
#if ENABLE_LENS_DIRT == 1
    finalGlow += finalGlow * ProceduralDirt(uv) * FX_DIRT_INTENSITY;
#endif

    // ---------------------------------------------------------------------
    // OPTICAL COMPOSITE BLEND
    // ---------------------------------------------------------------------
    float3 glowLight = max(finalGlow, 0.0f) * FX_BLOOM_INTENSITY;

#if FX_BLOOM_ENERGY_CONSERVING == 1
    float3 compositeLin = sceneLin + glowLight * saturate(1.0f - sceneLin * 0.20f);
#else
    float3 compositeLin = sceneLin + glowLight;
#endif

    // ---------------------------------------------------------------------
    // MASTER TONEMAPPING
    // ---------------------------------------------------------------------
    float3 cOut;

    // MASTER TONEMAPPING: Executed on the combined physical light
#if FX_TONEMAP_MODE == 1
    cOut = linearToPQ_Vec(compositeLin);
#elif FX_TONEMAP_MODE == 2
    cOut = Tonemap_FalseColor(compositeLin);
#elif FX_TONEMAP_MODE == 3
    cOut = Tonemap_KhronosPBRNeutral(compositeLin);
#elif FX_TONEMAP_MODE == 4
    cOut = Tonemap_ACES2_Fit(compositeLin);
#elif FX_TONEMAP_MODE == 5
    cOut = Tonemap_AgX(compositeLin);
#elif FX_TONEMAP_MODE == 6
    cOut = Tonemap_GT7(compositeLin);
#elif FX_TONEMAP_MODE == 7
    cOut = Tonemap_CineTone(compositeLin);
#elif FX_TONEMAP_MODE == 8
    cOut = Tonemap_Lumina(compositeLin);
#else
    cOut = compositeLin;
#endif

    // Return to Perceptual Display sRGB space
#if FX_TONEMAP_MODE != 1 && FX_TONEMAP_MODE != 2
#if (ENABLE_ITM == 1) || (FX_GAMMA_CORRECTION == 1) || (FX_TONEMAP_MODE != 0)
    cOut = LinearTosRGB(cOut);
#endif
#endif

    return float4(FX_RT_CLAMP(cOut), noiseAlpha);
}

// ENGINE TECHNIQUES
technique MaskHighLights
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_Glow();
        PixelShader = compile ps_3_0 PS_MaskHighLights();
    }
}

technique Glow
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_Glow();
        PixelShader = compile ps_3_0 PS_Glow();
    }
}

#endif // WIN32
