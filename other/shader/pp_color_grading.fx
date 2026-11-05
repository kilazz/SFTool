//******************************************************************************************
// File:            pp_color_grading.fx
// Description:     Scene-Linear ITM, Depth Heuristic, Auto-Exposure, LCE & Environmental Probe
// Pipeline Status: Executed FIRST (Inputs: Raw Scene, Outputs: Pure Scene-Linear FP16)
// Architecture:    Direct3D 9 / Shader Model 3.0
//******************************************************************************************

#ifndef WIN32

// COMMON LIBRARIES & STRUCTURES
#include "fx_common.fxh"

// EXTERNAL ENGINE PARAMETERS
float4 Offset;

// TEXTURES & SAMPLERS
texture SceneTexture;
texture LUTTexture;

sampler2D SceneImage = sampler_state
{
    Texture = <SceneTexture>;
    MinFilter = Linear;
    MagFilter = Linear;
    MipFilter = None;
    AddressU = Clamp;
    AddressV = Clamp;
};

sampler2D LUTImage = sampler_state
{
    Texture = <LUTTexture>;
    MinFilter = Linear;
    MagFilter = Linear;
    MipFilter = Linear;
    AddressU = Clamp;
    AddressV = Clamp;
};

// VERTEX SHADER
VS_OUTPUT VS_ColorGrading(const VS_INPUT input)
{
    VS_OUTPUT o = (VS_OUTPUT) 0;

    o.Position = input.Pos;
    o.TexCoord0 = input.TexCoord0;

    return o;
}

// PIXEL SHADER: MASTER COLOR GRADING & SCENE-LINEAR HDR PIPELINE
float4 PS_ColorGrading(VS_OUTPUT input) : COLOR
{
    float2 uv = input.TexCoord0.xy;
    float3 sourceColor = max(_TexTop(SceneImage, uv).rgb, 0.0f);
    float3 color = sourceColor;

#if (FX_GAMMA_CORRECTION == 1) || (ENABLE_ITM == 1)
    color = sRGBToLinear(color);
#endif

    // ---------------------------------------------------------------------
    // LOCAL STATISTICS EXTRACTION (Resolution-scaled 8-tap star pattern)
    // ---------------------------------------------------------------------
    float centerLuma = 0.0f;
    float avgLuma = 0.0f;
    float localContrast = 0.0f;

#if ENABLE_LCE == 1
    float2 lceOff = FX_LCE_SAMPLES * Offset.xy * FX_RES_SCALE;
    centerLuma = GetFastLinearLuma(sourceColor);

    // Diagonal X-Shape Taps
    float n0 = GetFastLinearLuma(_TexTop(SceneImage, uv + float2( lceOff.x,  lceOff.y)).rgb);
    float n1 = GetFastLinearLuma(_TexTop(SceneImage, uv + float2(-lceOff.x, -lceOff.y)).rgb);
    float n2 = GetFastLinearLuma(_TexTop(SceneImage, uv + float2( lceOff.x, -lceOff.y)).rgb);
    float n3 = GetFastLinearLuma(_TexTop(SceneImage, uv + float2(-lceOff.x,  lceOff.y)).rgb);

    // Orthogonal Cross Taps
    float n4 = GetFastLinearLuma(_TexTop(SceneImage, uv + float2( lceOff.x,  0.0f    )).rgb);
    float n5 = GetFastLinearLuma(_TexTop(SceneImage, uv + float2(-lceOff.x,  0.0f    )).rgb);
    float n6 = GetFastLinearLuma(_TexTop(SceneImage, uv + float2( 0.0f,      lceOff.y)).rgb);
    float n7 = GetFastLinearLuma(_TexTop(SceneImage, uv + float2( 0.0f,     -lceOff.y)).rgb);

    avgLuma = (n0 + n1 + n2 + n3 + n4 + n5 + n6 + n7) * 0.125f;

    // A small isolated bright feature has high center-to-neighborhood contrast.
    // Broad diffuse surfaces (snow, stone, clouds) evaluate near zero.
    localContrast = saturate((centerLuma - avgLuma) / max(centerLuma, 0.08f));
#endif

    // ---------------------------------------------------------------------
    // BASE INVERSE TONE MAPPING (ITM) & SPECULAR ASSIST
    // ---------------------------------------------------------------------
#if ENABLE_ITM == 1
    color = ITM_SceneLinear(color, FX_ITM_HDR_PEAK);

#if ENABLE_LCE == 1
    float maxC = max(color.r, max(color.g, color.b));
    float minC = min(color.r, min(color.g, color.b));
    float saturation = (maxC - minC) / max(maxC, 1e-4f);

    float neutralWeight = 1.0f - smoothstep(0.05f, 0.20f, saturation);
    float brightWeight = smoothstep(0.72f, 0.94f, centerLuma);
    float detailWeight = smoothstep(0.10f, 0.45f, localContrast);
    float localWhiteMask = brightWeight * detailWeight * neutralWeight;

    color *= lerp(1.0f, 1.20f, localWhiteMask);
#endif

    // Highlight Roll-off (Dynamic Headroom Limiter)
    float highlightLuma = max(GetLuma(color), 0.0f);
    float highlightExcess = max(highlightLuma - 1.0f, 0.0f);
    float detailProtection = smoothstep(0.06f, 0.30f, localContrast);
    float shoulderKnee = lerp(5.0f, 10.0f, detailProtection);
    float rolledLuma = 1.0f + (highlightExcess / (1.0f + highlightExcess / shoulderKnee));
    float rolloffScale = lerp(1.0f, rolledLuma / max(highlightLuma, 1e-4f), step(1.0f, highlightLuma));

    color *= rolloffScale;
#endif

    // ---------------------------------------------------------------------
    // LOCAL CONTRAST ENHANCEMENT (LCE) & MICRO-AO
    // ---------------------------------------------------------------------
#if ENABLE_LCE == 1
    float currentLuma = GetLuma(color);
    float lumaDetail = clamp(centerLuma - avgLuma, -FX_LCE_MAX_LIMIT, FX_LCE_MAX_LIMIT);
    // Soften contrast amplification on hot specular lobes to prevent razor edges on skin
    float specDamp = 1.0f - smoothstep(0.80f, 1.10f, currentLuma) * 0.75f;
    float lceMask = smoothstep(FX_LCE_SHADOW_THRESHOLD, FX_LCE_SHADOW_THRESHOLD * 2.0f, currentLuma) * specDamp;
    float newLuma = max(currentLuma + (lumaDetail * FX_LCE_STRENGTH * lceMask), 1e-5f);

    color *= newLuma / max(currentLuma, 1e-5f);

#if ENABLE_MICRO_AO == 1
    float cavity = saturate((avgLuma - centerLuma) * 3.0f);
    float microAO = 1.0f - (cavity * cavity * FX_MICRO_AO_STRENGTH * lceMask);
    color *= microAO;
#endif
#endif

    // ---------------------------------------------------------------------
    // PROCEDURAL ENVIRONMENTAL PROBE (IBL)
    // ---------------------------------------------------------------------
#if (ENABLE_ENV_PROBE == 1) && (ENABLE_LCE == 1)
    float dX = ((n0 + n2) - (n1 + n3)) * 0.7071f + (n4 - n5);
    float dY = ((n0 + n3) - (n1 + n2)) * 0.7071f + (n6 - n7);

    float3 screenNormal = normalize(float3(-dX * FX_ENV_BUMP_SCALE, -dY * FX_ENV_BUMP_SCALE, 1.0f));

    // World transformation (Spellforce 2 camera pitch angle ~50 degrees)
    static const float cosCam = 0.6428f;
    static const float sinCam = 0.7660f;

    float3 worldNormal;
    worldNormal.x = screenNormal.x;
    worldNormal.y = screenNormal.y * cosCam + screenNormal.z * sinCam;
    worldNormal.z = -screenNormal.y * sinCam + screenNormal.z * cosCam;
    worldNormal = normalize(worldNormal);

    float upWeight = saturate(worldNormal.y);
    float downWeight = saturate(-worldNormal.y);
    float horizonWeight = saturate(1.0f - abs(worldNormal.y));

    float3 envIrradiance = (FX_ENV_SKY_COLOR * upWeight) +
                           (FX_ENV_GROUND_COLOR * downWeight) +
                           (FX_ENV_HORIZON_COLOR * horizonWeight * 0.5f);

    float skyOcclusion = saturate(avgLuma * 2.0f);
    float3 ambientLighting = envIrradiance * FX_ENV_AMBIENT_INTENSITY * skyOcclusion;
    color += color * ambientLighting;

    // Gate specular sheen strictly to neutral metallic glints.
    // Warm skin tones and colored cloth are rejected so they remain natural and matte.
    float maxC_env = max(color.r, max(color.g, color.b));
    float minC_env = min(color.r, min(color.g, color.b));
    float sat_env = (maxC_env - minC_env) / max(maxC_env, 1e-4f);
    float isMetallicGlint = 1.0f - smoothstep(0.15f, 0.45f, sat_env);

    float viewAngle = saturate(screenNormal.z);
    float fresnel = pow(1.0f - viewAngle, 4.0f);
    float3 envSpecular = lerp(FX_ENV_HORIZON_COLOR, FX_ENV_SKY_COLOR, upWeight);
    float specularMask = localContrast * smoothstep(0.85f, 0.98f, centerLuma) * isMetallicGlint;

    color += envSpecular * (fresnel + 0.20f) * specularMask * FX_ENV_SPECULAR_INTENSITY;
#endif

    // ---------------------------------------------------------------------
    // SCREEN-Y PSEUDO-DEPTH HEURISTICS (Atmospheric Perspective)
    // ---------------------------------------------------------------------
#if ENABLE_SCREEN_Y_DEPTH == 1
    float distFar = saturate(1.0f - uv.y);
    color = lerp(color, color * FX_AERIAL_HAZE_COLOR + 0.025f, distFar * FX_AERIAL_HAZE_STRENGTH);
#endif

    // ---------------------------------------------------------------------
    // EXPOSURE CALCULATION (Auto-Exposure Grid vs Manual Constant)
    // ---------------------------------------------------------------------
#if ENABLE_AUTO_EXPOSURE == 1
    float gridLuma = 0.0f;
    for (int gy = 0; gy < 4; gy++)
    {
        for (int gx = 0; gx < 4; gx++)
        {
            float2 tapUV = float2(0.2f + 0.2f * (float)gx, 0.2f + 0.2f * (float)gy);
            gridLuma += GetFastLinearLuma(_TexTop(SceneImage, tapUV).rgb);
        }
    }
    gridLuma *= (1.0f / 16.0f);

    float autoExp = FX_AUTO_EXP_TARGET / max(gridLuma, 0.03f);
    autoExp = clamp(autoExp, FX_AUTO_EXP_MIN, FX_AUTO_EXP_MAX);
    color *= autoExp;
#else
    color *= FX_EXPOSURE;
#endif

    // ---------------------------------------------------------------------
    // CINEMATIC GRADING (ASC-CDL) IN ACESCCT LOG SPACE
    // ---------------------------------------------------------------------
#if ENABLE_ASC_CDL == 1
#if ENABLE_LOG_GRADING == 1
    color = LinearToLog(color);
#endif

    color = pow(max(color * FX_CDL_SLOPE + FX_CDL_OFFSET, 0.0f), FX_CDL_POWER);

#if ENABLE_LOG_GRADING == 1
    color = LogToLinear(color);
#endif
#endif

    // ---------------------------------------------------------------------
    // SMART VIBRANCE & PURKINJE EFFECT
    // ---------------------------------------------------------------------
    float postLuma = GetLuma(color);

#if ENABLE_SMART_VIBRANCE == 1
    float v_max = max(color.r, max(color.g, color.b));
    float v_min = min(color.r, min(color.g, color.b));
    float v_sat = max(v_max - v_min, 0.0f) / max(v_max, 1e-5f);

    color = lerp(color, float3(postLuma, postLuma, postLuma), -FX_VIBRANCE_AMOUNT * (1.0f - v_sat));
    color = max(color, 0.0f);
#endif

#if ENABLE_PURKINJE == 1
    float nightMask = smoothstep(FX_PURKINJE_DARK_LEVEL, 0.0f, postLuma);
    float3 nightCol = dot(color, float3(0.0f, 0.5f, 0.5f)) * float3(0.2f, 0.4f, 0.8f);

    color = lerp(color, nightCol, nightMask * FX_PURKINJE_STRENGTH);
#endif

    // ---------------------------------------------------------------------
    // COLOR VOLUMES & GAMUT PRESERVATION
    // ---------------------------------------------------------------------
#if ENABLE_GAMUT_COMPRESSION == 1
    color = CompressGamut(color, FX_GAMUT_THRESHOLD, FX_GAMUT_STRENGTH);
#endif

#if FX_WIDE_GAMUT_EMULATION == 1
    static const float3x3 sRGB_to_ACEScg = float3x3(
        0.613097f, 0.339523f, 0.047379f,
        0.070194f, 0.916354f, 0.013452f,
        0.020619f, 0.109570f, 0.869811f
    );
    static const float3x3 ACEScg_to_sRGB = float3x3(
        1.705051f, -0.621866f, -0.083185f,
       -0.130256f,  1.140803f, -0.010547f,
       -0.024007f, -0.128983f,  1.152990f
    );

    float3 acesC    = mul(sRGB_to_ACEScg, color);
    float  acesLuma = GetLumaAces(acesC);
    float  w_max    = max(acesC.r, max(acesC.g, acesC.b));
    float  w_min    = min(acesC.r, min(acesC.g, acesC.b));
    float  satWide  = max((w_max - w_min) / max(w_max, 1e-5f), 0.0f);

    acesC = max(acesLuma + (acesC - acesLuma) * (1.0f + FX_GAMUT_EXPANSION * smoothstep(0.2f, 0.9f, satWide)), 0.0f);
    color = max(mul(ACEScg_to_sRGB, acesC), 0.0f);
#endif

    // ---------------------------------------------------------------------
    // RATIO-PRESERVING 3D LOOKUP TABLE (LUT)
    // ---------------------------------------------------------------------
#if ENABLE_LUT == 1
#if FX_LUT_PRE_EXPOSURE == 1
    float effectiveExp = FX_EXPOSURE;
#if ENABLE_AUTO_EXPOSURE == 1
    effectiveExp = autoExp;
#endif
    float3 lutSrc = color * (1.0f / max(effectiveExp, 1e-4f));
#else
    float3 lutSrc = color;
#endif

    float3 lutInputSDR = saturate(LinearTosRGB(lutSrc));
    const float LUT_S = (LUT_SIZE - 1.0f) / LUT_SIZE;
    const float LUT_O = 0.5f / LUT_SIZE;

    float2 tcBase = (lutInputSDR.rg * LUT_S) + LUT_O;
    float b = lutInputSDR.b * LUT_S * LUT_SIZE;
    float slice1 = floor(b);
    float slice2 = min(slice1 + 1.0f, LUT_SIZE - 1.0f);
    float f = frac(b);

    float2 tc1 = float2((tcBase.x + slice1) * (1.0f / LUT_SIZE), tcBase.y);
    float2 tc2 = float2((tcBase.x + slice2) * (1.0f / LUT_SIZE), tcBase.y);

    float3 lut1 = _TexTop(LUTImage, tc1).rgb;
    float3 lut2 = _TexTop(LUTImage, tc2).rgb;

    float3 gradedSDR = sRGBToLinear(lerp(lut1, lut2, f));

#if FX_LUT_PRE_EXPOSURE == 1
    color = gradedSDR * max(lutSrc, 1.0f) * effectiveExp;
#else
    float hdrMask = saturate(1.0f - max(color.r, max(color.g, color.b)));
    color = lerp(color, gradedSDR, hdrMask);
#endif
#endif

    // Outputs pure physical Scene-Linear HDR floating point color
    return float4(FX_RT_CLAMP(color), 1.0f);
}

// AUXILIARY PIXEL SHADER (LUT Blend)
float4 PS_ColorLUTBlend(VS_OUTPUT input) : COLOR
{
    float3 sceneColor = _TexTop(SceneImage, input.TexCoord0.xy).rgb;
    float3 lutColor = _TexTop(LUTImage, input.TexCoord0.xy).rgb;
    float3 cOut = lerp(sceneColor, lutColor, Offset.w);

    return float4(FX_RT_CLAMP(cOut), 1.0f);
}

// ENGINE DISPATCH TECHNIQUES
technique ColorLUTBlend
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_ColorGrading();
        PixelShader = compile ps_3_0 PS_ColorLUTBlend();
    }
}

technique ColorGrading
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_ColorGrading();
        PixelShader = compile ps_3_0 PS_ColorGrading();
    }
}

#endif // WIN32
