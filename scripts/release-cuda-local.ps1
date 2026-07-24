#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Build the Windows CUDA installer locally and publish it to an existing GitHub
    release, then refresh the downloads-portal (R2) mirror.

.DESCRIPTION
    The CI `windows-cuda` installer job takes ~2h20m because GitHub's runners
    recompile the vendored CUDA/llama kernels cold every run. This machine keeps a
    warm `src-tauri/target`, so an incremental CUDA build takes minutes. This script
    faithfully reproduces the CI `windows-cuda` job's steps (frontend build, PDFium +
    CUDA/DirectML/cuDNN runtime staging, `tauri build --no-sign --bundles msi`), renames
    the MSI to the exact pattern the R2 mirror expects (`*_x64-cuda_en-US.msi`), uploads
    it to the release for the matching tag, and dispatches `mirror-release.yml`.

    Releases are unsigned (`--no-sign`), so a locally built MSI is equivalent to a CI one.
    The mirror requires exactly four production installers on the release; the other three
    (macOS dmg, DirectML msi, DirectML nsis exe) are still produced by CI.

.PARAMETER Tag
    Release tag to publish to (e.g. v0.10.10). Defaults to "v<package.json version>".

.PARAMETER CudaPath
    CUDA toolkit root. Defaults to $env:CUDA_PATH. Must contain bin\cudart64_12.dll etc.

.PARAMETER SkipUpload   Build + rename only; do not upload to the release.
.PARAMETER SkipMirror   Upload but do not dispatch the R2 mirror workflow.
.PARAMETER CancelCiCudaJob  Cancel a still-running CI `installer-windows-cuda` job for
    this tag before building locally (avoids a redundant 2h CI build).

.EXAMPLE
    pwsh scripts/release-cuda-local.ps1
    # builds + publishes the CUDA MSI for the current package.json version
#>
[CmdletBinding()]
param(
    [string]$Tag,
    [string]$CudaPath = $env:CUDA_PATH,
    [switch]$SkipUpload,
    [switch]$SkipMirror,
    [switch]$CancelCiCudaJob
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$timings = [ordered]@{}
function Step([string]$Name, [scriptblock]$Body) {
    Write-Host "`n=== $Name ===" -ForegroundColor Cyan
    $t = [System.Diagnostics.Stopwatch]::StartNew()
    & $Body
    $t.Stop()
    $timings[$Name] = $t.Elapsed
    Write-Host ("--- {0}: {1:mm\:ss}" -f $Name, $t.Elapsed) -ForegroundColor DarkGray
}
function Die([string]$Message) { throw "release-cuda-local: $Message" }
# Windows PowerShell 5.1 turns a native command's stderr into a terminating error when
# its own output is captured (non-console). Run build tools through `cmd /c "... 2>&1"`
# so stderr merges into stdout and PowerShell never sees raw native stderr.
function Invoke-Build([string]$CommandLine, [string]$What) {
    & cmd /c "$CommandLine 2>&1"
    if ($LASTEXITCODE -ne 0) { Die "$What failed (exit $LASTEXITCODE)" }
}

# --- Locate repo root (this script lives in <repo>/scripts) ---
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$SrcTauri = Join-Path $RepoRoot 'src-tauri'
Set-Location $RepoRoot
# Child processes (cmd/cargo/bun) use the process cwd, which Set-Location does not sync.
[System.IO.Directory]::SetCurrentDirectory($RepoRoot)

# --- Resolve version / tag ---
$pkgVersion = (Get-Content (Join-Path $RepoRoot 'package.json') -Raw | ConvertFrom-Json).version
if (-not $Tag) { $Tag = "v$pkgVersion" }
if ($Tag -ne "v$pkgVersion") {
    Write-Warning "Tag $Tag does not match package.json version $pkgVersion. The MSI will be stamped $pkgVersion."
}
$MsiName = "PURSUE.Data.Analyzer_${pkgVersion}_x64-cuda_en-US.msi"
Write-Host "Publishing CUDA installer for $Tag (version $pkgVersion) -> $MsiName"

$script:GitBash = $null
Step 'Preflight' {
    foreach ($tool in 'bun','cargo','gh') {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { Die "'$tool' is not on PATH" }
    }
    # Use Git Bash explicitly for the .sh staging scripts; the plain `bash` on this
    # machine can resolve to a (broken) WSL install.
    $bashCandidates = @(
        'C:\Program Files\Git\bin\bash.exe',
        'C:\Program Files\Git\usr\bin\bash.exe',
        "$env:ProgramFiles\Git\bin\bash.exe"
    )
    $script:GitBash = $bashCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $script:GitBash) { Die 'Git Bash (bash.exe) not found under Program Files\Git' }

    & gh auth status 2>$null *>$null; if ($LASTEXITCODE -ne 0) { Die "gh is not authenticated (run: gh auth login)" }

    if (-not $CudaPath -or -not (Test-Path (Join-Path $CudaPath 'bin\cudart64_12.dll'))) {
        Die "CUDA toolkit not found at '$CudaPath' (need bin\cudart64_12.dll). Pass -CudaPath or set CUDA_PATH."
    }

    # Free space guard: an incremental CUDA build + bundle needs headroom.
    $drive = (Get-Item $SrcTauri).PSDrive.Name
    $free = [math]::Round((Get-PSDrive $drive).Free / 1GB, 1)
    Write-Host "Free space on ${drive}: $free GB"
    if ($free -lt 12) { Die "Only $free GB free on ${drive}: - free up space before building (>=12 GB recommended)." }

    # WiX's ~2 GB cabinet and the pinned cuDNN download both use %TEMP% (usually C:). If that
    # drive is low, staging or the MSI bundle fail with a disk-full error (curl 23 / light.exe).
    # Route build temp to the target drive (which we just verified has headroom).
    $tempDrive = (Get-Item ([System.IO.Path]::GetTempPath())).PSDrive.Name
    $tempFree = [math]::Round((Get-PSDrive $tempDrive).Free / 1GB, 1)
    if ($tempDrive -ne $drive -and $tempFree -lt 15) {
        $buildTemp = Join-Path "${drive}:\" 'pursue-build-temp'
        New-Item -ItemType Directory -Force -Path $buildTemp | Out-Null
        $env:TEMP = $buildTemp; $env:TMP = $buildTemp; $env:RUNNER_TEMP = $buildTemp
        Write-Host "Temp drive ${tempDrive}: has only $tempFree GB free; routing build temp to $buildTemp"
    }

    if (-not $SkipUpload) {
        # The mirror needs all four installers; confirm CI already published the other three.
        $assets = (& gh release view $Tag --json assets --jq '.assets[].name' 2>$null)
        if ($LASTEXITCODE -ne 0) { Die "release $Tag not found (cut it with a 'release:' commit first)" }
        $assets = @($assets)
        $have = @($assets)
        $needDmg   = $have | Where-Object { $_ -match '_aarch64\.dmg$' }
        $needDmsi  = $have | Where-Object { $_ -match '_x64_en-US\.msi$' }
        $needNsis  = $have | Where-Object { $_ -match '_x64-setup\.exe$' }
        if (-not ($needDmg -and $needDmsi -and $needNsis)) {
            Write-Warning "The three CI installers are not all present on $Tag yet (dmg=$([bool]$needDmg) directml-msi=$([bool]$needDmsi) nsis=$([bool]$needNsis))."
            Write-Warning "You can still upload the CUDA MSI, but the mirror will fail until CI finishes. Continuing build..."
        }
    }
}

if ($CancelCiCudaJob) {
    Step 'Cancel redundant CI CUDA job' {
        $runId = & gh run list --workflow release.yml --json databaseId,headBranch,status `
            --jq "[.[] | select(.headBranch==\`"$Tag\`" and .status!=\`"completed\`")][0].databaseId" 2>$null
        if ($runId) {
            $jobs = & gh api "repos/{owner}/{repo}/actions/runs/$runId/jobs" --jq '.jobs[] | select(.name==\"installer-windows-cuda\" and .status!=\"completed\") | .id' 2>$null
            if ($jobs) {
                foreach ($j in @($jobs)) { & gh api -X POST "repos/{owner}/{repo}/actions/jobs/$j/cancel" *> $null }
                Write-Host "Requested cancel of CI installer-windows-cuda (run $runId)."
            } else { Write-Host "No running CI CUDA job to cancel." }
        } else { Write-Host "No in-progress CI run for $Tag." }
    }
}

Step 'Configure MSVC + CUDA + llama toolchain env' {
    # Prefer a VS 2022 (v17) toolset: CUDA 12.x nvcc rejects newer MSVC host compilers.
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { Die 'vswhere.exe not found (Visual Studio required)' }
    $vsPath = & $vswhere -latest -version '[17.0,18.0)' -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vsPath) {
        $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        Write-Warning "No VS 2022 (v17) toolset found; falling back to $vsPath. If nvcc rejects the host compiler, install VS 2022 Build Tools."
    }
    $vcvars = Join-Path $vsPath 'VC\Auxiliary\Build\vcvars64.bat'
    if (-not (Test-Path $vcvars)) { Die "vcvars64.bat not found under $vsPath" }
    # Import the MSVC developer environment (cl, INCLUDE, LIB, ...) into this session.
    & cmd /c "`"$vcvars`" >nul 2>&1 && set" | ForEach-Object {
        if ($_ -match '^([^=]+)=(.*)$') { Set-Item -Path "Env:$($matches[1])" -Value $matches[2] }
    }
    $cl = (Get-Command cl.exe -ErrorAction SilentlyContinue)
    if (-not $cl) { Die 'cl.exe not on PATH after vcvars64' }

    # VS bundles CMake + Ninja under CommonExtensions, but not every install ships the CMake
    # component and it may live in a different VS install than the v17 toolset used for cl.
    # Search every VS installation for them.
    $allVs = @(& $vswhere -all -products * -property installationPath) | Where-Object { $_ }
    $allVs = @($allVs + $vsPath | Select-Object -Unique)
    function Add-ToolFromVs([string]$tool, [string[]]$relDirs) {
        if (Get-Command $tool -ErrorAction SilentlyContinue) { return }
        foreach ($root in $allVs) {
            foreach ($rel in $relDirs) {
                $dir = Join-Path $root $rel
                if (Test-Path (Join-Path $dir "$tool.exe")) { $env:PATH = "$dir;$env:PATH"; return }
            }
        }
    }
    Add-ToolFromVs 'cmake' @('Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin')
    Add-ToolFromVs 'ninja' @('Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja')
    foreach ($tool in 'cmake','ninja') {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { Die "'$tool' not found in any VS install (needed by llama.cpp build)" }
    }

    # bindgen (llama-cpp-sys-4) needs libclang.dll. The repo vendors one under .tools\llvm.
    # On a warm target this crate does not rebuild, so treat a miss as a warning.
    $llvmCandidates = @(
        (Join-Path $RepoRoot '.tools\llvm\bin'),
        'C:\Program Files\LLVM\bin',
        (Join-Path $vsPath 'VC\Tools\Llvm\x64\bin')
    )
    $llvm = $llvmCandidates | Where-Object { Test-Path (Join-Path $_ 'libclang.dll') } | Select-Object -First 1
    if (-not $llvm) {
        Write-Warning "libclang.dll not found in: $($llvmCandidates -join '; '). A COLD llama-cpp-sys-4 rebuild would fail; a warm target build should skip it."
        $llvm = $llvmCandidates[0]
    } else {
        Write-Host "libclang: $llvm"
    }

    # Mirror the CI environment (.github/workflows/release.yml windows-cuda job).
    $env:LIBCLANG_PATH            = $llvm
    $env:CUDA_PATH                = $CudaPath
    $env:CUDA_HOME                = $CudaPath
    # cudaforge resolves nvcc as: $NVCC -> `which nvcc` (PATH) -> CUDA_HOME -> CUDA_PATH.
    # A different CUDA on PATH (e.g. an old 12.0) would win the PATH lookup, so pin NVCC
    # explicitly and put this toolkit's bin first.
    $env:NVCC                     = Join-Path $CudaPath 'bin\nvcc.exe'
    $env:PATH                     = (Join-Path $CudaPath 'bin') + ';' + $env:PATH
    $env:CUDA_COMPUTE_CAP         = '75'      # Turing/RTX 20xx floor; PTX JITs forward
    $env:CMAKE_GENERATOR          = 'Ninja'
    $env:NVCC_CCBIN               = $cl.Source
    $env:BINDGEN_EXTRA_CLANG_ARGS = '-DPURSUE_BINDGEN=1'
    $env:CARGO_BUILD_JOBS         = "$([Environment]::ProcessorCount)"
    $env:CMAKE_BUILD_PARALLEL_LEVEL = "$([Environment]::ProcessorCount)"
    Write-Host "cl:    $($cl.Source)"
    Write-Host "nvcc:  $((Get-Command nvcc -ErrorAction SilentlyContinue).Source)  (CUDA_PATH=$CudaPath)"
    Write-Host "cmake: $((Get-Command cmake).Source)"
    Write-Host "ninja: $((Get-Command ninja).Source)"

    # The vendored candle-kernels use half intrinsics (__hmax_nan) and headers that the
    # MSVC STL gates behind "CUDA 12.4 or newer". CUDA < 12.4 fails deep in nvcc; catch it here.
    $nvccVer = & "$CudaPath\bin\nvcc.exe" --version 2>$null | Select-String 'release ([0-9]+)\.([0-9]+)'
    if ($nvccVer) {
        $maj = [int]$nvccVer.Matches[0].Groups[1].Value
        $min = [int]$nvccVer.Matches[0].Groups[2].Value
        if ($maj -lt 12 -or ($maj -eq 12 -and $min -lt 4)) {
            Die "CUDA toolkit $maj.$min at $CudaPath is too old to build candle-kernels (needs >= 12.4, and must stay 12.x for cudart64_12 compatibility). Install CUDA 12.4-12.9 and pass -CudaPath or set CUDA_PATH. cudaforge already passes -allow-unsupported-compiler, so a modern MSVC toolset is fine."
        }
    }
}

Step 'Frontend dependencies (bun install)' {
    Invoke-Build 'bun install --frozen-lockfile' 'bun install'
}

Step 'Stage PDFium runtime' {
    # Merge stderr inside bash so PowerShell never sees raw native stderr (curl progress).
    & $script:GitBash -c './.github/scripts/stage-pdfium.sh 2>&1'
    if ($LASTEXITCODE -ne 0) { Die 'stage-pdfium.sh failed' }
}

Step 'Prebuild release provider set (cargo build --release)' {
    Invoke-Build "cd /d `"$SrcTauri`" && cargo build --release --features cuda,directml" 'cargo build --release --features cuda,directml'
}

Step 'Stage Windows native runtime (CUDA)' {
    # Call in-session (works on Windows PowerShell 5.1 and 7). The staging script's curl
    # calls are --silent so they emit no stderr on success (no NativeCommandError throw).
    & (Join-Path $RepoRoot '.github/scripts/stage-windows-runtime.ps1') -Cuda -Profile release
}

Step 'Build Tauri installer (msi, --no-sign)' {
    Invoke-Build 'bun tauri build --features cuda,directml --bundles msi --no-sign' 'tauri build'
}

$MsiDir = Join-Path $SrcTauri 'target\release\bundle\msi'
$built = @(Get-ChildItem $MsiDir -Filter '*_x64_en-US.msi' -ErrorAction SilentlyContinue)
if ($built.Count -ne 1) { Die "expected exactly one *_x64_en-US.msi in $MsiDir, found $($built.Count)" }
$FinalMsi = Join-Path $MsiDir $MsiName
Step 'Rename CUDA MSI' {
    Move-Item -LiteralPath $built[0].FullName -Destination $FinalMsi -Force -ErrorAction Stop
    $len = [math]::Round((Get-Item $FinalMsi).Length / 1MB, 1)
    Write-Host "Built: $FinalMsi ($len MB)"
}

if (-not $SkipUpload) {
    Step 'Upload CUDA MSI to release' {
        Invoke-Build "gh release upload $Tag `"$FinalMsi`" --clobber" 'gh release upload'
        Write-Host "Uploaded $MsiName to release $Tag"
    }
    if (-not $SkipMirror) {
        Step 'Dispatch downloads-portal mirror (mirror-release.yml)' {
            Invoke-Build "gh workflow run mirror-release.yml --field tag=$Tag" 'dispatch mirror-release.yml'
            Write-Host "Dispatched mirror-release.yml for $Tag. Watch: gh run watch --workflow mirror-release.yml"
        }
    }
}

$sw.Stop()
Write-Host "`n============ TIMING SUMMARY ============" -ForegroundColor Green
foreach ($k in $timings.Keys) { Write-Host ("{0,-45} {1:mm\:ss}" -f $k, $timings[$k]) }
Write-Host ("{0,-45} {1:hh\:mm\:ss}" -f 'TOTAL', $sw.Elapsed) -ForegroundColor Green
