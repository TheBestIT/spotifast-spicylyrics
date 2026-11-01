# Spotifast Spicy Lyrics: context for coding agents

Read this file first. It tells you how to work on this fork. Update it at the
end of each task (see "How to update this file").

Last update: 2026-11-01. Version in `Cargo.toml`: `0.15.0-spicy.2`.

## 1. Purpose and fork rules

This repository is a fork of Spotifast (`crmne/spotifast`). Spotifast is a
native Spotify client in Rust with egui. The fork adds Spicy Lyrics karaoke
lyrics, romaji for Japanese songs and a key setup menu. All other parts stay
the same as upstream.

- The fork repository is `TheBestIT/spotifast-spicylyrics` (git remote `fork`).
- The upstream repository is `crmne/spotifast` (git remote `origin`).
- The working branch is `main`. Push to `fork`. Never push to `origin`.
- Change only the lyrics features, the setup menu, the update source, the
  release workflows and the package metadata. Keep other upstream code as it is.
- Keep the upstream rules in `AGENTS.md` and `CONTRIBUTING.md`. This file adds
  the fork rules. Where they differ, the fork rules in this file apply.
- Upstream releases come into the fork with a merge commit
  ("Merge upstream vX.Y.Z"). This is the only exception to the AGENTS.md rule
  "no merge commits". Do not make other merge commits.
- The fork deleted some upstream workflows (triage, board, docs, Flatpak,
  Soloist preview). Keep them deleted after a merge of upstream.
- The fork has its own update signature key. The app accepts updates only
  from fork releases (`src/updates.rs`).

### Version rule

- The version is `<upstream version>-spicy.<N>`, for example `0.15.0-spicy.1`.
- `0.15.0` is the upstream version. Do not change it, except in a merge of a
  new upstream release. Then set N to 1.
- For a fork release on the same upstream version, increase only N.
- Tags have the form `v0.15.0-spicy.1`. The tag must be equal to the
  `Cargo.toml` version. The release workflow stops when they differ.
- The workflow `.github/workflows/upstream-watch.yml` checks upstream each day.
  It opens an issue when upstream has a newer release.

### Commit rules

- Use the name `TheBestIT` and the email
  `75973639+TheBestIT@users.noreply.github.com`.
- Do not add `Co-Authored-By` lines, Claude lines or "Generated with" lines.
- Write one topic in one commit. Each commit must compile and pass the tests.
- Write commit messages, comments and docs in ASD-STE100 Simplified
  Technical English. The README and the release notes use this style too.
- Do not commit, tag or push without an instruction from the user.

## 2. Folder layout on this computer

The parent folder is `D:\Projects\spotifast-spicylyrics`.

| Path | Content |
| --- | --- |
| `spotifast-src\` | This git repository. |
| `build.ps1` | The local build script (GNU target, no MilkDrop). `-Install` copies the program over the installed Spotifast. |
| `env.sh` | The build environment for Git Bash. Source it before each cargo command. |
| `secrets\` | The update signature keys of the fork (`update-signing-key.pem`, `update-public-key.hex`). Do not read them, copy them or put them in a commit. |
| `README.md` | An old local note about the build. The real README is in `spotifast-src`. |
| `test.log` | An old log of a test build of 0.14.1-spicy.1. It has only one line. You can ignore it. |
| `target-test\` | An old, almost empty target folder from 0.14.1. You can ignore it. |

The sibling folder `D:\Projects\spotifast-ios` holds the iOS client
(SwiftUI). Its lyrics views are the visual reference for this fork:
`ios/Spotifast/Views/Lyrics/KaraokeLineView.swift` and
`LyricsContentView.swift`.

### Secrets and keys

- **Spicy Lyrics key.** The app reads it at run time, not at build time. The
  `SPICY_LYRICS_KEY` environment variable comes first. Then the app reads
  `%APPDATA%\paolino\spotifast\config\spicy_lyrics_key.txt`. That file is
  present on this computer. Without a key, the Spicy Lyrics source is off.
  Spotify and LRCLIB lyrics still work. Demo mode needs no key.
- **Update signature key.** The private key is in GitHub as the secret
  `SPOTIFAST_UPDATE_SIGNING_KEY`. The release workflows use it. A local copy
  is in `..\secrets\`. The public key is in `assets/update-public-key.hex`.
- Never write a key value in a file, a log, a commit or a chat answer.

## 3. Repository map

### Fork files

| File | What it does |
| --- | --- |
| `src/lyrics.rs` | The lyrics model (`Lyrics`, `Line`, `Syllable`) and the parsers. `from_spicy` reads the Spicy Lyrics JSON. `Line` has `syllables`, `background` (background vocals), `opposite` (second singer) and `romanized`. It also has the LRCLIB client and the disk cache. |
| `src/romaji.rs` | Romaji for Japanese lines. It uses lindera with the IPADIC dictionary and wana_kana. `fill` adds romaji to lines that have none. |
| `src/ui/lyrics.rs` | Draws the lyrics: the side panel (`side_panel`, line loop in `contents`) and the full screen view (`fullscreen`, line loop in `fullscreen_contents`). The README calls the full screen view in the window "the cinema view". It uses the same code. Both line loops call `lyric_line`. `romanize_button` (the Romaji menu of the 4 script modes) and `spicy_button` are the header buttons. `script_label` gives the mode names. |
| `src/ui/lyric_line.rs` | One lyrics line as a block of sub-lines: the main line, the smaller, lighter secondary line (the other script) and the italic background vocal line. A line of the second singer aligns right. The port of the iOS `KaraokeLineView`. See "The line renderer" below. |
| `src/bidi.rs` | Upstream file. The fork made `has_rtl` public for the line renderer. Make no other change here. |
| `src/ui/dialogs.rs` | The Spicy Lyrics setup dialog (`Dialog::SpicyLyrics`) and `spicy_status`. |
| `src/ui/settings.rs` | The "Spicy Lyrics" section in Settings (near line 2300): the key status and the "Romaji for Japanese lyrics" row with the mode menu (`lyrics_script_picker`). |
| `src/settings.rs` | The saved settings. `lyrics_script: LyricsScript` keeps the script mode. `lyrics_romanize` is the old switch. It stays in the file for older versions. Change the mode only with `set_lyrics_script` (see section 10). |
| `src/model.rs` | `SpicyKeyStatus`, the `Action`s of the setup and `Action::SetLyricsScript`. |
| `src/app.rs` | The application state. It applies the actions (`OpenSpicyLyricsSetup`, `CheckSpicyKey`, `SaveSpicyKey`, `RemoveSpicyKey`) and the `Event::SpicyKey` events. It has about 420 inline tests. |
| `src/backend.rs` | The network runtime. `fetch_lyrics` asks the sources in this order: local file, Spicy Lyrics, Spotify, LRCLIB. `handle_spicy_key`, `spicy_key`, `spicy_env_key` and `check_spicy_key` handle the key. |
| `src/demo.rs` | Demo mode: sample data and the `--demo-show` surfaces. `sample_japanese_lyrics`, `sample_duet_lyrics` and `sample_lyrics` are the demo lyrics. It also has about 160 UI tests. |
| `src/entrypoint.rs` | The command line. It has the `--demo*` flags and the frame capture (`--demo-shot-frames`). |
| `src/updates.rs` | The update source: the fork releases and the fork key. |
| `docs/spicy/` | The README media (`karaoke.png`, `romaji.png`, `romaji-modes.png`, `duet.png`, `duet-panel.png`, `panel.png`, `setup.png`, `setup-working.png`, `demo.gif`, `demo.mp4`) and this file. |
| `docs/UPSTREAM-README.md` | The upstream README. |
| `scripts/build-windows.ps1`, `scripts/build-linux.sh` | The public build scripts that the README tells users to run. |
| `scripts/capture-readme-media.ps1` | Makes the README images and the demo video from demo mode (see section 7). |
| `packaging/release-notes/v*-spicy.*.md` | The fork release notes. |
| `packaging/flatpak/rocks.spotifast.Spotifast.metainfo.xml` | The Flatpak release list. Each fork release has an entry. |
| `.github/workflows/` | `ci.yml` (fork CI), `release.yml` (tag to release), `packages.yml` (Linux packages), `upstream-watch.yml` (daily upstream check). |

### Tests

- `tests/*.rs`: integration tests from upstream (branding, localization,
  packaging, tray, start-up). The fork adds no file here.
- Inline `#[cfg(test)]` modules: the fork tests are in `src/lyrics.rs` (12),
  `src/romaji.rs` (7), `src/ui/lyric_line.rs` (11), `src/ui/lyrics.rs` (2)
  and `src/settings.rs`
  (`the_old_romaji_switch_loads_as_a_lyrics_script`). `src/demo.rs` and
  `src/app.rs` have UI tests that use the `demo` feature.

### The line renderer (`src/ui/lyric_line.rs`)

- `parts(line, mode)` gives the sub-lines from top to bottom. Each `Part`
  has a `Script` (`Original` or `Romaji`) and a `Role` (`Main`,
  `Secondary` or `Background`). A line without `romanized` has only
  `Original`/`Main`. A line with `background` syllables gets one more
  `Background` part at the end. It is `Romaji` when the mode shows romaji
  and a background syllable has romaji.
- `part_flow(lyrics, index, part)` gives the flow of any part. The
  background flow comes from `joined` on `line.background`, with the times
  of the background syllables.
- `flow(lyrics, index, script)` gives the text of one sub-line and its timed
  `Piece`s (character ranges). The romaji pieces use the times of the
  original syllables. The text is `line.text` or `line.romanized`. The code
  finds each syllable in it in order. If it fails, `joined` builds the text
  from the syllables (no space between two CJK characters). A line without
  syllables splits its time by word length (`split_line_time`).
- `layout` measures the block (one galley for each sub-line, in
  `layout_part`). `paint` draws it. `show` allocates the full width and
  paints. Each galley fills from its own pieces (`paint_fill`).
- `aligns_right(line)` is true for `line.opposite` or a right-to-left line.
  Then each left-to-right galley has `halign = RIGHT`, thus each wrapped
  row is flush right. `paint` uses `crate::bidi::galley_pos`.
- `Style` has `background` (the font, semibold) and `background_gap` (the
  extra space above the background line). `layout_part` sets
  `TextFormat::italics` for the background. On the right, `paint` moves an
  italic galley left by `ITALIC_LEAN` (0.2 of the font size), because
  epaint skews the glyphs out of the galley. `paint_fill` adds the same
  lean to the right edge of the sung rows, thus the last glyph of a sung
  italic row gets the full sung colour.
- `layout` runs for each line in each frame. The galleys come from the
  egui galley cache, as before T3. The text and the pieces (`flow`) are
  calculated again in each frame. This is a small cost for each line.
- The secondary line and the background line use 0.8 opacity, except
  their sung part.
- Text with a right-to-left letter goes through `crate::bidi::layout`. A
  right-to-left sub-line has no fill and no italics.

## 4. Build

This computer has no Visual Studio. Use the Rust GNU target and the MinGW
linker at `C:\ProgramData\mingw64\mingw64\bin`. MilkDrop needs libclang and
GLEW, which are not installed. Thus always use `--no-default-features`.

The toolchain is pinned in `rust-toolchain.toml` (1.98.0).

### Release build (PowerShell)

```powershell
cd D:\Projects\spotifast-spicylyrics
powershell -ExecutionPolicy Bypass -File .\build.ps1
```

The program is then at
`spotifast-src\target\x86_64-pc-windows-gnu\release\spotifast.exe`.
Add `-Install` to replace the installed program at
`%LOCALAPPDATA%\Programs\Spotifast\spotifast.exe`. The script keeps a backup.

### Release build (Git Bash)

```sh
source /d/Projects/spotifast-spicylyrics/env.sh
cd /d/Projects/spotifast-spicylyrics/spotifast-src
cargo build --release --no-default-features
```

`env.sh` sets `CARGO_BUILD_TARGET=x86_64-pc-windows-gnu` and
`CARGO_TARGET_DIR=.../spotifast-src/target`. Thus all output goes to
`target\x86_64-pc-windows-gnu\`.

### Demo builds

The `demo` feature adds sample data and the `--demo*` flags. Never put it in
a release build for users.

Debug demo build (for quick checks):

```sh
source /d/Projects/spotifast-spicylyrics/env.sh
cd /d/Projects/spotifast-spicylyrics/spotifast-src
cargo build --no-default-features --features demo
```

The program is then at `target\x86_64-pc-windows-gnu\debug\spotifast.exe`.

Release demo build (for the README images and the video):

```sh
source /d/Projects/spotifast-spicylyrics/env.sh
cd /d/Projects/spotifast-spicylyrics/spotifast-src
CARGO_TARGET_DIR=target/demo cargo build --release --no-default-features --features demo
```

The program is then at
`target\demo\x86_64-pc-windows-gnu\release\spotifast.exe`. The separate
target folder keeps the user release program free of the demo feature.

### Measured results (2026-11-01, this computer)

| Command | Result | Time |
| --- | --- | --- |
| `cargo build --release --no-default-features` | Pass. `spotifast.exe` is about 81 MB. | 7 min 15 s |
| `cargo build --no-default-features --features demo` | Pass. | 3 min 17 s (after the test build) |
| `CARGO_TARGET_DIR=target/demo cargo build --release --no-default-features --features demo` | Pass. `--version` gives `spotifast 0.15.0-spicy.2` (checked after T3, T4 and T6). | 8 min 1 s (cold target folder). 6 min 50 s for the rebuild in T5. |

The release profile uses thin LTO and one codegen unit. Thus the last step
of a release build takes several minutes, also after a small change.

## 5. Tests

Run the tests with the GNU target and the `demo` feature:

```sh
source /d/Projects/spotifast-spicylyrics/env.sh
cd /d/Projects/spotifast-spicylyrics/spotifast-src
cargo test --no-default-features --features demo --lib
```

Result on 2026-11-01 after T8: 1498 passed, 0 failed, 3 ignored. The tests
took 8 s. Before T3, a cold run took 6 min 14 s (5 min 51 s to compile).
The 3 ignored tests are upstream tests:
`credentials::tests::native_store_round_trip` (needs the system credential
store), `soloist::install::tests::official_windows_archive_extracts` (needs
`SOLOIST_QA_ZIP`) and `soloist::tests::native_start_and_stop` (needs
`SOLOIST_QA_KEY`). Do not run them for lyrics work. No test failed or was
flaky in this run. No flaky test is known at this time.

To run only the lyrics, line renderer and romaji tests (about 1 s after a
build):

```sh
cargo test --no-default-features --features demo --lib -- lyrics:: lyric_line:: romaji:: lyrics_script
```

Notes:

- The `tests/` integration tests are not in the command above. They
  test upstream parts (packaging, tray, start-up). Run
  `cargo test --no-default-features --features demo --all-targets` to
  include them. Result on 2026-11-01 (T8): all passed. The lib had 1498
  passed and 3 ignored. The `spotifast` program had 16 passed.
  `branding` 3, `localization` 1, `packaging_icons` 1, `packaging_macos` 1
  and `update_launch` 2 passed. `omarchy`, `startup_diagnostics`, `tray`
  and the examples have no test on Windows. The compile took 10 min.
- `cargo test` and `cargo build` use the same target folder. Do not run
  two cargo commands at the same time in this checkout. Cargo blocks the
  second command until the first one stops.

Before a commit, also run these checks:

```sh
cargo fmt --all --check
cargo clippy --no-default-features --features demo --all-targets -- -D warnings
```

Result on 2026-11-01 after T8: `cargo fmt --all --check` passed. The clippy
command passed with no warnings in 1 min 47 s.

The fork CI (`.github/workflows/ci.yml`) runs `cargo fmt --check`,
`cargo clippy --all-targets --features demo -- -D warnings` and
`cargo test --all-targets --features demo` on Linux. On Windows it runs
`cargo test --lib --features demo` with MSVC and MilkDrop. It also runs the
two public build scripts.

## 6. Demo mode

Demo mode starts the app with sample data and no Spotify connection. It needs
the demo build (section 4). It does not touch the real sign-in. It keeps its
data in a temporary folder.

Start it from the `spotifast-src` folder:

```powershell
.\target\x86_64-pc-windows-gnu\debug\spotifast.exe --demo --demo-show romaji
```

The sample track is "Rosewood" by Bonobo. It plays at 1:23 (83 000 ms) of
3:00. The demo sets the key status to "Working", so no key is necessary.

### Surfaces for the fork

Give one or more surfaces to `--demo-show`, separated by commas.

| Surface | What it shows |
| --- | --- |
| `lyrics` | The main window with the lyrics side panel. The lyrics are English sample lines with line timing. |
| `lyrics-fullscreen` | The same lyrics in real full screen. |
| `lyrics-fullscreen-view` | The full screen lyrics view, drawn in the window at its size. Use it for screenshots. |
| `lyrics-fullscreen-instrumental` | The full screen view of an instrumental track. |
| `romaji` | The full screen view with the Japanese sample lyrics (`sample_japanese_lyrics`), in the mode "Kanji on top". Each syllable has its own time and romaji. |
| `romaji-on-top`, `kanji-on-top`, `kanji-only`, `romaji-only` | Set the lyrics script mode. Put the mode after `romaji` or `duet`, for example `romaji,romaji-only`. |
| `duet` | The full screen view (drawn in the window) with the duet sample (`sample_duet_lyrics`): lead lines on the left, second singer lines on the right, background vocals in italics. Two lines are Japanese with romaji, and one has a Japanese background with romaji. The mode is "Kanji only". Use `duet,kanji-on-top` to see the romaji. |
| `duet-panel` | The main window with the duet sample in the side panel. |
| `spicy-lyrics` | The setup dialog with no key. |
| `spicy-lyrics-working` | The setup dialog after a key check that passed. |
| `light` | The light theme. Add it to another surface, for example `romaji,light`. |

The flag `--help` of the demo build lists all upstream surfaces.

### Japanese lyrics with kanji

The `romaji` surface sets the mode "Kanji on top". Add a mode surface to
start in another mode. Click the **Romaji** button in the header to open the
menu of the 4 modes. To change the start state, edit the `"romaji"` branch
in `apply_flags` in `src/demo.rs`.

### Other demo flags

| Flag | Use |
| --- | --- |
| `--demo-page <page>` | Open a page, for example `home`, `settings`, `playlist:pl1`. |
| `--demo-size 1280x800` | Set the inner window size. |
| `--demo-shot <file.png>` | Write a PNG of the window and stop. It implies `--demo`. |
| `--demo-shot-delay <ms>` | Wait before the shot (default 6000 ms, for cover images). |
| `--demo-shot-frames <n>` | Write n frames to the folder of `--demo-shot`, as `frame-<number>-<ms>.png`. |
| `--demo-click X,Y` | Click at a point before the shot, for example to open a menu. |
| `--demo-language <locale>` | Set the interface language. |

## 7. Screenshots and the demo video for the README

The app draws with OpenGL, so a browser tool cannot capture it. Use the
built-in flag `--demo-shot`. The app writes its own frame buffer to a PNG
file. This method does not depend on the screen position or on other windows.

The script `scripts/capture-readme-media.ps1` does all the captures:

```powershell
cd D:\Projects\spotifast-spicylyrics\spotifast-src
powershell -ExecutionPolicy Bypass -File scripts\capture-readme-media.ps1
```

- It needs the release demo build at
  `target\demo\x86_64-pc-windows-gnu\release\spotifast.exe` (section 4).
  Use `-Exe` for another path.
- It writes to `docs\spicy` by default. Use `-OutDir` for another folder.
  Make a test run into a temporary folder first.
- Use `-Only romaji,karaoke` to make only some files.
- Each image is 1280x800, as in the current README. The script waits
  3000 ms (`--demo-shot-delay 3000`). The track is then at 1:26, and the
  active line is half sung.
- The video is 1440x900, 330 frames, about 11 s. The script uses ffmpeg
  (installed with winget on this computer) to make `demo.mp4` (30 fps) and
  a 900 px wide `demo.gif` (15 fps).
- A release build is a Windows GUI program. The PowerShell call operator
  (`&`) does not wait for it. The script uses `Start-Process -Wait`.
- A debug demo build writes only about 6 frames each second. Use it only
  for the still images.

| File | Surface |
| --- | --- |
| `karaoke.png` | `lyrics-fullscreen-view` |
| `romaji.png` | `romaji` (kanji on top, romaji under it) |
| `romaji-modes.png` | `romaji,<mode>` for the 4 modes, joined in a 2x2 grid (1600x1000) with a label on each cell |
| `duet.png` | `duet,kanji-on-top` |
| `duet-panel.png` | `duet-panel` |
| `panel.png` | `lyrics` |
| `setup.png` | `spicy-lyrics` |
| `setup-working.png` | `spicy-lyrics-working` |
| `demo.mp4`, `demo.gif` | `lyrics-fullscreen-view`, 330 frames |

The name `romaji-modes` in `-Only` makes the 4 mode captures in a temporary
folder. Then the script joins them with System.Drawing (no extra tool). To
get the 4 single files, run the 4 surfaces by hand, for example
`--demo-show romaji,romaji-only`.

Result on 2026-11-01 (T5): a full run into a temporary folder passed. It
made 8 images, `demo.mp4` (1440x900, 11.0 s, 328 KB) and `demo.gif`
(900x563, 1.6 MB). The files were copied to `docs\spicy`, except
`setup.png` and `setup-working.png`. Their look did not change, so the old
files stay. Sizes: `karaoke.png` 763 KB, `romaji.png` 739 KB,
`romaji-modes.png` 702 KB, `duet.png` 737 KB, `duet-panel.png` 610 KB,
`panel.png` 634 KB. All single images are 1280x800.

To capture one view by hand:

```powershell
.\target\x86_64-pc-windows-gnu\debug\spotifast.exe --demo-show romaji --demo-size 1280x800 --demo-shot $env:TEMP\romaji.png --demo-shot-delay 3000
```

The demo needs no Spotify sign-in and no network for the lyrics. Cover
images come from the network. Without a network, the covers are empty.

The fill of an English line in the demo goes fast. The demo lines have no
end time. Thus `split_line_time` in `src/ui/lyric_line.rs` fills at 90 ms
for each letter (1.2 s minimum). This is correct behaviour, not a fault.

Look at each new image before you commit it. Use the Read tool to see a PNG.
The README images are project media, so they can go in a commit. Do not
commit before/after review images. Upstream CI rejects them.

## 8. Release and version bump

Do these steps for a fork release, for example `0.15.0-spicy.2`. Commit
`ba7a714` ("Release 0.14.1-spicy.1") is a good example.

1. Set `version` in `Cargo.toml` to `0.15.0-spicy.2`.
2. Run a build. The build updates the version in `Cargo.lock`.
3. Add a `<release>` entry at the top of the `<releases>` list in
   `packaging/flatpak/rocks.spotifast.Spotifast.metainfo.xml`. Use the date
   and the URL of the fork tag.
4. Write `packaging/release-notes/v0.15.0-spicy.2.md`. Copy the form of
   `v0.15.0-spicy.1.md`: a summary, the demo GIF, `New` and `Fixed`
   sections, `Install`, `Thanks`, and the full changelog link. Use
   ASD-STE100.
5. Change the example tag in the comments of `src/updates.rs`,
   `.github/workflows/release.yml` and `.github/workflows/packages.yml`.
6. Change the README where it names the version (section "The fork and the
   original"), if necessary.
7. Run the tests and the checks. Commit with the message
   "Release 0.15.0-spicy.2".
8. With the user's approval only: push `main` to `fork`, wait for CI, then
   push the tag `v0.15.0-spicy.2`. The tag starts `release.yml`. It builds
   the Windows installer, the MSI, the ZIP and the Linux packages, and
   signs the update files.
9. Check the release page: the notes, the files and `checksums.txt`.

The upstream steps for the website, Homebrew and AUR do not apply to the fork.

## 9. README rules

- The README is for users. Write it in ASD-STE100.
- Keep the structure: title block, "What the fork adds", "Screenshots",
  "Install", "Set up Spicy Lyrics", "Romaji for Japanese songs", "Duets and background vocals", "Build from
  source", "How it works", "Services and privacy", "The fork and the
  original", "Report a problem", "Credits", "License".
- Update the README in the same change when a user-visible lyrics feature,
  setting, file or network access changes.
- Keep the images in `docs/spicy/` and link them with relative paths. Release
  notes use the raw GitHub URL of `main`.
- Every image needs an `alt` text that tells what the image shows.
- Say that the screenshots show demo mode with sample data.
- Do not change `docs/UPSTREAM-README.md`.
- Do not use em dashes (AGENTS.md rule).

## 10. Open work and current state

State on 2026-11-01: `HEAD` is `1c38a40` ("Give the AppImage a PNG icon").
Version `0.15.0-spicy.1` is released. All tasks (T1 to T8) are done. No
change is in a commit yet. The tag `v0.15.0-spicy.2` is NOT created.

What is left (each step needs the user's approval):

1. Commit the changes. Use one topic in one commit (section 1), for
   example: the context file and the capture script, the romaji modes
   (T3), the duet layout (T4), the README and the media (T5, T7), and
   "Release 0.15.0-spicy.2" (T6). Each commit must compile and pass the
   tests. `src/demo.rs` has the T3 and the T4 demo changes together.
2. Push `main` to `fork` and wait for CI.
3. Push the tag `v0.15.0-spicy.2` and check the release (section 8).
4. If the release date is not 2026-11-01, change the date in the Flatpak
   metainfo before the release commit.

Known facts:

- T3 replaced the romaji switch with `settings.lyrics_script`
  (`LyricsScript`: `RomajiOnTop`, `KanjiOnTop`, `KanjiOnly`, `RomajiOnly`).
  The default is `KanjiOnly`, as before. `Settings::load` calls
  `migrate_lyrics_script`: a file without `lyrics_script` and with
  `lyrics_romanize: true` loads as `RomajiOnly`, else as `KanjiOnly`. An
  unknown mode loads as the default, and the other settings still load.
- The file also keeps `lyrics_romanize`. Its value is
  `LyricsScript::romaji_is_main`. Thus 0.15.0-spicy.1 and older read a file
  of the new version and keep the romaji. Change the mode only with
  `Settings::set_lyrics_script`, which sets the two fields (T8 fix).
- The action is `Action::SetLyricsScript(mode)`. The **Romaji** pill in the
  header opens a menu of the 4 modes. Settings has a menu in the row
  "Romaji for Japanese lyrics". The new UI strings are not in the
  translation catalogs (as the other fork strings).
- The active line in both views fills syllable by syllable through
  `lyric_line`. The romaji line fills with the times of the original
  syllables. The panel keeps its old look (regular, bold when active, accent
  fill). Full screen: main line bold at `size`, secondary line regular at
  `size * 20 / 28`, gap `size * 6 / 28`. The secondary line has 0.8 opacity
  except its sung part.
- The CJK fallback font has one weight. Thus a kanji main line is not bold.
  Only its Latin words are bold.
- T4 draws `Line.opposite` (right side) and `Line.background` (italic
  sub-line) in both views. See "The line renderer" in section 3.
- Sizes. Panel: background 14 semibold italic, 4 + 1.5 points above it.
  Full screen: background `size * 20 / 28`, `size * (6 + 2) / 28` above it.
- A click anywhere on the full width of a line seeks to the line. Before
  T3, only a click on the text did this.
- A new field in `Settings` fails the upstream "new settings" check. The fork
  CI does not run that check, but keep old settings files readable.

Known limits (not fixed, low risk):

- The active line comes from the lead times only. A background syllable
  after the start of the next line does not fill.
- `romaji::fill` does not make romaji for `line.background`. A Japanese
  background without `TransliteratedText` shows kanji in all modes (as iOS).
- When the syllables are not in the line text in order, the line shows the
  text that `joined` builds from the syllables, not `line.text`.
- In `joined`, a syllable that has only spaces removes the space between
  the words around it.
- A sub-line with right-to-left letters in a left-to-right paragraph moves
  to the right edge as one block. Its wrapped rows are not flush right.
- An italic background line on the left can go about 3 points (panel) past
  the right edge of the column when it wraps at the full width.
- The new strings ("Romaji on top", "Kanji on top", "Kanji only",
  "Romaji only", "Choose how the lyrics show the romaji" and the new
  Settings detail) show in English in all languages.

T8 review result: two bugs found and fixed, each with a regression test.

- The settings file of the new version had no `lyrics_romanize`. Thus
  0.15.0-spicy.1 showed kanji after a downgrade from a romaji mode. Test:
  `the_old_romaji_switch_loads_as_a_lyrics_script`.
- The sung rows of a wrapped italic line stopped at the edge of the galley.
  The leaning part of the last glyph stayed in the unsung colour. Test:
  `a_sung_italic_row_fills_its_leaning_last_glyph`.
- A build does not change `Cargo.lock`. The metainfo XML is valid. The
  release notes follow the AGENTS.md form. `docs/spicy` has no review
  capture: the README names each image.

| Id | Task | State |
| --- | --- | --- |
| T1 | Write this context file | Done (not in a commit) |
| T3 | Romaji display mode switch: romaji on top, kanji on top, kanji only, romaji only. | Done (not in a commit) |
| T4 | Singer side alignment (`opposite` to the right) and background vocal sub-lines, like the iOS client. | Done (not in a commit) |
| T5 | New screenshots and video with `scripts/capture-readme-media.ps1`. | Done (not in a commit). New: `romaji-modes.png` (1600x1000), `duet.png`, `duet-panel.png`. Changed: `karaoke.png`, `romaji.png`, `panel.png`, `demo.gif`, `demo.mp4` (11 s). |
| T6 | Bump the version to `0.15.0-spicy.2` (section 8). | Done (not in a commit). Tag NOT created. |
| T7 | Update the README for T3 and T4 (section 9). | Done (not in a commit) |
| T8 | Review all changes: tests, clippy, fmt, images, README. | Done. See "T8 review result". |

## 11. How to update this file

Update this file at the end of each task, before the final report.

- Change the state of the task in section 10. Add new open work there.
- Change the "Last update" date and the version at the top.
- When you change a build or test command, run it and write the real result.
- When you add a fork file, add it to the map in section 3.
- When you add a demo surface, add it to section 6.
- Remove facts that are no longer true. Do not keep a history here. Git has
  the history.
- Keep the ASD-STE100 style: short sentences, active voice, one instruction
  in one sentence.
