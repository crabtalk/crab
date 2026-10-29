# Installs `crab` from the latest release of github.com/crabtalk/crab, on
# Windows.
#
#   irm https://crab.talk/install.ps1 | iex
#
# CRAB_INSTALL_DIR sets where `crab.exe` goes; it defaults to ~\.local\bin,
# which is added to the user's PATH when it is not on it.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'crabtalk/crab'
$Asset = 'crab-x86_64-pc-windows-msvc.zip'
$InstallDir = if ($env:CRAB_INSTALL_DIR) { $env:CRAB_INSTALL_DIR } else { Join-Path $HOME '.local\bin' }

function Fail($message) {
  Write-Error "crab: $message"
  exit 1
}

# The latest release's asset, and the sha256 GitHub records for it.
$api = "https://api.github.com/repos/$Repo/releases/latest"
try {
  $release = Invoke-RestMethod -Uri $api -Headers @{ 'User-Agent' = 'crab-install' }
} catch {
  Fail "could not read $api"
}
$found = $release.assets | Where-Object { $_.name -eq $Asset } | Select-Object -First 1
if (-not $found) { Fail "the latest release has no $Asset" }
if (-not ($found.digest -like 'sha256:*')) { Fail "GitHub records no sha256 for $($found.browser_download_url)" }
$digest = $found.digest.Substring('sha256:'.Length)

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("crab-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
  $zip = Join-Path $tmp $Asset
  Write-Host "downloading $($found.browser_download_url)"
  try {
    Invoke-WebRequest -Uri $found.browser_download_url -OutFile $zip -UseBasicParsing
  } catch {
    Fail "could not download $($found.browser_download_url)"
  }
  if ((Get-FileHash -Algorithm SHA256 $zip).Hash.ToLower() -ne $digest.ToLower()) {
    Fail "$($found.browser_download_url) does not match the sha256 GitHub records for it"
  }
  Expand-Archive -Path $zip -DestinationPath $tmp -Force
  New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
  Move-Item -Path (Join-Path $tmp 'crab.exe') -Destination (Join-Path $InstallDir 'crab.exe') -Force
} finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

$exe = Join-Path $InstallDir 'crab.exe'
$version = try { & $exe --version } catch { 'crab' }
Write-Host "installed $version to $exe"

$path = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (($path -split ';') -contains $InstallDir)) {
  [Environment]::SetEnvironmentVariable('Path', (@($path, $InstallDir) | Where-Object { $_ }) -join ';', 'User')
  Write-Host "added $InstallDir to your PATH; open a new terminal to run crab"
}
