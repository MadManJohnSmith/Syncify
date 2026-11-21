# Ensamblado del portable de Windows (#247): dist/portable/Syncify-Portable + zip.
#
# El portable es la opcion sin instalador: Syncify.exe + bin/ (ffmpeg, ffprobe
# y fpcalc colocados por dependency_manager.py) + scripts/ (puentes) + python/
# autonomo (install-runtime), con el bootstrapper opcional de WebView2 y el
# LEEME. Es el mismo arbol relativo que espera la app: get_project_root
# encuentra scripts/ junto al ejecutable y get_python_executable resuelve
# python/python.exe. El overlay de Tauri (tauri.windows.conf.json y su mapeo
# `_up_` para el NSIS) no cambia: el portable replica esa disposicion en su
# raiz, sin renombrar recursos.
#
# Las exclusiones de seguridad (TASK-95 / SEC-011) NO viven aqui: se consumen
# de scripts/ci/exclusions.txt, la fuente unica que tambien aplica el tarball
# crudo de Linux (assemble_raw.sh, rsync --exclude-from). El LEEME es una
# plantilla de fichero (scripts/ci/portable/LEEME.txt), no un here-string: un
# here-string inline ya rompio el YAML una vez (#246) y el texto se cambia en
# un solo sitio.
#
# WebView2 se conserva tal cual del inline: descarga por fwlink, allowlist de
# SHA-256 y tolerancia de release engineering (si no se puede descargar o el
# digest no esta en la allowlist, warning y se sigue sin bootstrapper).
#
# Uso: assemble_portable.ps1 [-RepoRoot <raiz>] [-SkipWebView2]
#
# Probado contra raices de repo sinteticas (scripts/tests/test_ci_windows_portable_assembly.py):
# arbol de scripts con basura -> salida limpia, y el mismo arbol produce el
# mismo scripts/ limpio que el adaptador Linux.

[CmdletBinding()]
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path,
    # Solo para los fixtures de test: evita la descarga de red. El build de
    # release no lo pasa y conserva el comportamiento del inline.
    [switch]$SkipWebView2
)

$ErrorActionPreference = "Stop"

$Binary = "target/release/Syncify.exe"
$AlternativeBinary = "target/release/syncify-tauri.exe"
$ScriptsDir = "scripts"
$ExternalsDir = "bin"
$Runtime = "python"
$Exclusions = "scripts/ci/exclusions.txt"
$ReadmeTemplate = "scripts/ci/portable/LEEME.txt"
$Out = "dist/portable"
$Name = "Syncify-Portable"
$ZipName = "Syncify-Windows-Portable.zip"

function Get-ExclusionPatterns {
    param([string]$Path)

    # Misma semantica que rsync --exclude-from: lineas vacias y comentarios se
    # ignoran; cada glob sin barra inicial casa contra el nombre en cualquier
    # nivel del arbol.
    $patterns = @()
    foreach ($line in Get-Content -LiteralPath $Path) {
        $pattern = $line.Trim()
        if (-not $pattern) { continue }
        if ($pattern.StartsWith("#")) { continue }
        $patterns += $pattern
    }
    return $patterns
}

function Test-ExcludedName {
    param(
        [string]$Name,
        [string[]]$Patterns
    )

    # -like es case-insensitive, la semantica segura en NTFS: un
    # `Credentials.json` tambien es un secreto. Las globs canonicas son
    # minusculas.
    foreach ($pattern in $Patterns) {
        if ($Name -like $pattern) { return $true }
    }
    return $false
}

function Remove-ExcludedItems {
    param(
        [string]$Root,
        [string[]]$Patterns
    )

    # Purga recursiva de defensa en profundidad: aunque Copy-Item -Exclude
    # filtre la copia, ningun subdirectorio debe retener secretos o caches.
    $excluded = Get-ChildItem -LiteralPath $Root -Recurse -Force | Where-Object {
        Test-ExcludedName -Name $_.Name -Patterns $Patterns
    }
    # De dentro hacia fuera: al borrar un directorio excluido sus hijos ya no
    # existen; los Remove-Item repetidos se ignoran.
    foreach ($item in ($excluded | Sort-Object { $_.FullName.Length } -Descending)) {
        Remove-Item -LiteralPath $item.FullName -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Push-Location $RepoRoot
try {
    # Validar todas las entradas ANTES de crear el arbol de salida: un fallo
    # temprano no deja un portable a medias que un paso posterior empaquetaria.
    foreach ($required in @($Exclusions, $ReadmeTemplate)) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
            throw "No existe la entrada requerida del portable: $required"
        }
    }
    if (-not (Test-Path -LiteralPath $ScriptsDir -PathType Container)) {
        throw "No existe el directorio de scripts: $ScriptsDir"
    }
    $binary = @($Binary, $AlternativeBinary) |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1
    if (-not $binary) {
        throw "No se encontro el ejecutable en target/release/ (corrio tauri build?)"
    }

    [string[]]$patterns = @(Get-ExclusionPatterns -Path $Exclusions)

    if (Test-Path -LiteralPath $Out) {
        Remove-Item -LiteralPath $Out -Recurse -Force
    }
    $portable = Join-Path $Out $Name
    New-Item -ItemType Directory -Force -Path $portable | Out-Null

    Write-Host ">>> Ejecutable: $binary"
    Copy-Item -LiteralPath $binary -Destination (Join-Path $portable "Syncify.exe")

    Write-Host ">>> scripts/ con las exclusiones de $Exclusions"
    $portableScripts = Join-Path $portable "scripts"
    Copy-Item -Path $ScriptsDir -Destination $portableScripts -Recurse -Force -Exclude $patterns
    Remove-ExcludedItems -Root $portableScripts -Patterns $patterns

    if (Test-Path -LiteralPath $ExternalsDir -PathType Container) {
        Write-Host ">>> bin/"
        Copy-Item -Path $ExternalsDir -Destination (Join-Path $portable "bin") -Recurse -Force
    } else {
        Write-Warning "No existe bin/: el portable saldra sin ffmpeg/ffprobe/fpcalc"
    }

    if (Test-Path -LiteralPath $Runtime -PathType Container) {
        Write-Host ">>> python/"
        Copy-Item -Path $Runtime -Destination (Join-Path $portable "python") -Recurse -Force
    } else {
        Write-Warning "No existe python/: el portable saldra sin runtime embebido"
    }

    Copy-Item -LiteralPath $ReadmeTemplate -Destination (Join-Path $portable "LEEME.txt")

    if ($SkipWebView2) {
        Write-Host ">>> WebView2 omitido (-SkipWebView2: fixture de test, sin red)"
    } else {
        Write-Host ">>> Descargando instalador WebView2 de soporte..."
        $webview2Path = Join-Path $portable "instalar_webview2_si_falla.exe"
        try {
            Invoke-WebRequest -Uri "https://go.microsoft.com/fwlink/p/?LinkId=2124703" -OutFile $webview2Path
            Write-Host ">>> Verificando integridad SHA-256 de instalador WebView2..."
            $wvHash = (Get-FileHash -Path $webview2Path -Algorithm SHA256).Hash.ToLower()
            $knownWvHashes = @(
                "17debf797a6c737959bc588236e897936ffac1af5f7e515e674ab32f9edfe719"
            )
            if ($knownWvHashes -notcontains $wvHash) {
                Remove-Item -Force $webview2Path
                throw "WebView2 installer SHA256 verification failed: untrusted digest $wvHash"
            }
        } catch {
            Write-Warning "No se pudo descargar o validar el instalador opcional de WebView2: $_"
            if (Test-Path $webview2Path) { Remove-Item -Force $webview2Path }
        }
    }

    $zipPath = Join-Path $Out $ZipName
    Compress-Archive -Path (Join-Path $portable "*") -DestinationPath $zipPath -Force
    Write-Host "Version Portable comprimida con exito en $zipPath"
} finally {
    Pop-Location
}
