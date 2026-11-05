//******************************************************************************************
// File:         fx_common.fxh
// Description:  Common Boilerplate, Universal Clamping, Resolution Scaling & I/O
// Architecture: Direct3D 9 / Shader Model 3.0
//******************************************************************************************

#ifndef FX_COMMON_H
#define FX_COMMON_H

#include "fx_settings.fxh"
#include "fx_math.fxh"

// Universal intermediate render target clamp:
// In FP16 mode: allows HDR values > 1.0 to survive across passes without clipping.
// In 8-bit SDR mode: clamps to [0, 1] to prevent texture driver wrapping artifacts.
#if FX_ENABLE_FP16_PATCH == 1
#define FX_RT_CLAMP(c) max((c), 0.0f)
#else
#define FX_RT_CLAMP(c) saturate((c))
#endif

// Resolution scale relative to 1080p baseline (1.0 at 1080p, 2.0 at 4K)
// In Direct3D 9, Offset.y holds the vertical texel dimension (1.0 / ScreenHeight)
#define FX_RES_SCALE (1.0f / max(1080.0f * Offset.y, 1e-4f))

// Standard Vertex Shader I/O Structures matching engine quad buffer
struct VS_INPUT
{
    float4 Pos : POSITION;
    float4 TexCoord0 : TEXCOORD0;
};

struct VS_OUTPUT
{
    float4 Position : POSITION;
    float4 TexCoord0 : TEXCOORD0;
};

#endif // FX_COMMON_H
