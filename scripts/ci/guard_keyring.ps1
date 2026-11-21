# Guardia del keyring de Windows (#246).
#
# Sin la feature `windows-native`, keyring compila su store mock (EntryOnly, sin
# persistencia) y cada dos arranques se rotaba la clave de cifrado, dejando
# TODAS las cuentas marcadas como irrecuperables. Linux no puede cazar ese
# fallo porque el test es `#[cfg(target_os = "windows")]`.
#
# Leccion STATUS_ENTRYPOINT_NOT_FOUND: el exe de test no lleva el manifiesto
# que tauri-build embebe en el binario de la app, asi que el loader resuelve
# comctl32 v5 y muere al cargar por imports que solo existen en v6
# (SetWindowSubclass y compania, via tao/wry). Por eso el manifiesto v6 se
# embebe en el exe de test con mt.exe ANTES de correrlo, y por eso el
# diagnostico del loader (dumpbin) vive en el camino de fallo de esta guardia:
# distingue un fallo del runner entero (una crate minima tambien muere) de uno
# del exe de la app, y lista los dependientes del exe para ver que DLL resuelve
# el loader y desde donde.
#
# El manifiesto es un fichero (scripts/ci/comctl32.manifest), no un
# here-string: un here-string inline ya rompio el YAML del workflow una vez.
#
# Los parametros de herramientas existen para los tests de comportamiento
# (scripts/tests/test_ci_windows_packaging_guards.py): en CI se usan los
# defaults y nadie los pasa.

[CmdletBinding()]
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path,
    [string]$Manifest = (Join-Path $PSScriptRoot "comctl32.manifest"),
    [string]$Cargo = "cargo",
    [string]$Mt,
    [string]$Dumpbin,
    [string]$MinimalCrate = "syncify-fmp4-flac"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $Manifest -PathType Leaf)) {
    throw "No existe el manifiesto comctl32: $Manifest"
}

function Resolve-Dumpbin {
    if ($Dumpbin) {
        if (Test-Path -LiteralPath $Dumpbin -PathType Leaf) { return $Dumpbin }
        Write-Warning "dumpbin no existe: $Dumpbin"
        return $null
    }
    $programFilesX86 = ${env:ProgramFiles(x86)}
    if (-not $programFilesX86) {
        Write-Warning "ProgramFiles(x86) no esta definido: no se puede localizar vswhere/dumpbin"
        return $null
    }
    $vswhere = Join-Path $programFilesX86 "Microsoft Visual Studio/Installer/vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
        Write-Warning "vswhere no encontrado: $vswhere"
        return $null
    }
    $vsRoot = & $vswhere -latest -property installationPath
    if ($LASTEXITCODE -ne 0 -or -not $vsRoot) {
        Write-Warning "vswhere no devolvio una instalacion de Visual Studio"
        return $null
    }
    $dumpbinCandidate = Get-ChildItem -Path (Join-Path $vsRoot "VC/Tools/MSVC/*/bin/Hostx64/x64/dumpbin.exe") -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if (-not $dumpbinCandidate) {
        Write-Warning "dumpbin.exe no encontrado bajo $vsRoot"
        return $null
    }
    return $dumpbinCandidate.FullName
}

function Invoke-LoaderDiagnostics {
    param([string]$ExePath)

    # Nunca debe tapar el fallo original: cada sonda es best-effort y el exit
    # code de la guardia se preserva fuera de esta funcion.
    if ($MinimalCrate) {
        Write-Host ">>> 1. Carga un binario de test minimo? ($MinimalCrate)"
        & $Cargo test --locked -p $MinimalCrate --lib
    }

    Write-Host ">>> 2. Dependientes del exe de la app"
    Write-Host "exe: $ExePath"
    $dumpbinTool = Resolve-Dumpbin
    if (-not $dumpbinTool) {
        Write-Warning "Sin dumpbin: diagnostico del loader incompleto"
        return
    }
    Write-Host "dumpbin: $dumpbinTool"
    & $dumpbinTool /DEPENDENTS $ExePath

    Write-Host ">>> 3. Imports de comctl32 (v5 no exporta lo que importa tao/wry)"
    & $dumpbinTool /IMPORTS:comctl32.dll $ExePath
}

Push-Location $RepoRoot
try {
    Write-Host ">>> Compilando el binario de test del keyring (sin ejecutar)"
    & $Cargo test --locked -p syncify-tauri --lib --no-run
    if ($LASTEXITCODE -ne 0) {
        throw "cargo test --no-run fallo con codigo $LASTEXITCODE"
    }

    $deps = Join-Path (Join-Path (Join-Path $RepoRoot "target") "debug") "deps"
    $exe = Get-ChildItem -Path $deps -Filter "syncify_tauri_lib-*.exe" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $exe) {
        throw "No se encontro el exe de test en $deps (syncify_tauri_lib-*.exe)"
    }

    if ($Mt) {
        if (-not (Test-Path -LiteralPath $Mt -PathType Leaf)) {
            throw "mt.exe no existe: $Mt"
        }
    } else {
        $mtCandidate = Get-ChildItem -Path "C:/Program Files (x86)/Windows Kits/10/bin/*/x64/mt.exe" -ErrorAction SilentlyContinue |
            Sort-Object FullName -Descending | Select-Object -First 1
        if (-not $mtCandidate) {
            throw "mt.exe no encontrado en Windows Kits"
        }
        $Mt = $mtCandidate.FullName
    }

    Write-Host "mt: $Mt"
    Write-Host "exe: $($exe.FullName)"
    Write-Host "manifest: $Manifest"

    & $Mt -manifest $Manifest -outputresource:"$($exe.FullName);#1"
    if ($LASTEXITCODE -ne 0) {
        throw "mt.exe fallo al embeber el manifiesto"
    }

    Write-Host ">>> Test de persistencia del Credential Manager"
    & $Cargo test --locked -p syncify-tauri --lib crypto::tests::windows_build_uses_a_persistent_credential_store -- --nocapture
    $guardExit = $LASTEXITCODE

    if ($guardExit -ne 0) {
        Write-Host ">>> La guardia fallo (salida $guardExit): diagnostico del loader"
        try {
            Invoke-LoaderDiagnostics -ExePath $exe.FullName
        } catch {
            Write-Warning "El diagnostico del loader tambien fallo: $_"
        }
    }
} finally {
    Pop-Location
}

exit $guardExit
