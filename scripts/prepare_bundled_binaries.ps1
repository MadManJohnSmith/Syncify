# Copia los binarios externos de bin\ a src-tauri\binaries\ con el sufijo del
# target triple, tal y como exige bundle.externalBin en tauri.conf.json.
# Espejo PowerShell de prepare_bundled_binaries.sh para builds de produccion
# locales en Windows. Requiere bin\{ffmpeg,ffprobe,fpcalc}.exe (descargalos
# antes con: python scripts/dependency_manager.py).
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$triple = if ($env:TARGET_TRIPLE) {
    $env:TARGET_TRIPLE
} else {
    ((rustc -vV | Select-String '^host:').Line -split '\s+')[1]
}
$ext = if ($triple -like "*windows*") { ".exe" } else { "" }

New-Item -ItemType Directory -Force -Path "src-tauri/binaries" | Out-Null

$missing = $false
foreach ($tool in @("ffmpeg", "ffprobe", "fpcalc")) {
    $src = "bin/$tool$ext"
    $dst = "src-tauri/binaries/$tool-$triple$ext"
    if (Test-Path $src) {
        Copy-Item -Force $src $dst
        Write-Host "OK: $dst"
    } else {
        Write-Host "FALTA: $src - ejecuta antes: python scripts/dependency_manager.py"
        $missing = $true
    }
}

if ($missing) { exit 1 }
