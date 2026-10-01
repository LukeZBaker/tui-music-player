# Tempo

Tempo is a focused directory-browser music player for the GNOME desktop. It is
written in Rust with GTK 4 and libadwaita, and uses GStreamer for gapless
playback.

## Features

- Browse folders without importing or maintaining a music library.
- Shows only recognized audio files; files are validated by GStreamer before
  playback.
- Plays the rest of the current directory in sorted order.
- Queues the next URI from GStreamer's `about-to-finish` signal for gapless
  transitions.
- Reports inaccessible directories, unsupported files, and playback failures
  without crashing.

## Build

Install Rust and the development packages for GTK 4, libadwaita, GStreamer,
and the GStreamer playback/pbutils plugins. Then run:

```sh
cargo run
```

Audio codec support depends on the GStreamer plugin packages installed by your
distribution.

