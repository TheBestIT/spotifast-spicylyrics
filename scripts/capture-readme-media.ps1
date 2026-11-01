<#
.SYNOPSIS
  Make the README screenshots and the demo video of the fork from demo mode.

.DESCRIPTION
  The script starts a demo build of Spotifast once for each image. Each run
  uses the built-in flag --demo-shot, which writes the frame buffer of the
  window to a PNG file and stops the program. Thus the capture does not
  depend on the screen, the window position or other windows.

  The demo build needs the cargo feature "demo". Build it first, in Git Bash:

    source /d/Projects/spotifast-spicylyrics/env.sh
    CARGO_TARGET_DIR=target/demo cargo build --release --no-default-features --features demo

  The default -Exe is that release demo build. A debug demo build also
  works for the images, but it writes only about 6 frames each second, so
  the video is not smooth.

  For the video, the program writes a sequence of frames. Then ffmpeg makes
  demo.mp4 and demo.gif from the frames, with the real time of each frame.

  The script writes the files to docs\spicy by default. Look at each file
  before you commit it.

.PARAMETER Exe
  The demo build of spotifast.exe. The default is
  target\demo\x86_64-pc-windows-gnu\release\spotifast.exe.

.PARAMETER OutDir
  The folder for the files. The default is docs\spicy.

.PARAMETER Only
  The names of the files to make, for example karaoke,romaji. The names are
  karaoke, romaji, romaji-modes, duet, duet-panel, panel, setup,
  setup-working and video. The default is all. The name romaji-modes makes
  one image with the 4 romaji modes in a 2x2 grid.

.PARAMETER Frames
  The number of frames for the video. The default is 330 (about 11 s).

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\capture-readme-media.ps1

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\capture-readme-media.ps1 -Only romaji -OutDir $env:TEMP\shots
#>
param(
    [string]$Exe,
    [string]$OutDir,
    [string[]]$Only = @("karaoke", "romaji", "romaji-modes", "duet", "duet-panel", "panel", "setup", "setup-working", "video"),
    [int]$Frames = 330
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)
if (-not $Exe) { $Exe = Join-Path $root "target\demo\x86_64-pc-windows-gnu\release\spotifast.exe" }
if (-not $OutDir) { $OutDir = Join-Path $root "docs\spicy" }
# A comma list from the command line arrives as one string.
$Only = $Only | ForEach-Object { $_ -split "," } | ForEach-Object { $_.Trim() } | Where-Object { $_ }

if (-not (Test-Path $Exe)) { throw "The program is not found at $Exe. Build it with --features demo." }
$help = & $Exe --help 2>&1 | Out-String
if ($help -notmatch "--demo-shot") { throw "$Exe has no demo mode. Build it with --features demo." }
New-Item -ItemType Directory -Force $OutDir | Out-Null

# A release build is a Windows GUI program, so the call operator does not
# wait for it. Start-Process -Wait waits, and gives the exit code.
function Invoke-Demo([string[]]$Arguments) {
    $quoted = $Arguments | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }
    $process = Start-Process -FilePath $Exe -ArgumentList $quoted -Wait -PassThru -NoNewWindow
    return $process.ExitCode
}

# The name of each image, and the demo surface that it shows.
$shots = [ordered]@{
    "karaoke"       = "lyrics-fullscreen-view"
    "romaji"        = "romaji"
    "duet"          = "duet,kanji-on-top"
    "duet-panel"    = "duet-panel"
    "panel"         = "lyrics"
    "setup"         = "spicy-lyrics"
    "setup-working" = "spicy-lyrics-working"
}

foreach ($name in $shots.Keys) {
    if ($Only -notcontains $name) { continue }
    $file = Join-Path $OutDir "$name.png"
    Write-Host "==> $name.png ($($shots[$name]))" -ForegroundColor Cyan
    # The demo track starts at 1:23. After 3 s it is at 1:26, where the
    # active line is half sung in both sample lyrics.
    $code = Invoke-Demo @("--demo-show", $shots[$name], "--demo-size", "1280x800", "--demo-shot", $file, "--demo-shot-delay", "3000")
    if ($code -ne 0 -or -not (Test-Path $file)) { throw "The capture of $name.png failed (exit code $code)." }
}

if ($Only -contains "romaji-modes") {
    # Capture the 4 modes, then join them in a 2x2 grid with a small label.
    Add-Type -AssemblyName System.Drawing
    $modeDir = Join-Path ([System.IO.Path]::GetTempPath()) "spotifast-modes-$PID"
    New-Item -ItemType Directory -Force $modeDir | Out-Null
    $modes = [ordered]@{
        "romaji-on-top" = "Romaji on top"
        "kanji-on-top"  = "Kanji on top"
        "kanji-only"    = "Kanji only"
        "romaji-only"   = "Romaji only"
    }
    $cellW = 800; $cellH = 500
    $grid = New-Object System.Drawing.Bitmap ($cellW * 2), ($cellH * 2)
    $g = [System.Drawing.Graphics]::FromImage($grid)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $font = New-Object System.Drawing.Font "Segoe UI Semibold", 15
    $i = 0
    foreach ($mode in $modes.Keys) {
        Write-Host "==> mode $mode" -ForegroundColor Cyan
        $file = Join-Path $modeDir "$mode.png"
        $code = Invoke-Demo @("--demo-show", "romaji,$mode", "--demo-size", "1280x800", "--demo-shot", $file, "--demo-shot-delay", "3000")
        if ($code -ne 0 -or -not (Test-Path $file)) { throw "The capture of $mode failed (exit code $code)." }
        $img = [System.Drawing.Image]::FromFile($file)
        $x = ($i % 2) * $cellW; $y = [math]::Floor($i / 2) * $cellH
        $g.DrawImage($img, $x, $y, $cellW, $cellH)
        $img.Dispose()
        $label = $modes[$mode]
        $size = $g.MeasureString($label, $font)
        $px = $x + 10; $py = $y + 6
        $g.FillRectangle((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 0, 0, 0))), $px, $py, ($size.Width + 12), ($size.Height + 10))
        $g.DrawString($label, $font, [System.Drawing.Brushes]::White, ($px + 6), ($py + 5))
        $i++
    }
    # A thin line between the cells.
    $pen = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 40, 40, 40)), 2
    $g.DrawLine($pen, $cellW, 0, $cellW, $cellH * 2)
    $g.DrawLine($pen, 0, $cellH, $cellW * 2, $cellH)
    $g.Dispose()
    $grid.Save((Join-Path $OutDir "romaji-modes.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $grid.Dispose()
    Remove-Item -Recurse -Force $modeDir
}

if ($Only -contains "video") {
    $ffmpeg = (Get-Command ffmpeg -ErrorAction SilentlyContinue).Source
    if (-not $ffmpeg) { throw "ffmpeg is not found. Install it with: winget install Gyan.FFmpeg" }
    $frameDir = Join-Path ([System.IO.Path]::GetTempPath()) "spotifast-demo-frames-$PID"
    Write-Host "==> $Frames frames in $frameDir" -ForegroundColor Cyan
    $code = Invoke-Demo @("--demo-show", "lyrics-fullscreen-view", "--demo-size", "1440x900", "--demo-shot", $frameDir, "--demo-shot-frames", "$Frames")
    if ($code -ne 0) { throw "The frame capture failed (exit code $code)." }

    # Each frame is frame-<number>-<milliseconds>.png. Give each frame its
    # real length in an ffmpeg concat list.
    $list = Get-ChildItem $frameDir -Filter "frame-*.png" | ForEach-Object {
        $parts = $_.BaseName -split "-"
        [pscustomobject]@{ Path = $_.FullName; Number = [int]$parts[1]; Ms = [int]$parts[2] }
    } | Sort-Object Number
    if ($list.Count -lt 2) { throw "The program wrote fewer than 2 frames." }
    $lines = New-Object System.Collections.Generic.List[string]
    for ($i = 0; $i -lt $list.Count; $i++) {
        $next = if ($i + 1 -lt $list.Count) { $list[$i + 1].Ms } else { $list[$i].Ms + 33 }
        $seconds = [math]::Max(($next - $list[$i].Ms), 1) / 1000.0
        $lines.Add("file '$($list[$i].Path -replace '\\', '/')'")
        $lines.Add("duration $($seconds.ToString([System.Globalization.CultureInfo]::InvariantCulture))")
    }
    # The concat format uses the last file again for its duration.
    $lines.Add("file '$($list[-1].Path -replace '\\', '/')'")
    $concat = Join-Path $frameDir "frames.txt"
    [System.IO.File]::WriteAllLines($concat, $lines)

    $mp4 = Join-Path $OutDir "demo.mp4"
    $gif = Join-Path $OutDir "demo.gif"
    Write-Host "==> demo.mp4" -ForegroundColor Cyan
    & $ffmpeg -y -loglevel error -f concat -safe 0 -i $concat -vf "fps=30,format=yuv420p" -c:v libx264 -crf 20 -movflags +faststart $mp4
    if ($LASTEXITCODE -ne 0) { throw "ffmpeg could not make demo.mp4." }
    Write-Host "==> demo.gif" -ForegroundColor Cyan
    & $ffmpeg -y -loglevel error -i $mp4 -vf "fps=15,scale=900:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4" $gif
    if ($LASTEXITCODE -ne 0) { throw "ffmpeg could not make demo.gif." }
    Remove-Item -Recurse -Force $frameDir
}

Write-Host "The files are in $OutDir." -ForegroundColor Green
