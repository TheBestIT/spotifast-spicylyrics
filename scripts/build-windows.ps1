<#
.SYNOPSIS
  Build Spotifast Spicy Lyrics from source on Windows, and install it if you ask.

.DESCRIPTION
  The script finds a C toolchain, builds a release program with cargo, and
  tells you where the program is. It works with either toolchain:

    * Visual Studio Build Tools (MSVC) - the toolchain the official releases use.
    * MinGW-w64 (GCC) - a smaller download.

  MilkDrop is left out by default, because it needs CMake, LLVM (libclang)
  and GLEW as well. Add -WithMilkDrop when those tools are installed.

.PARAMETER Install
  Copy the program to %LOCALAPPDATA%\Programs\Spotifast and make a Start menu
  shortcut. An installed Spotifast is saved as spotifast.exe.original first.

.PARAMETER WithMilkDrop
  Build with the MilkDrop visualiser (needs MSVC, CMake, LLVM and GLEW from vcpkg).

.PARAMETER InstallRust
  Install rustup with winget when cargo is missing.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1 -Install
#>
param(
    [switch]$Install,
    [switch]$WithMilkDrop,
    [switch]$InstallRust
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)

function Step($text) { Write-Host "==> $text" -ForegroundColor Cyan }
function Fail($text) { Write-Host "ERROR: $text" -ForegroundColor Red; exit 1 }

# 1. Rust.
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    if ($InstallRust) {
        Step "Install rustup with winget"
        winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements
        $env:PATH = "$cargoBin;$env:PATH"
    } else {
        Fail "cargo is not installed. Run this script again with -InstallRust, or install Rust from https://rustup.rs"
    }
}

# 2. A C toolchain: MSVC first, then MinGW-w64.
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$msvc = $false
if (Test-Path $vswhere) {
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    $msvc = [bool]$vs
}
$gcc = Get-Command gcc -ErrorAction SilentlyContinue
if ($msvc) {
    $target = "x86_64-pc-windows-msvc"
    Step "Use Visual Studio Build Tools (MSVC)"
} elseif ($gcc) {
    $target = "x86_64-pc-windows-gnu"
    Step "Use MinGW-w64 at $($gcc.Source)"
    if ($WithMilkDrop) { Fail "MilkDrop builds need MSVC. Remove -WithMilkDrop or install Visual Studio Build Tools." }
} else {
    Write-Host ""
    Write-Host "No C toolchain is installed. Install ONE of these, then run the script again:" -ForegroundColor Yellow
    Write-Host "  Visual Studio Build Tools (recommended):"
    Write-Host "    winget install Microsoft.VisualStudio.2022.BuildTools --override `"--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended`""
    Write-Host "  MinGW-w64 (smaller):"
    Write-Host "    winget install BrechtSanders.WinLibs.POSIX.UCRT"
    exit 1
}
# 3. Build. Inside the repository, so rustup uses the pinned toolchain.
$features = if ($WithMilkDrop) { @() } else { @("--no-default-features") }
Push-Location $root
try {
    rustup target add $target | Out-Null
    Step "Build the release program (the first build takes 5 to 15 minutes)"
    & cargo build --release --target $target @features
    if ($LASTEXITCODE -ne 0) { Fail "The build failed. Read the messages above." }
} finally {
    Pop-Location
}
$exe = Join-Path $root "target\$target\release\spotifast.exe"
Write-Host ""
Write-Host "Built: $exe" -ForegroundColor Green

# 4. Install.
if ($Install) {
    $dir = Join-Path $env:LOCALAPPDATA "Programs\Spotifast"
    $dest = Join-Path $dir "spotifast.exe"
    Step "Install to $dir"
    Get-Process spotifast -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 500
    New-Item -ItemType Directory -Force $dir | Out-Null
    if ((Test-Path $dest) -and -not (Test-Path "$dest.original")) {
        Copy-Item $dest "$dest.original"
        Write-Host "Saved the earlier program as $dest.original"
    }
    Copy-Item $exe $dest -Force
    $shell = New-Object -ComObject WScript.Shell
    $link = $shell.CreateShortcut((Join-Path ([Environment]::GetFolderPath("Programs")) "Spotifast.lnk"))
    $link.TargetPath = $dest
    $link.Save()
    Write-Host "Installed. Open Spotifast from the Start menu." -ForegroundColor Green
}
