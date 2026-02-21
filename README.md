# TUI Music Player (Rust)

A terminal music player with:

- Directory browser
- Keyboard-driven controls
- Gapless playlist playback by enqueueing all files in the current folder to a single output sink

## Features

- Navigate directories and audio files from the current working directory.
- Open a directory or start playback from a selected file.
- When you start playback, the app enqueues all supported files in that folder (starting from the selected one), which allows continuous gapless transitions where source/decoder format permits it.
- Pause/resume, stop, and quit controls.

## Supported file types

- `mp3`
- `flac`
- `wav`
- `ogg`
- `m4a`

(Actual codec support depends on the audio backend and codec availability in `rodio`.)

## Build instructions

### Prerequisites

- Rust toolchain (stable) from [rustup](https://rustup.rs)
- A working audio output device

### Build

```bash
cargo build --release
```

Binary output:

```text
./target/release/tui-music-player
```

### Run

```bash
cargo run --release
```

Or run from a specific music directory:

```bash
cd /path/to/your/music
/path/to/your/repo/target/release/tui-music-player
```

## Controls

- `↑/↓` or `j/k`: Move selection
- `Enter` or `l`: Open directory / play selected file (and queue folder)
- `Backspace` or `h`: Go to parent directory
- `Space`: Pause/Resume
- `s`: Stop playback
- `q`: Quit

## Export to GitHub

If your local repository already has a remote set:

```bash
git add .
git commit -m "Add Rust TUI music player with directory browser and gapless queue playback"
git push origin <your-branch>
```

To connect a new GitHub repository:

```bash
git remote add origin git@github.com:<your-user>/<your-repo>.git
git branch -M main
git push -u origin main
```


## One-command publish helper

Use the included script to set/update `origin` and push your current branch:

```bash
./scripts/publish_to_github.sh git@github.com:<your-user>/<your-repo>.git
```

Or specify a branch explicitly:

```bash
./scripts/publish_to_github.sh git@github.com:<your-user>/<your-repo>.git main
```
