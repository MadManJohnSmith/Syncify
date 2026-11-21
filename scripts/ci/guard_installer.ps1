# Guardia del instalador NSIS de Windows (#246).
#
# Sin esta verificacion, un instalador sin el runtime Python embebido sale con
# exit 0 y la app cae al Python del sistema en una instalacion limpia
# (auditoria 1). Se mira DENTRO del instalador con 7-Zip, que sabe listar
# archivos NSIS, buscando la ruta donde Tauri aplana el glob
# `../python/**/*`: `_up_/python/python.exe`.
#
# TOLERANCIA (decision de release engineering, conservada tal cual del inline):
# si 7-Zip no consigue listar el instalador se avisa por warning y se sale 0.
# Bloquear la release por una limitacion de la herramienta seria peor que
# publicar sin esta comprobacion. Si 7-Zip SI lista y no encuentra el runtime,
# el build falla: ese es justo el fallo silencioso que hay que cazar.
#
# Los parametros de herramientas existen para los tests de comportamiento
# (scripts/tests/test_ci_windows_packaging_guards.py): en CI se usan los
# defaults y nadie los pasa.

[CmdletBinding()]
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path,
    [string]$Installer,
    [string]$PythonExe,
    [string]$SevenZip = "7z"
)

$ErrorActionPreference = "Stop"

if (-not $PythonExe) {
    $PythonExe = Join-Path $RepoRoot "python/python.exe"
}
if (-not $Installer) {
    $nsisDir = Join-Path (Join-Path (Join-Path $RepoRoot "target") "release") "bundle/nsis"
    $installerCandidate = Get-ChildItem -Path $nsisDir -Filter "*.exe" -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if (-not $installerCandidate) {
        throw "No se encontro el instalador NSIS en target/release/bundle/nsis/"
    }
    $Installer = $installerCandidate.FullName
} elseif (-not (Test-Path -LiteralPath $Installer -PathType Leaf)) {
    throw "No existe el instalador NSIS: $Installer"
}

if (-not (Test-Path -LiteralPath $PythonExe -PathType Leaf)) {
    throw "python/python.exe no existe: el instalador se empaquetaria sin runtime"
}

$listing = [System.IO.Path]::GetTempFileName()
try {
    & $SevenZip l $Installer > $listing 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Warning "7-Zip no pudo listar el instalador (salida $LASTEXITCODE): verificacion omitida"
        Get-Content -LiteralPath $listing -TotalCount 10
    } elseif (-not (Select-String -Path $listing -Pattern '_up_.+python\.exe' -Quiet)) {
        Write-Host "Primeras entradas del instalador:"
        Get-Content -LiteralPath $listing -TotalCount 20
        throw "El instalador NSIS no contiene _up_/python/python.exe: bundle.resources no lo empaco"
    } else {
        Write-Host "Runtime Python embebido confirmado en: $(Split-Path -Leaf $Installer)"
    }
} finally {
    Remove-Item -LiteralPath $listing -Force -ErrorAction SilentlyContinue
}

# Camino normal y camino tolerado salen 0; solo el runtime ausente con listado
# valido sale distinto de cero (el throw de arriba).
exit 0
