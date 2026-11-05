//******************************************************************************************
// File:            pp_debug.fx
// Description:     Comprehensive Diagnostics, Calibration Patterns & Buffer Inspector
// Pipeline Status: Utility Output / Calibration Stage
// Architecture:    Direct3D 9 / Shader Model 3.0
//******************************************************************************************

#ifndef WIN32

// COMMON LIBRARIES & STRUCTURES
#include "fx_common.fxh"

// EXTERNAL ENGINE PARAMETERS
float4 Offset;

// TEXTURES & SAMPLERS
texture SceneTexture;
texture DebugTexture;
//texture Debug2Texture;

sampler2D SceneImage = sampler_state
{
    Texture = <SceneTexture>;
    MinFilter = Linear;
    MagFilter = Linear;
    MipFilter = None;
    AddressU = Clamp;
    AddressV = Clamp;
};

sampler2D DebugImage = sampler_state
{
    Texture = <DebugTexture>;
    MinFilter = Linear;
    MagFilter = Linear;
    MipFilter = None;
    AddressU = Clamp;
    AddressV = Clamp;
};

//sampler2D Debug2Image = sampler_state
//{
//    Texture = <Debug2Texture>;
//    MinFilter = Linear;
//    MagFilter = Linear;
//    MipFilter = None;
//    AddressU = Clamp;
//    AddressV = Clamp;
//};

// VERTEX SHADER
VS_OUTPUT VS_Debug(const VS_INPUT input)
{
    VS_OUTPUT o = (VS_OUTPUT) 0;

    o.Position = input.Pos;
    o.TexCoord0.xy = input.TexCoord0.xy;
    o.TexCoord0.zw = input.TexCoord0.xy * 4.0f; // 4x zoom for thumbnail quadrant inspection

    return o;
}

float4 PS_Debug(const VS_OUTPUT input) : COLOR
{
    float2 uv = input.TexCoord0.xy;
    float4 sceneTex = _TexTop(SceneImage, uv);
    float3 color = sceneTex.rgb;

#if FX_DEBUG_VIEW == 1
    // Visualizes Alpha Seed Bus (Noise Grain / Temporal Dither Phase)
    return float4(sceneTex.aaa, 1.0f);

#elif FX_DEBUG_VIEW == 2
    // Fullscreen Calibration Ramp: verifies monotonicity of the active tonemapper
    if (uv.y > 0.85f)
    {
        float rampLinear = uv.x * 10.0f; // 0 to 10x HDR linear light
        float gridTick   = step(0.04f, frac(uv.x * 10.0f)); // Marker at every 1.0x SDR step
        float3 mappedRamp = Tonemap_Lumina(float3(rampLinear, rampLinear, rampLinear)) * gridTick;
        return float4(LinearTosRGB(mappedRamp), 1.0f);
    }
    return float4(saturate(color), 1.0f);

#elif FX_DEBUG_VIEW == 3
    // False-Color IRE Exposure Zones
    return float4(Tonemap_FalseColor(color), 1.0f);

#elif FX_DEBUG_VIEW == 4
    // Out-of-bounds Detector (Magenta = Exceeds 1.0, Cyan = Negative / NaN)
    if (any(color > 1.0f)) return float4(1.0f, 0.0f, 1.0f, 1.0f);
    if (any(color < 0.0f)) return float4(0.0f, 1.0f, 1.0f, 1.0f);
    return float4(GetLuma(color).xxx * 0.35f, 1.0f);

#else
    // Default Engine Debug Picture-in-Picture Quadrant
    float3 cOutDbg = tex2D(DebugImage, input.TexCoord0.zw).rgb;
    if (input.TexCoord0.z < 1.0f && input.TexCoord0.w < 1.0f)
    {
        color = cOutDbg;
    }
    return float4(saturate(color), 1.0f);
#endif
}

// ENGINE DISPATCH TECHNIQUES
technique Debug
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_Debug();
        PixelShader = compile ps_3_0 PS_Debug();
    }
}

#endif // WIN32
