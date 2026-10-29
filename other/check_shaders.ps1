# =====================================================================
# HLSL Shader Validation & Instruction Audit Script (Shader Model 3.0)
# Designed for SpellForce 2 / DirectX 9 Engine Pipelines
# =====================================================================

$ErrorActionPreference = "Continue"

# Always work relative to the script folder, so the script runs from any directory
$root = $PSScriptRoot
if ([string]::IsNullOrEmpty($root)) { $root = Split-Path -Parent $MyInvocation.MyCommand.Path }
if ([string]::IsNullOrEmpty($root)) { $root = (Get-Location).Path }
Push-Location $root

# Locate fxc.exe (Hardcoded fallback + Auto-detection across Windows Kits)
$fxc = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.28000.0\x86\fxc.exe"

if (-not (Test-Path $fxc)) {
    $foundFxc = Get-ChildItem -Path "C:\Program Files (x86)\Windows Kits\10\bin" -Filter "fxc.exe" -Recurse -ErrorAction SilentlyContinue |
                Where-Object { $_.FullName -match "\\x86\\fxc\.exe$|\\x64\\fxc\.exe$" } |
                Sort-Object FullName |
                Select-Object -Last 1

    if ($foundFxc) {
        $fxc = $foundFxc.FullName
    } else {
        Write-Host "[CRITICAL ERROR] fxc.exe was not found! Please specify the path manually." -ForegroundColor Red
        Pop-Location
        exit 1
    }
}

Write-Host "Using compiler: $fxc`n" -ForegroundColor DarkGray

# Shader manifest: Files and corresponding entry points
$shaders = @(
    @{
        file = "pp_blur.fx"
        vs   = @("VS_CopyLinear", "VS_DownSample", "VS_DualKawase_Down", "VS_DualKawase_Up", "VS_BlurDesaturate")
        ps   = @("PS_CopyLinear", "PS_DownSample", "PS_DualKawase_Down", "PS_DualKawase_Up", "PS_BlurDesaturate")
    },
    @{
        file = "pp_color_grading.fx"
        vs   = @("VS_ColorGrading")
        ps   = @("PS_ColorGrading", "PS_ColorLUTBlend")
    },
    @{
        file = "pp_debug.fx"
        vs   = @("VS_Debug")
        ps   = @("PS_Debug")
    },
    @{
        file = "pp_fxaa.fx"
        vs   = @("VS_FXAA")
        ps   = @("PS_FXAA")
    },
    @{
        file = "pp_glow.fx"
        vs   = @("VS_Glow")
        ps   = @("PS_MaskHighLights", "PS_Glow")
    },
    @{
        file = "pp_noise.fx"
        vs   = @("VS_Noise")
        ps   = @("PS_Noise")
    },
    @{
        file = "pp_sharpen.fx"
        vs   = @("VS_Sharpen")
        ps   = @("PS_Sharpen")
    }
)

$asmFile   = Join-Path $root "temp_asm.txt"
$nullOut   = Join-Path $root "temp_fx.obj"
$slotLimit = 512   # ps_3_0 guaranteed minimum; loops with dynamic break may be counted differently
$errCount  = 0
$warnCount = 0

function Show-Warnings($out, $indent) {
    # Print compiler warnings while filtering out the deprecation notice X4717
    $w = $out | Where-Object { "$_" -match "warning X\d+" -and "$_" -notmatch "X4717" }
    foreach ($line in $w) {
        $script:warnCount++
        Write-Host "$indent[WARN] $line" -ForegroundColor Yellow
    }
}

foreach ($s in $shaders) {
    $file = Join-Path $root $s.file
    if (-not (Test-Path $file)) {
        Write-Host "File not found: $file" -ForegroundColor DarkYellow
        continue
    }

    Write-Host "=====================================================" -ForegroundColor Cyan
    Write-Host " FILE: $($s.file)" -ForegroundColor Cyan
    Write-Host "=====================================================" -ForegroundColor Cyan

    # A. Whole-effect check (techniques, samplers, uniform bindings)
    $out = & $fxc /T fx_2_0 /O3 /nologo /Fo $nullOut /I $root $file 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  [OK] Effect (techniques + samplers)" -ForegroundColor Green
        Show-Warnings $out "    "
    } else {
        $errCount++
        Write-Host "  [ERROR] Effect (fx_2_0) failed to compile!" -ForegroundColor Red
        $out | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkRed }
    }

    # B. Validate Vertex Shaders (vs_3_0)
    foreach ($vs in $s.vs) {
        $out = & $fxc /T vs_3_0 /E $vs /O3 /nologo /I $root $file 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  [OK] Vertex Shader: $vs" -ForegroundColor Green
            Show-Warnings $out "    "
        } else {
            $errCount++
            Write-Host "  [ERROR] VS $vs failed to compile!" -ForegroundColor Red
            $out | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkRed }
        }
    }

    # C. Validate Pixel Shaders (ps_3_0) and report instruction / texture slots
    foreach ($ps in $s.ps) {
        $out = & $fxc /T ps_3_0 /E $ps /O3 /nologo /I $root /Fc $asmFile $file 2>&1

        if ($LASTEXITCODE -eq 0) {
            $info = ""
            $slots = 0
            if (Test-Path $asmFile) {
                $line = Get-Content $asmFile | Select-String -Pattern "approximately (\d+) instruction slots used( \((\d+) texture, (\d+) arithmetic\))?" | Select-Object -First 1
                if ($line) {
                    $slots = [int]$line.Matches[0].Groups[1].Value
                    $info  = " -> $slots slots"
                    if ($line.Matches[0].Groups[3].Success) {
                        $info += " ($($line.Matches[0].Groups[3].Value) tex, $($line.Matches[0].Groups[4].Value) alu)"
                    }
                }
            }

            if ($slots -gt $slotLimit) {
                $errCount++
                Write-Host "  [OVERFLOW] Pixel Shader: $ps$info (EXCEEDS $slotLimit)" -ForegroundColor Red
            } else {
                Write-Host "  [OK] Pixel Shader: $ps$info" -ForegroundColor Green
            }
            Show-Warnings $out "    "
        } else {
            $errCount++
            Write-Host "  [ERROR] PS $ps failed to compile!" -ForegroundColor Red
            $out | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkRed }
        }

        if (Test-Path $asmFile) { Remove-Item $asmFile -Force }
    }

    if (Test-Path $nullOut) { Remove-Item $nullOut -Force }
    Write-Host ""
}

Pop-Location

$color = if ($errCount -eq 0) { "Green" } else { "Red" }
Write-Host "Shader validation completed: $errCount error(s), $warnCount warning(s)." -ForegroundColor $color
Write-Host ""
