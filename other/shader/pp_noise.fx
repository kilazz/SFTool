//******************************************************************************************
// File:            pp_noise.fx
// Description:     Cinematic Lens Distortion & Alpha Seed Bus Preservation
// Pipeline Status: Executed AFTER ColorGrading (Pass-through for Scene-Linear HDR)
//******************************************************************************************

#ifndef WIN32

// COMMON LIBRARIES & STRUCTURES
#include "fx_common.fxh"

// EXTERNAL ENGINE PARAMETERS
float4 Offset;

// TEXTURES & SAMPLERS
texture SceneTexture;
texture NoiseTexture;

sampler2D SceneImage = sampler_state
{
    Texture = <SceneTexture>;
    MinFilter = Linear;
    MagFilter = Linear;
    MipFilter = None;
    AddressU = Clamp;
    AddressV = Clamp;
};

sampler2D NoiseImage = sampler_state
{
    Texture = <NoiseTexture>;
    MinFilter = Point; // Point filtering is essential for sharp, non-blurry grain definition
    MagFilter = Point;
    MipFilter = None;
    AddressU = Wrap; // Seamless repeating pattern across the entire viewport
    AddressV = Wrap;
};

// VERTEX SHADER
// В VS_Noise замени o.TexCoord0.zw на:
VS_OUTPUT VS_Noise(const VS_INPUT input)
{
    VS_OUTPUT o = (VS_OUTPUT) 0;
    o.Position = input.Pos;
    o.TexCoord0.xy = input.TexCoord0.xy;

    o.TexCoord0.zw = input.TexCoord0.xy * 5.0f + float2(Offset.x, Offset.x * 0.5f);

    return o;
}

// PIXEL SHADER
float4 PS_Noise(VS_OUTPUT input) : COLOR
{
    float2 uv = input.TexCoord0.xy;

    // LENS DISTORTION (Spherize / Barrel Curvature)
#if ENABLE_LENS_DISTORTION == 1
    float2 centerCoord = uv - 0.5f;
    float  r2          = dot(centerCoord, centerCoord);
    uv                 = uv + centerCoord * (FX_LENS_DISTORTION * r2);
#endif

    // Pass-through of pure Scene-Linear HDR color.
    float4 cOut = _TexTop(SceneImage, uv);

    // ALPHA SEED BUS: the upstream passes carry no grain seed (ColorGrading writes alpha = 1.0),
    // so the animated noise texture is sampled here and published in alpha. Sharpen/Glow forward
    // it untouched and the final pass uses it for film grain and FRC dither phase.
    // Noise coordinates deliberately ignore lens distortion (grain is a sensor effect).
    cOut.a = _TexTop(NoiseImage, input.TexCoord0.zw).FX_NOISE_SEED_CHANNEL;

    return cOut;
}

// ENGINE DISPATCH TECHNIQUES
technique Noise
{
    pass p0
    {
        VertexShader = compile vs_3_0 VS_Noise();
        PixelShader = compile ps_3_0 PS_Noise();
    }
}

#endif // WIN32
