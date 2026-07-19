param(
    [switch]$Cuda
)

$ErrorActionPreference = 'Stop'
$stageDir = Join-Path $PSScriptRoot '..\..\src-tauri\assets\native_runtime'
$stageDir = [System.IO.Path]::GetFullPath($stageDir)
New-Item -ItemType Directory -Force -Path $stageDir | Out-Null

function Download-SignedMicrosoftFile([string]$Url, [string]$Destination) {
    & curl.exe --fail --location --retry 5 --retry-all-errors $Url --output $Destination
    if ($LASTEXITCODE -ne 0) { throw "Download failed: $Url" }
    $signature = Get-AuthenticodeSignature -LiteralPath $Destination
    if ($signature.Status -ne 'Valid' -or
        $signature.SignerCertificate.Subject -notmatch '(^|, )O=Microsoft Corporation(, |$)') {
        throw "Microsoft Authenticode validation failed for $Url"
    }
}

function Copy-ResolvedFile([string]$Source, [string]$DestinationDirectory) {
    $item = Get-Item -LiteralPath $Source
    if ($item.LinkType) {
        $target = @($item.Target)[0]
        if (-not [System.IO.Path]::IsPathRooted($target)) {
            $target = Join-Path $item.DirectoryName $target
        }
        $item = Get-Item -LiteralPath $target
    }
    if (-not $item -or $item.Length -eq 0) {
        throw "Runtime file is missing or empty: $Source"
    }
    Copy-Item -LiteralPath $item.FullName -Destination $DestinationDirectory -Force
}

function Download-PinnedFile([string]$Url, [string]$Destination, [string]$Sha256) {
    & curl.exe --fail --location --retry 5 --retry-all-errors $Url --output $Destination
    if ($LASTEXITCODE -ne 0) { throw "Download failed: $Url" }
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $Destination).Hash.ToLowerInvariant()
    if ($actual -ne $Sha256) {
        throw "Checksum mismatch for ${Url}: expected $Sha256, got $actual"
    }
}

$directMl = Join-Path $PSScriptRoot '..\..\src-tauri\target\debug\DirectML.dll'
Copy-ResolvedFile $directMl $stageDir
Download-PinnedFile `
    'https://raw.githubusercontent.com/microsoft/DirectML/8700779fe7a09ea7a007cf3d7ab4293c78e41017/LICENSE' `
    (Join-Path $stageDir 'Microsoft-DirectML-LICENSE.txt') `
    '27ebda9d51f0a56b7e281ccd8230a27236dcb51c05f64b07869ecf6e965d68b0'
Download-PinnedFile `
    'https://raw.githubusercontent.com/microsoft/onnxruntime/v1.24.2/LICENSE' `
    (Join-Path $stageDir 'Microsoft-ONNX-Runtime-LICENSE.txt') `
    '2f07c72751aed99790b8a4869cf2311df85a860b22ded05fa22803587a48922c'

# ONNX Runtime and the MSVC-built CUDA/DirectML libraries depend on the current
# Microsoft Visual C++ v14 runtime. Bundle Microsoft's signed x64 redistributable
# so the NSIS installer can repair/install it without asking users to diagnose a
# missing vcruntime DLL after installation.
Download-SignedMicrosoftFile `
    'https://aka.ms/vs/17/release/vc_redist.x64.exe' `
    (Join-Path $stageDir 'vc_redist.x64.exe')

# ort's copy-dylibs feature may create symlinks into its user-level download cache. Those
# links are fragile in sandboxes and on locked-down Windows installations, and the test
# loader searches target\debug\deps before our application setup can configure DLL paths.
# Materialize the verified DLL beside both executables so tests exercise the same payload
# that the installer will ship.
$targetRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\src-tauri\target\debug'))
foreach ($directory in @($targetRoot, (Join-Path $targetRoot 'deps'))) {
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    $destination = Join-Path $directory 'DirectML.dll'
    if (Test-Path -LiteralPath $destination) {
        $resolvedDestination = [System.IO.Path]::GetFullPath($destination)
        if (-not $resolvedDestination.StartsWith($targetRoot + [System.IO.Path]::DirectorySeparatorChar) -and
            $resolvedDestination -ne (Join-Path $targetRoot 'DirectML.dll')) {
            throw "Unsafe DirectML materialization target: $resolvedDestination"
        }
        Remove-Item -LiteralPath $destination -Force
    }
    Copy-Item -LiteralPath (Join-Path $stageDir 'DirectML.dll') -Destination $destination -Force
}

if ($Cuda) {
    foreach ($name in @('onnxruntime_providers_cuda.dll', 'onnxruntime_providers_shared.dll')) {
        Copy-ResolvedFile (Join-Path $PSScriptRoot "..\..\src-tauri\target\debug\$name") $stageDir
    }

    if (-not $env:CUDA_PATH -or -not (Test-Path -LiteralPath $env:CUDA_PATH)) {
        throw 'CUDA_PATH does not point to the installed CUDA 12 toolkit.'
    }
    foreach ($pattern in @('cudart64_12.dll', 'cublas64_12.dll', 'cublasLt64_12.dll', 'cufft64_*.dll', 'nvrtc64_*.dll', 'nvrtc-builtins64_*.dll')) {
        $matches = @(Get-ChildItem -LiteralPath (Join-Path $env:CUDA_PATH 'bin') -Filter $pattern)
        if ($matches.Count -eq 0) {
            throw "CUDA runtime component not found: $pattern"
        }
        $matches | ForEach-Object { Copy-ResolvedFile $_.FullName $stageDir }
    }
    $cudaLicense = Join-Path $env:CUDA_PATH 'LICENSE'
    if (-not (Test-Path -LiteralPath $cudaLicense)) {
        throw 'The CUDA toolkit redistribution license was not found.'
    }
    Copy-Item -LiteralPath $cudaLicense -Destination (Join-Path $stageDir 'NVIDIA-CUDA-LICENSE.txt') -Force

    $cudnnVersion = '9.13.1.26'
    $cudnnArchive = "cudnn-windows-x86_64-${cudnnVersion}_cuda12-archive.zip"
    $cudnnSha256 = '09c429bff7b69419e596efbd8193ba8430b808c8cfaab58050e2a16ea9184ad7'
    $tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
    $workDir = Join-Path $tempRoot "pursue-cudnn-$cudnnVersion"
    $archivePath = Join-Path $tempRoot $cudnnArchive
    $runnerTemp = [System.IO.Path]::GetFullPath($tempRoot).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    if (-not ([System.IO.Path]::GetFullPath($workDir).StartsWith($runnerTemp))) {
        throw "Unsafe cuDNN temporary path: $workDir"
    }
    New-Item -ItemType Directory -Force -Path $workDir | Out-Null
    & curl.exe --fail --location --retry 5 --retry-all-errors `
        "https://developer.download.nvidia.com/compute/cudnn/redist/cudnn/windows-x86_64/$cudnnArchive" `
        --output $archivePath
    if ($LASTEXITCODE -ne 0) { throw 'cuDNN download failed.' }
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $archivePath).Hash.ToLowerInvariant()
    if ($actual -ne $cudnnSha256) {
        throw "cuDNN checksum mismatch: expected $cudnnSha256, got $actual"
    }
    Expand-Archive -LiteralPath $archivePath -DestinationPath $workDir -Force
    $cudnnDlls = @(Get-ChildItem -LiteralPath $workDir -Filter 'cudnn*.dll' -Recurse)
    if ($cudnnDlls.Count -lt 2) { throw 'The cuDNN archive did not contain the expected runtime DLLs.' }
    $cudnnDlls | ForEach-Object { Copy-ResolvedFile $_.FullName $stageDir }
    $license = Get-ChildItem -LiteralPath $workDir -Filter 'LICENSE*' -Recurse | Select-Object -First 1
    if (-not $license) { throw 'The cuDNN license was not found in the archive.' }
    Copy-Item -LiteralPath $license.FullName -Destination (Join-Path $stageDir 'NVIDIA-cuDNN-LICENSE.txt') -Force
    Remove-Item -LiteralPath $archivePath -Force
    Remove-Item -LiteralPath $workDir -Recurse -Force
}

$required = @('DirectML.dll', 'vc_redist.x64.exe')
if ($Cuda) {
    $required += @('onnxruntime_providers_cuda.dll', 'onnxruntime_providers_shared.dll', 'cudnn64_9.dll', 'cudart64_12.dll', 'cublas64_12.dll', 'cublasLt64_12.dll', 'cufft64_11.dll')
}
foreach ($name in $required) {
    $path = Join-Path $stageDir $name
    if (-not (Test-Path -LiteralPath $path) -or (Get-Item -LiteralPath $path).Length -eq 0) {
        throw "Staged runtime validation failed: $name"
    }
}
Write-Host "Staged native runtime: $($required -join ', ')"
