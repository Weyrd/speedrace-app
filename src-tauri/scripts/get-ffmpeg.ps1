<#
.SYNOPSIS
  Fetch the bundled ffmpeg sidecar (Speedrace minimal build, win64, ~10 MB).

.DESCRIPTION
  Downloads our pinned minimal from-source ffmpeg (built by scripts/build-ffmpeg.ps1,
  hosted as a GitHub prerelease asset on this repo), verifies its SHA256, and extracts
  ffmpeg.exe to src-tauri/binaries/ffmpeg-<target-triple>.exe so Tauri's externalBin
  bundler picks it up. Run once after cloning (and after bumping the pin below).
  Idempotent: skips if the target already exists, unless -Force.

  The build contains exactly the components the app's pipeline uses (see
  README.md) — it is not a general-purpose ffmpeg.

  DTLS backend matters: the WHIP muxer needs a real DTLS-SRTP stack to publish to
  MediaMTX. This build is --enable-gnutls. Do NOT swap in a Windows SChannel build
  (e.g. BtbN's default win64-gpl): SChannel compiles the whip muxer but its DTLS
  fails the handshake at runtime (SEC_E_ALGORITHM_MISMATCH 0x80090331). GnuTLS,
  OpenSSL, or mbedTLS builds all work. Verify with:
    ffmpeg -buildconf   (expect --enable-gnutls / --enable-openssl / --enable-mbedtls)
    ffmpeg -protocols   (expect both `dtls` and `srtp`)

  GPL note: this build links x264 (GPL) and is distributed under GPLv3; the release
  the asset lives on records the exact upstream source commit and configure flags.

.NOTES
  To bump ffmpeg: rebuild with build-ffmpeg.ps1, verify per README.md,
  upload as a NEW prerelease tag (never overwrite an existing asset), then update
  $Url and $ExpectedSha256 here. The release must stay marked prerelease or it
  shadows `releases/latest`, which the app updater resolves.

  Fallback known-good full build (231 MB, same GnuTLS backend) if the minimal pin
  is ever broken: https://github.com/GyanD/codexffmpeg/releases/download/8.1.2/ffmpeg-8.1.2-full_build.zip
  (sha256 b8cdefab5f50590a076c27c2b56b0294a0e6154faded28ba1ba05ebc4f801f57)
#>
[CmdletBinding()]
param([switch]$Force)

$ErrorActionPreference = 'Stop'

# --- Pin -------------------------------------------------------------------
$Url           = 'https://github.com/Weyrd/speedrace-app/releases/download/ffmpeg-min-1/ffmpeg-min-win64.zip'
$ExpectedSha256 = '9b92d54352e0457a951fe07d7cd7a2d28707db7dca1dc8c2a429b64931a0ae90'
# ---------------------------------------------------------------------------

# Tauri sidecar naming: binaries/<name>-<target-triple>.exe
$triple = $null
try { $triple = (& rustc -vV | Select-String '^host:').ToString().Split(' ')[1] } catch {}
if (-not $triple) { $triple = 'x86_64-pc-windows-msvc' }

$binDir = Join-Path $PSScriptRoot '..\binaries'
$target = Join-Path $binDir "ffmpeg-$triple.exe"

if ((Test-Path $target) -and -not $Force) {
    Write-Host "ffmpeg sidecar already present: $target"
    Write-Host "Re-run with -Force to re-download."
    exit 0
}

New-Item -ItemType Directory -Force -Path $binDir | Out-Null

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("mom-ffmpeg-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$zip = Join-Path $tmp 'ffmpeg.zip'

try {
    Write-Host "Downloading $Url"
    curl.exe -sL --fail -o $zip $Url
    if ($LASTEXITCODE -ne 0) { throw "download failed (curl exit $LASTEXITCODE)" }

    $actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $ExpectedSha256) {
        throw @"
SHA256 mismatch for the downloaded ffmpeg build.
  expected: $ExpectedSha256
  actual:   $actual
The pinned $Url changed unexpectedly (release assets should be immutable — bumps
get a new tag). Only update `$ExpectedSha256 after confirming the asset is a legit
Speedrace minimal build with a DTLS backend: ffmpeg -buildconf shows --enable-gnutls,
and ffmpeg -protocols lists both dtls and srtp.
"@
    }

    Expand-Archive $zip -DestinationPath $tmp -Force
    $src = Get-ChildItem $tmp -Recurse -Filter ffmpeg.exe | Select-Object -First 1 -ExpandProperty FullName
    if (-not $src) { throw "ffmpeg.exe not found inside the archive" }

    Copy-Item $src $target -Force
    Write-Host "Installed sidecar: $target"
    & $target -hide_banner -version | Select-Object -First 1
}
finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
