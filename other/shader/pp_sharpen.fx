//******************************************************************************************
// File:            pp_sharpen.fx
// Description:     AMD FidelityFX RCAS with Perceptual Luma Weighing for HDR
// Pipeline Status: Executed AFTER Noise and BEFORE Bloom
//******************************************************************************************

#ifndef WIN32

// COMMON LIBRARIES & STRUCTURES
#include "fx_common.fxh"

// EXTERNAL ENGINE PARAMETERS
float4 Offset;

// TEXTURES & SAMPLERS
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
struct VS_OUTPUT_SHARPEN
{
    float4 Position : POSITION;
    float4 TexCoord[3] : TEXCOORD0;
};

// VERTEX SHADER
VS_OUTPUT_SHARPEN VS_Sharpen(const VS_INPUT input)
{
    VS_OUTPUT_SHARPEN o = (VS_OUTPUT_SHARPEN) 0;

    o.Position = input.Pos;

    // Sample Point [0]: Center Pixel (e)
    o.TexCoord[0].xy = input.TexCoord0.xy;

    // Sample Points [1]: Vertical and Horizontal Neighbors (b, d)
    o.TexCoord[1].xy = input.TexCoord0.xy + float2(0.0f, -Offset.y);
    o.TexCoord[1].zw = input.TexCoord0.xy + float2(-Offset.x, 0.0f);

    // Sample Points [2]: Right (f) and Bottom (h)
    o.TexCoord[2].xy = input.TexCoord0.xy + float2(Offset.x, 0.0f);
    o.TexCoord[2].zw = input.TexCoord0.xy + float2(0.0f, Offset.y);

    return o;
}

// RCAS HDR RANGE COMPRESSION
// RCAS lobe limits (hitMin / hitMax) assume values in [0, 1]. On scene-linear HDR data any neighbor
// channel above 1.0 flips hitMax positive and zeroes the lobe, silently disabling sharpening over
// every bright area. The kernel is therefore run on x / (1 + x) and expanded afterwards.
#if FX_ENABLE_FP16_PATCH == 1
#define RcasCompress(x) ((x) / (1.0f + (x)))
#define RcasExpand(y)   ((y) / max(1.0f - (y), 1e-3f))
#else
#define RcasCompress(x) (x)
#define RcasExpand(y)   (y)
#endif

// PIXEL SHADER
float4 PS_Sharpen(VS_OUTPUT_SHARPEN input) : COLOR
{
    // Read the center pixel and preserve the film grain seed from the Alpha channel
    float4 e_tex = _TexTop(SceneImage, input.TexCoord[0].xy);
    float3 e = RcasCompress(max(e_tex.rgb, 0.0f));
    float noiseAlpha = e_tex.a;

    // Sample cross-neighborhood taps (compressed into the [0, 1) range RCAS expects)
    float3 b = RcasCompress(max(_TexTop(SceneImage, input.TexCoord[1].xy).rgb, 0.0f));
    float3 d = RcasCompress(max(_TexTop(SceneImage, input.TexCoord[1].zw).rgb, 0.0f));
    float3 f = RcasCompress(max(_TexTop(SceneImage, input.TexCoord[2].xy).rgb, 0.0f));
    float3 h = RcasCompress(max(_TexTop(SceneImage, input.TexCoord[2].zw).rgb, 0.0f));

    // Evaluated in perceptual space (sqrt) to prevent dark ringing halos around intense HDR speculars
    float bL = sqrt(GetRCASLuma(b));
    float dL = sqrt(GetRCASLuma(d));
    float eL = sqrt(GetRCASLuma(e));
    float fL = sqrt(GetRCASLuma(f));
    float hL = sqrt(GetRCASLuma(h));

    // RCAS NOISE SUPPRESSION PASS
    // Reduces sharpening over flat surfaces and low-contrast regions to prevent noise amplification
    float nz = 1.0f;
#if FX_RCAS_DENOISE == 1
    float avgLuma = (bL + dL + fL + hL) * 0.25f;
    float range = max(max(max(bL, dL), max(hL, fL)), eL) -
                  min(min(min(bL, dL), min(eL, fL)), hL);

    nz = saturate(abs(avgLuma - eL) / max(range, 1e-5f));
    nz = -0.5f * nz + 1.0f;
#endif

    // CALCULATE CONTRAST ADAPTIVE LOBE
    // Adjusts the sharpening kernel weight dynamically based on local high-frequency contrast
    float3 minRGB = min(min(b, d), min(f, h));
    float3 maxRGB = max(max(b, d), max(f, h));

    // Analytical peak coefficients for RCAS limit boundaries
    float2 peakC = float2(1.0f, -4.0f);

    float3 hitMin = minRGB / max(4.0f * maxRGB, 1e-5f);
    float3 hitMax = (peakC.xxx - maxRGB) / min(4.0f * minRGB + peakC.yyy, -1e-5f);

    float3 lobeRGB = max(-hitMin, hitMax);

    // Convert RGB lobe vector to a scalar limit and clamp to eliminate ringing artifacts
    float lobe = max(-FX_RCAS_LIMIT, min(max(lobeRGB.r, max(lobeRGB.g, lobeRGB.b)), 0.0f)) * FX_RCAS_SHARPNESS;

#if FX_RCAS_DENOISE == 1
    lobe *= nz;
#endif

    // FINAL ADAPTIVE COLOR RECONSTRUCTION
    // Blends the weighted cross-neighborhood with the center pixel
    float3 sharpened = ((b + d + f + h) * lobe + e) / (4.0f * lobe + 1.0f);

    // Back to scene-linear. The compressed result is clamped just below 1.0 to keep the inverse finite.
#if FX_ENABLE_FP16_PATCH == 1
    sharpened = clamp(sharpened, 0.0f, 0.999f);
#endif
    sharpened = RcasExpand(sharpened);

    // FP16: No hard clamp at 1.0, protecting intense speculars from turning dull!
    return float4(FX_RT_CLAMP(sharpened), noiseAlpha);
}

// ENGINE DISPATCH TECHNIQUES
technique Sharpen
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_Sharpen();
        PixelShader = compile ps_3_0 PS_Sharpen();
    }
}

#endif // WIN32
