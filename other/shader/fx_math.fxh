//******************************************************************************************
// File:         fx_math.fxh
// Description:  Global Math Library, Color Science & Tonemapping Operators
// Architecture: Direct3D 9 / Shader Model 3.0
//******************************************************************************************

#ifndef FX_MATH_H
#define FX_MATH_H

#include "fx_settings.fxh"

// MACROS & UTILITIES
#define _TexTop(t, p) tex2Dlod(t, float4(p, 0.0f, 0.0f))

// =========================================================================================
// LUMINANCE & COLOR SPACES
// =========================================================================================

// Standard Rec.709 Luminance coefficients (ITU-R BT.709-6)
float GetLuma(float3 color)
{
    return dot(color, float3(0.212639f, 0.715169f, 0.072192f));
}

// Fast Gamma-2.0 Linear Luma approximation for high-tap spatial filters (LCE)
float GetFastLinearLuma(float3 srgb)
{
    float3 lin = srgb * srgb;
    return dot(lin, float3(0.212639f, 0.715169f, 0.072192f));
}

// ACEScg Luminance coefficients
float GetLumaAces(float3 color)
{
    return dot(color, float3(0.272229f, 0.674082f, 0.053689f));
}

// Optimized Luma weights specifically tuned for RCAS edge detection
float GetRCASLuma(float3 rgb)
{
    return dot(rgb, float3(0.5f, 1.0f, 0.5f));
}

// Exact Piecewise sRGB to Linear conversion (IEC 61966-2-1)
float3 sRGBToLinear(float3 c)
{
    float3 linearCol = pow(abs((c + 0.055f) * (1.0f / 1.055f)), 2.4f);
    float3 srgbCol = c * (1.0f / 12.92f); // HLSL will fold this to exact IEEE 754 precision

    return lerp(srgbCol, linearCol, step(0.04045f, c));
}

// Display-referred EOTF conversion supporting Pure 2.2 and Piecewise sRGB
float3 LinearTosRGB(float3 c)
{
#if FX_OUTPUT_EOTF == 0
    // Pure Gamma 2.2: preserves deep, unlifted blacks on consumer monitors
    return pow(max(c, 0.0f), 1.0f / 2.2f);
#else
    // Official IEC Piecewise sRGB
    float3 linearCol = (pow(abs(c), 1.0f / 2.4f) * 1.055f) - 0.055f;
    float3 srgbCol   = c * 12.92f;
    return lerp(srgbCol, linearCol, step(0.0031308f, c));
#endif
}

// Display-referred black floor lifting (Anti-Fog / De-Hazing)
float3 ApplySoftTrueBlack(float3 color, float strength)
{
    float minComp = min(color.r, min(color.g, color.b));
    float shadowOffset = (minComp < 0.04f) ? (minComp - 12.5f * minComp * minComp) : 0.02f;
    float normFactor = 1.0f / max(1.0f - 0.02f * strength, 1e-5f);

    return max(0.0f, (color - shadowOffset * strength) * normFactor);
}

// =========================================================================================
// LOG SPACE
// =========================================================================================

// Linear to ACEScct Log transform (S-2016-001)
//   toe:  A * x + B,  A = 10.5402377, B = 0.0729055, valid for x <= 0.0078125
//   log:  (log2(x) + 9.72) / 17.52
float3 LinearToLog(float3 x)
{
    x = max(x, 0.0f);
    float3 isToe = step(x, 0.0078125f);
    float3 toe = 10.5402377f * x + 0.0729055f;
    float3 logPrt = (log2(max(x, 1e-10f)) + 9.72f) * (1.0f / 17.52f);

    return lerp(logPrt, toe, isToe);
}

// ACEScct Log back to Linear transform
float3 LogToLinear(float3 x)
{
    float3 isToe = step(x, 0.1552511f);
    float3 toe = (x - 0.0729055f) * (1.0f / 10.5402377f);
    float3 expPrt = exp2(x * 17.52f - 9.72f);

    return lerp(expPrt, toe, isToe);
}

// =========================================================================================
// PERCEPTUAL QUANTIZER (PQ ST 2084) & ICTCP
// =========================================================================================

static const float PQ_N = (2610.0f / 4096.0f / 4.0f);
static const float PQ_M = (2523.0f / 4096.0f * 128.0f);
static const float PQ_C1 = (3424.0f / 4096.0f);
static const float PQ_C2 = (2413.0f / 4096.0f * 32.0f);
static const float PQ_C3 = (2392.0f / 4096.0f * 32.0f);
static const float PQ_MAX_VAL = 100.0f; // Reference SDR White in Nits (1.0 = 100 nits)

float NitToPQ(float nits)
{
    float normalized = max(nits / 10000.0f, 0.0f);
    float colToPow = pow(normalized, PQ_N);

    return pow(abs((PQ_C1 + PQ_C2 * colToPow) / (1.0f + PQ_C3 * colToPow)), PQ_M);
}

// Fast scalar PQ functions (saves 16 pow instructions vs vector PQ)
float linearToPQ_Scalar(float linearVal)
{
    float col = max(linearVal / PQ_MAX_VAL, 0.0f);
    float colToPow = pow(col, PQ_N);

    return pow(abs((PQ_C1 + PQ_C2 * colToPow) / (1.0f + PQ_C3 * colToPow)), PQ_M);
}

float PQtoLinear_Scalar(float pqVal)
{
    float colToPow = pow(abs(pqVal), 1.0f / PQ_M);
    float num = max(colToPow - PQ_C1, 0.0f);
    float den = max(PQ_C2 - (PQ_C3 * colToPow), 1e-5f);

    return pow(abs(num / den), 1.0f / PQ_N) * PQ_MAX_VAL;
}

float3 linearToPQ_Vec(float3 linearVal)
{
    float3 col = max(linearVal / PQ_MAX_VAL, 0.0f);
    float3 colToPow = pow(col, PQ_N);

    return pow(abs((PQ_C1 + PQ_C2 * colToPow) / (1.0f + PQ_C3 * colToPow)), PQ_M);
}

float3 PQtoLinear_Vec(float3 pqVal)
{
    float3 colToPow = pow(abs(pqVal), 1.0f / PQ_M);
    float3 num = max(colToPow - PQ_C1, 0.0f);
    float3 den = max(PQ_C2 - (PQ_C3 * colToPow), 1e-5f);

    return pow(abs(num / den), 1.0f / PQ_N) * PQ_MAX_VAL;
}

// STRICT REC.709 -> ICTCP TRANSFORMATION
// Exact analytical composition: M(Rec709->Rec2020) * M(Rec2020->LMS)
float3 RGBToICtCp(float3 col)
{
    static const float3x3 matRec709toLMS = float3x3(
        0.295809f, 0.623101f, 0.081089f,
        0.156249f, 0.727326f, 0.116425f,
        0.035136f, 0.156575f, 0.808290f
    );

    float3 lms = mul(matRec709toLMS, max(0.0f, col));
    float3 pqLms = linearToPQ_Vec(lms);

    // Official ITU-R BT.2100-2 Table 4 coefficients
    static const float3x3 matLMStoICtCp = float3x3(
         0.500000f, 0.500000f, 0.000000f,
         1.613770f, -3.323486f, 1.709717f,
         4.378174f, -4.245605f, -0.132568f
    );

    return mul(matLMStoICtCp, pqLms);
}

// STRICT ICTCP -> REC.709 TRANSFORMATION
// Exact inverse: M(ICtCp->LMS) * M(LMS->Rec709)
float3 ICtCpToRGB(float3 col)
{
    // Official ITU-R BT.2100-2 Table 4 inverse coefficients
    static const float3x3 matICtCptoLMS = float3x3(
         1.0f, 0.008609f, 0.111030f,
         1.0f, -0.008609f, -0.111030f,
         1.0f, 0.560031f, -0.320627f
    );

    float3 pqLms = mul(matICtCptoLMS, col);
    float3 lms = PQtoLinear_Vec(pqLms);

    static const float3x3 matLMStoRec709 = float3x3(
         6.173363f, -5.320381f, 0.147018f,
        -1.324312f, 2.560226f, -0.235915f,
        -0.011817f, -0.264672f, 1.276489f
    );

    return mul(matLMStoRec709, lms);
}

 // =========================================================================================
 // SCENE-LINEAR UNIVERSAL DE-TONEMAPPER (Diffuse-Protected ITM)
 // =========================================================================================

float3 ITM_SceneLinear(float3 sdrLinear, float hdrPeakNits)
{
    float3 color = max(sdrLinear, 0.0f);

    float maxC = max(color.r, max(color.g, color.b));
    float minC = min(color.r, min(color.g, color.b));
    float saturation = (maxC - minC) / max(maxC, 1e-4f);

    float saturationWeight = smoothstep(0.45f, 0.85f, saturation);

    float warmthRatio = saturate((color.r - color.b) / max(color.r, 1e-4f));
    
    float isOrganic = smoothstep(0.15f, 0.45f, warmthRatio);

    float maxAllowedGain = lerp(FX_ITM_MAX_GAIN, 1.25f, isOrganic);

    float threshold = lerp(0.985f, 0.88f, saturationWeight * (1.0f - isOrganic * 0.95f));
    float x = saturate((maxC - threshold) / max(1.0f - threshold, 1e-4f));

    float shoulder = x * x * (3.0f - 2.0f * x);
    float expansion = shoulder * shoulder * saturationWeight * (1.0f - isOrganic * 0.90f);

    float requestedGain = max(hdrPeakNits / 100.0f, 1.0f);
    float peakGain = min(requestedGain, maxAllowedGain);
    float gain = lerp(1.0f, peakGain, expansion);

    float3 hdrColor = color * gain;

    float desatAmount = expansion * 0.60f;
    float luma = GetLuma(hdrColor);
    hdrColor = lerp(hdrColor, float3(luma, luma, luma), desatAmount);

    return hdrColor;
}

// =========================================================================================
// DITHERING & OPTICAL EFFECTS
// =========================================================================================

float GetIGN(float2 uv, float2 res)
{
    float2 p = floor(uv * res);
    return frac(52.9829189f * frac(dot(p, float2(0.06711056f, 0.00583715f))));
}

// Bayer 2x2 dither with a random per-pixel phase (FRC Mode 0). Output range [-0.5, 0.5)
// `seed` is any value in [0,1) that varies per pixel and/or per frame (the animated grain texture is used).
// NOTE: the pixel shaders of this pipeline have no real frame counter (Offset.x is the texel size
//       in the final pass), so true time-based FRC is impossible; a random phase is used instead.
float GetFRC_Bayer(float2 uv, float2 res, float seed)
{
    float2 p = floor(uv * res);
    float bayer = frac(dot(fmod(p, 2.0f), float2(0.5f, 0.25f)));
    float phase = step(0.5f, frac(seed * 43.758f)); // decorrelated from the grain amplitude

    return frac(bayer + phase * 0.5f) - 0.5f;
}

// Interleaved Gradient Noise dither with a random per-pixel phase (FRC Mode 1). Output range [-0.5, 0.5)
float GetFRC_IGN(float2 uv, float2 res, float seed)
{
    float2 p = floor(uv * res);
    float phase = frac(seed * 43.758f);

    return frac(52.9829189f * frac(dot(p, float2(0.06711056f, 0.00583715f)) + phase)) - 0.5f;
}

// Cinematic Vignette
float3 ApplyVignette(float3 color, float2 uv)
{
    float2 centerOffset = uv - 0.5f;
    float distSq = dot(centerOffset, centerOffset);
    float vignette = pow(abs(saturate(1.0f - distSq * 1.5f)), FX_VIGNETTE_STRENGTH * 2.0f);

    return color * vignette;
}

// Procedural organic lens smudge and dust model (Branchless 4-point value hash noise)
float ProceduralDirt(float2 uv)
{
    float2 p = uv * 4.0f;
    float2 i = floor(p);
    float2 f = frac(p);
    float2 u = f * f * (3.0f - 2.0f * f);

    float4 h;
    h.x = frac(sin(dot(i + float2(0.0f, 0.0f), float2(127.1f, 311.7f))) * 43758.5453f);
    h.y = frac(sin(dot(i + float2(1.0f, 0.0f), float2(127.1f, 311.7f))) * 43758.5453f);
    h.z = frac(sin(dot(i + float2(0.0f, 1.0f), float2(127.1f, 311.7f))) * 43758.5453f);
    h.w = frac(sin(dot(i + float2(1.0f, 1.0f), float2(127.1f, 311.7f))) * 43758.5453f);

    float n = lerp(lerp(h.x, h.y, u.x), lerp(h.z, h.w, u.x), u.y);

    return pow(abs(n), 7.0f) * 2.5f;
}

// =========================================================================================
// TONEMAPPERS & COMPRESSION
// =========================================================================================

float3 CompressGamut(float3 color, float threshold, float strength)
{
    float luma = GetLuma(color);
    float maxChannel = max(color.r, max(color.g, color.b));
    float saturation = maxChannel - luma;
    float compression = saturate((saturation - threshold) * strength);

    return lerp(color, float3(luma, luma, luma), compression);
}

// Exponential luminance compression to simulate photographic film shoulder
float rangeCompress_Scalar(float val, float threshold)
{
    float compressed = threshold + (1.0f - threshold) * (1.0f - exp(-(val - threshold) / (1.0f - threshold)));

    return lerp(val, compressed, step(threshold, val));
}

// Debug Tool: False Color Heat Map
float3 Tonemap_FalseColor(float3 color)
{
    float luma = GetLuma(color);
    float logLuma = clamp((log2(max(luma, 1e-5f)) + 10.0f) * 0.071428f, 0.0f, 1.0f);

    float r = smoothstep(0.5f, 0.8f, logLuma) + step(0.9f, logLuma);
    float g = smoothstep(0.2f, 0.5f, logLuma) - smoothstep(0.7f, 0.9f, logLuma);
    float b = smoothstep(0.0f, 0.2f, logLuma) - smoothstep(0.4f, 0.5f, logLuma);

    return saturate(float3(r, g, b));
}

// Khronos PBR Neutral
float3 Tonemap_KhronosPBRNeutral(float3 color)
{
    const float startCompression = 0.8f - 0.04f;
    const float desaturation = 0.15f;

    float x = min(color.r, min(color.g, color.b));
    float offset = lerp(x - 6.25f * x * x, 0.04f, step(0.08f, x));
    color -= offset;

    float peak = max(color.r, max(color.g, color.b));
    float d = 1.0f - startCompression;
    float newPeak = 1.0f - d * d / max(peak + d - startCompression, 1e-5f);

    float3 compressedColor = color * (newPeak / max(peak, 1e-5f));
    float g = 1.0f - 1.0f / (desaturation * (peak - newPeak) + 1.0f);

    compressedColor = lerp(compressedColor, float3(newPeak, newPeak, newPeak), g);

    return lerp(color, compressedColor, step(startCompression, peak));
}

// ACES 2.0 Fit
float ACES2_ToneScale_Fwd(float Y)
{
    float m_2 = 1.0470f;
    float s_2 = 0.9078f;
    float g = 1.1500f;
    float t_1 = 0.0400f;

    float f = m_2 * pow(max(0.0f, Y) / (Y + s_2), g);
    float h = max(0.0f, (f * f) / (f + t_1));

    return h;
}

float3 Tonemap_ACES2_Fit(float3 color)
{
    float3 ictcp = RGBToICtCp(color);
    float I_linear = PQtoLinear_Scalar(ictcp.x);
    float I_mapped = ACES2_ToneScale_Fwd(I_linear);

    float chromaScale = saturate(lerp(1.1f, 0.0f, pow(abs(max(I_mapped, 0.0f)), 1.5f)));

    ictcp.x = linearToPQ_Scalar(I_mapped);
    ictcp.yz = ictcp.yz * chromaScale;

    return saturate(ICtCpToRGB(ictcp));
}

// AgX Tonemapper
float3 Tonemap_AgX(float3 val)
{
    static const float3x3 Rec709ToRec2020 = float3x3(
        0.6274040f, 0.3292830f, 0.0433131f,
        0.0690973f, 0.9195404f, 0.0113623f,
        0.0163914f, 0.0880133f, 0.8955953f
    );

    static const float3x3 Rec2020ToRec709 = float3x3(
         1.6604910f, -0.5876411f, -0.0728499f,
        -0.1245505f, 1.1328999f, -0.0083494f,
        -0.0181508f, -0.1005789f, 1.1187297f
    );

    static const float3x3 AgX_Inset = float3x3(
         0.856627f, 0.095121f, 0.048252f,
         0.137319f, 0.761242f, 0.101439f,
         0.111898f, 0.076799f, 0.811302f
    );

    // Outset (inverse of the inset with the AgX look rotation), rows sum to 1.0
    static const float3x3 AgX_Outset = float3x3(
         1.127101f, -0.110607f, -0.016494f,
        -0.141330f, 1.157824f, -0.016494f,
        -0.141330f, -0.110607f, 1.251936f
    );

    // log2(2^-10 * 0.18) and log2(2^6.5 * 0.18)  (16.5 stops around middle gray)
    const float minEv = -12.47393f;
    const float maxEv = 4.026069f;
    const float invRange = 1.0f / (maxEv - minEv);

    float3 agx = mul(Rec709ToRec2020, max(val, 0.0f));
    agx = mul(AgX_Inset, agx);
    agx = clamp((log2(max(agx, 1e-10f)) - minEv) * invRange, 0.0f, 1.0f);

    // Sigmoid (6th-order polynomial fit of the AgX base contrast)
    float3 x2 = agx * agx;
    float3 x4 = x2 * x2;

    agx = 15.5f * x4 * x2 - 40.14f * x4 * agx + 31.96f * x4 - 6.868f * x2 * agx + 0.4298f * x2 + 0.1191f * agx - 0.00232f;

    agx = mul(AgX_Outset, agx);
    agx = pow(max(agx, 0.0f), 2.2f); // 2.2 reference EOTF
    agx = mul(Rec2020ToRec709, agx);

    return saturate(agx);
}

// GT7 Tonemapper
float3 GT7_CurveEval(float3 x, float peak, float mid, float linearSec, float toe, float kA, float kB, float kC)
{
    float3 t = saturate(x / mid);
    float3 weightLinear = t * t * (3.0f - 2.0f * t);
    float3 weightToe = 1.0f - weightLinear;

    float3 shoulder = kA + kB * exp(x * kC);
    float3 toeMapped = mid * pow(abs(max(x / mid, 0.0f)), toe);
    float3 linearToe = weightToe * toeMapped + weightLinear * x;

    return lerp(linearToe, shoulder, step(linearSec * peak, x));
}

// GT7 Tonemapping (Gran Turismo 7 / Polyphony Digital)
float3 Tonemap_GT7(float3 rgb)
{
    float peakFb = 2.5f;
    float sdrCorrection = 0.4f;

    float mid = 0.538f;
    float alpha = 0.250f;
    float linearSec = 0.444f;
    float toe = 1.280f;

    float k = (linearSec - 1.0f) / (alpha - 1.0f);
    float kA = peakFb * linearSec + peakFb * k;
    float kB = -peakFb * k * exp(linearSec / k);
    float kC = -1.0f / (k * peakFb);

    float fbTargetUcs = linearToPQ_Scalar(peakFb);
    float3 ucs = RGBToICtCp(rgb);
    float3 skewedRgb = GT7_CurveEval(rgb, peakFb, mid, linearSec, toe, kA, kB, kC);
    float3 skewedUcs = RGBToICtCp(skewedRgb);

    float chromaT = saturate((ucs.x / max(fbTargetUcs, 1e-5f) - 0.98f) / 0.18f);
    float chromaScale = 1.0f - (chromaT * chromaT * (3.0f - 2.0f * chromaT));

    float3 scaledUcs = float3(skewedUcs.x, ucs.y * chromaScale, ucs.z * chromaScale);
    float3 scaledRgb = max(ICtCpToRGB(scaledUcs), 0.0f);
    float3 blended = lerp(skewedRgb, scaledRgb, 0.6f);

    return max(min(blended, peakFb), 0.0f) * sdrCorrection;
}

// Cinetone Tonemapper
float3 Tonemap_CineTone(float3 col)
{
    float3 ictcp = RGBToICtCp(col);
    float saturationAmount = pow(abs(smoothstep(1.0f, 0.3f, ictcp.x)), 1.3f);
    col = ICtCpToRGB(ictcp * float3(1.0f, saturationAmount, saturationAmount));

    float linearSegmentEnd = 0.25f;
    float maxCol = max(col.r, max(col.g, col.b));
    float mappedMax = rangeCompress_Scalar(maxCol, linearSegmentEnd);
    float denominator = max(maxCol, 1e-5f);

    float3 compressedHue = (col * mappedMax) / denominator;
    float3 perChannel = float3(
        rangeCompress_Scalar(col.r, linearSegmentEnd),
        rangeCompress_Scalar(col.g, linearSegmentEnd),
        rangeCompress_Scalar(col.b, linearSegmentEnd)
    );

    col = lerp(perChannel, compressedHue, 0.6f);

    return max(col, 0.0f);
}

// LUMINA Tonemapper
float3 Tonemap_Lumina(float3 vSample)
{
    float3 cleanSample = CompressGamut(vSample, 0.90f, 1.50f);
    float3 ictcp = RGBToICtCp(cleanSample);
    float I_pq = ictcp.x;

    static const float pqTargetMax = 0.508078f; // 100 nits SDR reference white in PQ
    static const float rcpPqTargetMax = 1.968202f;
    static const float shoulderPivot = 0.315008f;
    static const float toePivot = 0.050808f;
    static const float yMax = 0.193070f;

    // Real scene peak: the ITM expansion is capped by FX_ITM_MAX_GAIN, so a nominal
    // FX_ITM_HDR_PEAK above that never actually occurs in the frame.
#if ENABLE_ITM == 1
    float itmPeakNits = min(FX_ITM_HDR_PEAK, 100.0f * FX_ITM_MAX_GAIN);
#else
    float itmPeakNits = 100.0f;
#endif
    float scenePeakNits = max(itmPeakNits * FX_EXPOSURE, 100.0f);
    float pqSourceMax = NitToPQ(scenePeakNits);
    float xMax = max(pqSourceMax - shoulderPivot, 1e-4f);

    // Shadow toe
    float t_toe = saturate(I_pq * (1.0f / toePivot));
    float toe_val = toePivot * (t_toe + 0.15f * t_toe * (1.0f - t_toe));
    float mappedI_pq = lerp(toe_val, I_pq, step(toePivot, I_pq));

    // Midtone S-curve
    float normI = saturate(mappedI_pq * rcpPqTargetMax);
    float sInput = normI - 0.5f;
    mappedI_pq += (0.15f * pqTargetMax) * (sInput * (1.0f - 2.0f * abs(sInput)));

    // Shoulder
    float dx = max(mappedI_pq - shoulderPivot, 0.0f);
    const float kExp = 3.65f;
    const float normExp = 1.0f / (1.0f - exp(-kExp));
    float shoulder_val = shoulderPivot + yMax * (1.0f - exp(-(dx * kExp) / xMax)) * normExp;

    mappedI_pq = saturate(lerp(mappedI_pq, min(shoulder_val, pqTargetMax), step(shoulderPivot, mappedI_pq)));
    normI = mappedI_pq * rcpPqTargetMax;

    // Spectral burnout
    float chromaT = saturate((normI - 0.84f) * (1.0f / 0.16f));
    float satScale = 1.0f - (chromaT * chromaT * (3.0f - 2.0f * chromaT));

    float warmPreserve = smoothstep(0.78f, 1.0f, normI) * 0.85f;
    ictcp.y *= lerp(satScale, 1.0f, warmPreserve);
    ictcp.z *= satScale;
    ictcp.x = mappedI_pq;

    float3 pureICtCpRGB = max(ICtCpToRGB(ictcp), 0.0f);

    float I_lin = PQtoLinear_Scalar(I_pq);
    float mappedI_lin = PQtoLinear_Scalar(mappedI_pq);
    float3 huePreservedRGB = max(cleanSample * (mappedI_lin / max(I_lin, 1e-5f)), 0.0f);

    float blendFactor = smoothstep(0.35f, 0.92f, normI);
    float3 finalColor = lerp(huePreservedRGB, pureICtCpRGB, blendFactor * 0.78f);

    return saturate(finalColor);
}

#endif // FX_MATH_H
