//******************************************************************************************
// File:            pp_blur.fx
// Description:     Dual Kawase Blur & Jimenez-Karis Filter in Unclamped FP16
// Pipeline Status: Executed as a sub-process for pp_glow.fx to create bloom buffers
// Architecture:    Direct3D 9 / Shader Model 3.0
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
struct VS_OUTPUT_BLUR
{
    float4 Position : POSITION;
    float4 TexCoord[5] : TEXCOORD0;
};

// VERTEX SHADERS
VS_OUTPUT_BLUR VS_CopyLinear(const VS_INPUT input)
{
    VS_OUTPUT_BLUR o = (VS_OUTPUT_BLUR) 0;

    o.Position = input.Pos;
    o.TexCoord[0].xyzw = input.TexCoord0.xyzw;

    return o;
}

VS_OUTPUT_BLUR VS_DownSample(const VS_INPUT input)
{
    VS_OUTPUT_BLUR o = (VS_OUTPUT_BLUR) 0;

    o.Position = input.Pos;
    o.TexCoord[0].xyzw = input.TexCoord0.xyzw;

#if ENABLE_KARIS_DOWNSAMPLE == 0
    float2 scaledOffset = Offset.xy * FX_RES_SCALE;
    o.TexCoord[1].xy = input.TexCoord0.xy + (float2(-1.0f, -1.0f) * scaledOffset);
    o.TexCoord[1].zw = input.TexCoord0.xy + (float2( 1.0f, -1.0f) * scaledOffset);
    o.TexCoord[2].xy = input.TexCoord0.xy + (float2(-1.0f,  1.0f) * scaledOffset);
    o.TexCoord[2].zw = input.TexCoord0.xy + (float2( 1.0f,  1.0f) * scaledOffset);
#endif

    return o;
}

// DUAL KAWASE DOWNSAMPLE (5-Tap Bilinear)
VS_OUTPUT_BLUR VS_DualKawase_Down(const VS_INPUT input, const uniform float radius)
{
    VS_OUTPUT_BLUR o = (VS_OUTPUT_BLUR) 0;

    o.Position = input.Pos;
    float2 off = Offset.xy * (radius * FX_RES_SCALE);

    o.TexCoord[0].xyzw = input.TexCoord0.xyzw;

    o.TexCoord[1].xy = input.TexCoord0.xy + float2(-off.x, -off.y);
    o.TexCoord[1].zw = input.TexCoord0.xy + float2(off.x, -off.y);
    o.TexCoord[2].xy = input.TexCoord0.xy + float2(-off.x, off.y);
    o.TexCoord[2].zw = input.TexCoord0.xy + float2(off.x, off.y);

    return o;
}

// DUAL KAWASE UPSAMPLE (8-Tap Bilinear)
VS_OUTPUT_BLUR VS_DualKawase_Up(const VS_INPUT input, const uniform float radius)
{
    VS_OUTPUT_BLUR o = (VS_OUTPUT_BLUR) 0;

    o.Position = input.Pos;
    float2 off = Offset.xy * (radius * FX_RES_SCALE);

    o.TexCoord[0].xyzw = input.TexCoord0.xyzw;

    // 4 Orthogonal Taps (Weight 1/12)
    o.TexCoord[1].xy = input.TexCoord0.xy + float2(2.0f * off.x, 0.0f);
    o.TexCoord[1].zw = input.TexCoord0.xy + float2(-2.0f * off.x, 0.0f);
    o.TexCoord[2].xy = input.TexCoord0.xy + float2(0.0f, 2.0f * off.y);
    o.TexCoord[2].zw = input.TexCoord0.xy + float2(0.0f, -2.0f * off.y);

    // 4 Diagonal Taps (Weight 2/12)
    o.TexCoord[3].xy = input.TexCoord0.xy + float2(off.x, off.y);
    o.TexCoord[3].zw = input.TexCoord0.xy + float2(-off.x, off.y);
    o.TexCoord[4].xy = input.TexCoord0.xy + float2(off.x, -off.y);
    o.TexCoord[4].zw = input.TexCoord0.xy + float2(-off.x, -off.y);

    return o;
}

VS_OUTPUT_BLUR VS_BlurDesaturate(const VS_INPUT input)
{
    VS_OUTPUT_BLUR o = (VS_OUTPUT_BLUR) 0;

    o.Position = input.Pos;
    o.TexCoord[0].xyzw = input.TexCoord0.xyzw;

    return o;
}

// PIXEL SHADERS
float4 PS_CopyLinear(const VS_OUTPUT_BLUR input) : COLOR
{
    float3 cOut = _TexTop(SceneImage, input.TexCoord[0].xy).rgb;

    return float4(FX_RT_CLAMP(cOut), 1.0f);
}

float4 PS_DownSample(const VS_OUTPUT_BLUR input) : COLOR
{
    float2 uv = input.TexCoord[0].xy;
    float2 off = Offset.xy * FX_RES_SCALE;

#if ENABLE_KARIS_DOWNSAMPLE == 1
    // 13-TAP JIMENEZ / KARIS FILTER WITH ANTI-FLICKER LUMA WEIGHTING
    // Pattern layout:
    // a . b . c
    // . d . e .
    // f . g . h
    // . i . j .
    // k . l . m

    float3 a = _TexTop(SceneImage, uv + float2(-2.0f*off.x, -2.0f*off.y)).rgb;
    float3 b = _TexTop(SceneImage, uv + float2( 0.0f,       -2.0f*off.y)).rgb;
    float3 c = _TexTop(SceneImage, uv + float2( 2.0f*off.x, -2.0f*off.y)).rgb;

    float3 d = _TexTop(SceneImage, uv + float2(-off.x,      -off.y)).rgb;
    float3 e = _TexTop(SceneImage, uv + float2( off.x,      -off.y)).rgb;

    float3 f = _TexTop(SceneImage, uv + float2(-2.0f*off.x,  0.0f)).rgb;
    float3 g = _TexTop(SceneImage, uv).rgb; // Center
    float3 h = _TexTop(SceneImage, uv + float2( 2.0f*off.x,  0.0f)).rgb;

    float3 i = _TexTop(SceneImage, uv + float2(-off.x,       off.y)).rgb;
    float3 j = _TexTop(SceneImage, uv + float2( off.x,       off.y)).rgb;

    float3 k = _TexTop(SceneImage, uv + float2(-2.0f*off.x,  2.0f*off.y)).rgb;
    float3 l = _TexTop(SceneImage, uv + float2( 0.0f,        2.0f*off.y)).rgb;
    float3 m = _TexTop(SceneImage, uv + float2( 2.0f*off.x,  2.0f*off.y)).rgb;

    // 5 Overlapping 2x2 bilinear boxes
    float3 g0 = (a + b + f + g) * 0.25f; // Top-Left Box
    float3 g1 = (b + c + g + h) * 0.25f; // Top-Right Box
    float3 g2 = (f + g + k + l) * 0.25f; // Bottom-Left Box
    float3 g3 = (g + h + l + m) * 0.25f; // Bottom-Right Box
    float3 g4 = (d + e + i + j) * 0.25f; // Center Box

    // Karis luma weighting: Suppresses fireflies and specular flickering in bloom
    float w0 = 0.125f / (1.0f + GetLuma(g0));
    float w1 = 0.125f / (1.0f + GetLuma(g1));
    float w2 = 0.125f / (1.0f + GetLuma(g2));
    float w3 = 0.125f / (1.0f + GetLuma(g3));
    float w4 = 0.500f / (1.0f + GetLuma(g4));

    float3 cOut = (g0 * w0 + g1 * w1 + g2 * w2 + g3 * w3 + g4 * w4) / (w0 + w1 + w2 + w3 + w4);
#else
    // Standard 4-tap Downsample
    float3 cOut = _TexTop( SceneImage, input.TexCoord[1].xy ).rgb;
    cOut += _TexTop( SceneImage, input.TexCoord[1].zw ).rgb;
    cOut += _TexTop( SceneImage, input.TexCoord[2].xy ).rgb;
    cOut += _TexTop( SceneImage, input.TexCoord[2].zw ).rgb;
    cOut *= 0.25f;
#endif

    return float4(FX_RT_CLAMP(cOut), 1.0f);
}

float4 PS_DualKawase_Down(const VS_OUTPUT_BLUR input) : COLOR
{
    // Marius Bjorge 5-Tap Downsample (Weighting: Center 4/8, Diagonals 1/8 each)
    float3 sum = _TexTop(SceneImage, input.TexCoord[0].xy).rgb * 4.0f;

    sum += _TexTop(SceneImage, input.TexCoord[1].xy).rgb;
    sum += _TexTop(SceneImage, input.TexCoord[1].zw).rgb;
    sum += _TexTop(SceneImage, input.TexCoord[2].xy).rgb;
    sum += _TexTop(SceneImage, input.TexCoord[2].zw).rgb;

    return float4(FX_RT_CLAMP(sum * 0.125f), 1.0f);
}

float4 PS_DualKawase_Up(const VS_OUTPUT_BLUR input) : COLOR
{
    float3 sum = 0.0f;

    // 4 Orthogonal Taps (Weight 1/12)
    sum += _TexTop(SceneImage, input.TexCoord[1].xy).rgb;
    sum += _TexTop(SceneImage, input.TexCoord[1].zw).rgb;
    sum += _TexTop(SceneImage, input.TexCoord[2].xy).rgb;
    sum += _TexTop(SceneImage, input.TexCoord[2].zw).rgb;

    // 4 Diagonal Taps (Weight 2/12)
    sum += _TexTop(SceneImage, input.TexCoord[3].xy).rgb * 2.0f;
    sum += _TexTop(SceneImage, input.TexCoord[3].zw).rgb * 2.0f;
    sum += _TexTop(SceneImage, input.TexCoord[4].xy).rgb * 2.0f;
    sum += _TexTop(SceneImage, input.TexCoord[4].zw).rgb * 2.0f;

    return float4(FX_RT_CLAMP(sum * 0.083333333f), 1.0f);
}

float4 PS_BlurDesaturate(const VS_OUTPUT_BLUR input) : COLOR
{
    float3 cOut = _TexTop(SceneImage, input.TexCoord[0].xy).rgb;
    float lum = GetLuma(cOut);

    cOut = lerp(cOut, float3(lum, lum, lum), FX_BLUR_DESATURATE_INTENSITY);

    return float4(FX_RT_CLAMP(cOut), 1.0f);
}

// ENGINE TECHNIQUES
technique CopyLinear
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_CopyLinear();
        PixelShader = compile ps_3_0 PS_CopyLinear();
    }
}

technique DownSample
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_DownSample();
        PixelShader = compile ps_3_0 PS_DownSample();
    }
}

// Mapped engine H/V calls to Dual Kawase Downsample/Upsample cycles
technique Blur9H
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_DualKawase_Down(1.0f);
        PixelShader = compile ps_3_0 PS_DualKawase_Down();
    }
}

technique Blur9V
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_DualKawase_Up(1.0f);
        PixelShader = compile ps_3_0 PS_DualKawase_Up();
    }
}

technique Blur13H
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_DualKawase_Down(2.0f);
        PixelShader = compile ps_3_0 PS_DualKawase_Down();
    }
}

technique Blur13V
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_DualKawase_Up(2.0f);
        PixelShader = compile ps_3_0 PS_DualKawase_Up();
    }
}

technique BlurDesaturate
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_BlurDesaturate();
        PixelShader = compile ps_3_0 PS_BlurDesaturate();
    }
}

#endif // WIN32
