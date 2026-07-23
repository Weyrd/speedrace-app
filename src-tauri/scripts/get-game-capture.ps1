<#
.SYNOPSIS
  Fetch the OBS game-capture helper binaries used to capture exclusive-fullscreen games.

.DESCRIPTION
  Exclusive-fullscreen games (Celeste etc.) are not DWM-composited, so WGC window capture
  returns black and ddagrab loses the display on the mode flip. The only way to grab such a
  game's pixels WITHOUT reading the whole screen is graphics-API hook injection — the OBS
  Game Capture model. We reuse OBS Studio's proven `win-capture` binaries as separate helper
  processes + an injected DLL; our own Rust client (src-tauri/src/stream/gamecapture/) drives
  them over the documented shared-memory/event protocol. Our app links none of this code.

  Downloads the pinned OBS portable zip, verifies its SHA256, and extracts the six win-capture
  files into src-tauri/binaries/gamecapture/ so Tauri's bundle.resources ships them next to the
  exe. Run once after cloning (and after bumping the pin below). Idempotent: skips if already
  present unless -Force.

  Files extracted (both bitnesses — a 64-bit host must still capture 32-bit games):
    graphics-hook32.dll  graphics-hook64.dll        (the injected hook)
    inject-helper32.exe  inject-helper64.exe        (cross-bitness injector)
    get-graphics-offsets32.exe  get-graphics-offsets64.exe  (per-boot D3D vtable offsets)

.NOTES
  LICENSING: OBS Studio is GPLv2. These are unmodified OBS binaries redistributed as separate
  helper executables + an injected DLL (mere aggregation — our app does not link them). Shipping
  them obliges us to carry OBS's attribution + a written offer of the corresponding source
  (obsproject/obs-studio @ the pinned tag). Keep NOTICE/licenses in sync when bumping the pin.

  ABI COUPLING: the `struct hook_info` layout mirrored in gamecapture/protocol.rs is the OBS
  win-capture ABI. It is stable across OBS releases but is NOT guaranteed forever — when you bump
  the pin, re-verify protocol.rs against shared/obs-hook-config/graphics-hook-info.h at that tag.

  To bump OBS: pick a new release tag, update $ObsVersion + $ExpectedSha256 (the GitHub release
  API exposes each asset's digest: `gh api repos/obsproject/obs-studio/releases/tags/<tag>
  --jq '.assets[] | "\(.name) \(.digest)"'`), re-run with -Force, re-verify protocol.rs.
#>
[CmdletBinding()]
param([switch]$Force)

$ErrorActionPreference = 'Stop'

# --- Pin -------------------------------------------------------------------
$ObsVersion     = '32.1.2'
$Url            = "https://github.com/obsproject/obs-studio/releases/download/$ObsVersion/OBS-Studio-$ObsVersion-Windows-x64.zip"
$ExpectedSha256 = '8d97e4563bd8d22d03e63042aa7dccede1d555c9bd35ce8a9e5019b0d0201bf6'

$wanted = @(
    'graphics-hook32.dll',
    'graphics-hook64.dll',
    'inject-helper32.exe',
    'inject-helper64.exe',
    'get-graphics-offsets32.exe',
    'get-graphics-offsets64.exe'
)

$dstDir = Join-Path $PSScriptRoot '..\binaries\gamecapture'

$haveAll = (Test-Path $dstDir) -and -not ($wanted | Where-Object { -not (Test-Path (Join-Path $dstDir $_)) })
if ($haveAll -and -not $Force) {
    Write-Host "OBS game-capture binaries already present: $dstDir"
    Write-Host "Re-run with -Force to re-download."
    exit 0
}

New-Item -ItemType Directory -Force -Path $dstDir | Out-Null

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("mom-gc-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$zip = Join-Path $tmp 'obs.zip'

try {
    Write-Host "Downloading OBS $ObsVersion portable (~185 MB, one-time)"
    Write-Host "  $Url"
    curl.exe -sL --fail -o $zip $Url
    if ($LASTEXITCODE -ne 0) { throw "download failed (curl exit $LASTEXITCODE)" }

    $actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $ExpectedSha256) {
        throw @"
SHA256 mismatch for the OBS portable zip.
  expected: $ExpectedSha256
  actual:   $actual
Release assets are immutable per tag; a bump gets a NEW tag. Only update `$ExpectedSha256 after
confirming the asset digest via the GitHub release API for the pinned tag.
"@
    }

    Write-Host "Extracting win-capture binaries"
    Expand-Archive $zip -DestinationPath $tmp -Force

    foreach ($name in $wanted) {
        $src = Get-ChildItem $tmp -Recurse -Filter $name -ErrorAction SilentlyContinue |
               Select-Object -First 1 -ExpandProperty FullName
        if (-not $src) { throw "'$name' not found inside the OBS zip (layout changed?)" }
        Copy-Item $src (Join-Path $dstDir $name) -Force
        Write-Host "  + $name"
    }

    Write-Host "Installed OBS game-capture binaries: $dstDir"
}
finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
