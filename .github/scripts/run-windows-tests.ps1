param(
    [string]$Features = 'directml'
)

$ErrorActionPreference = 'Stop'
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$tauriDir = Join-Path $repoRoot 'src-tauri'
$manifest = Join-Path $tauriDir 'windows-common-controls.manifest'
$mt = (Get-Command mt.exe -ErrorAction Stop).Source

Push-Location $tauriDir
try {
    & cargo test --features $Features --lib --no-run
    if ($LASTEXITCODE -ne 0) { throw 'Windows test compilation failed.' }

    $testExe = Get-ChildItem -LiteralPath (Join-Path $tauriDir 'target\debug\deps') `
        -Filter 'pursue_data_analyzer_lib-*.exe' |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if (-not $testExe) { throw 'Cargo did not produce the expected unit-test executable.' }

    # Tauri's application manifest is attached only to the production binary. Embed the
    # same common-controls v6 activation into the Rust unit harness; otherwise Windows
    # binds comctl32 v5 and exits before main with STATUS_ENTRYPOINT_NOT_FOUND.
    & $mt -nologo -manifest $manifest "-outputresource:$($testExe.FullName);#1"
    if ($LASTEXITCODE -ne 0) { throw 'Failed to embed the Windows test manifest.' }

    & $testExe.FullName
    if ($LASTEXITCODE -ne 0) { throw "Windows unit tests failed with exit code $LASTEXITCODE." }
}
finally {
    Pop-Location
}
