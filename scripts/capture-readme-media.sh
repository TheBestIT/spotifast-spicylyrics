#!/usr/bin/env bash
# Make the README media of the fork on Linux (Xvfb, ImageMagick, ffmpeg).
# Same steps as scripts/capture-readme-media.ps1.
# Usage: scripts/capture-readme-media.sh [OUT_DIR] [EXE]
# Build the demo first: cargo build --release --features demo
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-$root/docs/spicy}"
exe="${2:-$root/target/release/spotifast}"
work="$(mktemp -d "${TMPDIR:-/tmp}/spicy-media.XXXXXX")"
trap 'rm -rf "$work"' EXIT
mkdir -p "$out"
demo() { xvfb-run -a -s "-screen 0 1600x1000x24" "$exe" "$@"; }
shot() { # name surface
  demo --demo-show "$2" --demo-size 1280x800 --demo-shot "$work/$1.png" --demo-shot-delay 3000
  test -s "$work/$1.png"
}
shot karaoke lyrics-fullscreen-view
shot romaji romaji
shot duet duet,kanji-on-top
shot duet-panel duet-panel
shot panel lyrics
shot setup spicy-lyrics
shot setup-working spicy-lyrics-working
# The 4 romaji modes in a 2x2 grid of 800x500 cells, each with a label.
i=0
for m in "romaji-on-top:Romaji on top" "kanji-on-top:Kanji on top" "kanji-only:Kanji only" "romaji-only:Romaji only"; do
  mode="${m%%:*}"; label="${m#*:}"
  shot "mode-$mode" "romaji,$mode"
  convert "$work/mode-$mode.png" -resize 800x500! -fill black -draw "rectangle 10,6 230,44" \
    -font DejaVu-Sans-Bold -fill white -pointsize 20 -annotate +18+32 "$label" "$work/cell$i.png"
  i=$((i + 1))
done
montage "$work"/cell0.png "$work"/cell1.png "$work"/cell2.png "$work"/cell3.png \
  -tile 2x2 -geometry 800x500+0+0 -background '#282828' "$work/romaji-modes.png"
# The video: 330 frames at 1440x900, then mp4 (30 fps) and gif (900 px, 15 fps).
demo --demo-show lyrics-fullscreen-view --demo-size 1440x900 --demo-shot "$work/frames" --demo-shot-frames 330
python3 -I - "$work/frames" > "$work/frames.txt" <<'P'
import os, sys
d = sys.argv[1]
fr = sorted((int(f.split("-")[1]), int(f.split("-")[2].split(".")[0]), f)
            for f in os.listdir(d) if f.startswith("frame-"))
for i, (_, ms, f) in enumerate(fr):
    nxt = fr[i + 1][1] if i + 1 < len(fr) else ms + 33
    print(f"file '{d}/{f}'\nduration {max(nxt - ms, 1) / 1000:.4f}")
print(f"file '{d}/{fr[-1][2]}'")
P
ffmpeg -y -loglevel error -f concat -safe 0 -i "$work/frames.txt" -vf "fps=30,format=yuv420p" \
  -c:v libx264 -crf 20 -movflags +faststart "$work/demo.mp4"
ffmpeg -y -loglevel error -i "$work/demo.mp4" \
  -vf "fps=15,scale=900:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4" "$work/demo.gif"
mkdir -p "$out/new"
cp "$work"/{karaoke,romaji,romaji-modes,duet,duet-panel,panel,setup,setup-working}.png "$work"/demo.mp4 "$work"/demo.gif "$out/new/"
echo "The new files are in $out/new. Look at each one, then move them over the old files."
