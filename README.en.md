# UAD-RE · Volt Control

[简体中文](README.md) | **English**

Third-party control software for the **Universal Audio Volt 2 Gen 2**: set monitor and headphone volume from your desktop, and drive it all from a MIDI controller.

## Repository layout

| Path | Contents |
|------|----------|
| `docs/protocol.md` | Protocol research (UAMixerHelper's UASrv protocol, the parameter tree, findings verified on hardware; in Chinese) |
| `research/` | Research scripts (protocol probing, tree export, push monitoring, screenshot/click helpers) |
| `volt-control/` | The app: Tauri 2 + Rust backend, Svelte 5 + TypeScript frontend |

## How it works

`UAMixerHelper.exe` (installed with UAD Console; single-instance, keeps running after UAD Console is closed) serves the device's parameter tree on `127.0.0.1:4720`. No UA component reliably starts it at login (UA Connect only asks for it once, right after a driver install), so Volt Control launches it itself when the port refuses connections. UA Connect is not required. Volt Control is just another client of the helper: it reads and writes parameters and subscribes to changes, so it stays in sync with UAD Console and the front-panel controls without conflicting with either.

> The Volt has a single knob; its buttons choose what the knob controls. Monitor and headphone volume in the app are **the same parameters** as the knob, synced both ways in real time.
> Limits: which item the device screen is showing (the knob's focus) is not exposed to the computer, so the app cannot switch screen pages. Button LED brightness has no parameter either (only the screen brightness does).

## Features

- **Main**: monitor volume / mute / mono / Dim; headphone volume / mute; for inputs 1 and 2: gain, +48V, Tone, compressor, polarity invert, DIRECT monitoring, auto gain, plugged-jack type, stereo link
- **Mixer**: faders, pan, mute, solo and pre-fader sends for the monitor mix and CUE 1 / CUE 2, with live meters
- **MIDI**: right-click any control, then move your controller to map it. 7-bit and 14-bit CC (detected while learning), absolute knobs/faders, three relative encoder formats, toggle / momentary / trigger buttons. Volume-type controls use soft takeover by default (no jumps), and value changes are sent back to the controller (LED rings, motorized faders)
- **Settings**: sample rate, screen brightness, No-Clip, identify device, meter hold times, screen-off when idle; **volume safety limits** (apply to both the UI and MIDI)
- **Languages**: Simplified Chinese, Traditional Chinese, English, Japanese. Follows the Windows display language by default; can be changed in Settings and takes effect immediately
- Closing the window keeps it in the tray, still answering MIDI; starts in the tray at Windows sign-in; tray menu toggles monitor/headphone mute; reconnects automatically

## Build and run

```bash
cd volt-control
npm install
npx tauri dev      # development
npx tauri build    # installer: src-tauri/target/release/bundle/nsis/
```

Build release binaries with `npx tauri build`, not `cargo build --release`. A plain cargo build does not embed the frontend, and the window will try to load the dev server (`localhost refused to connect`).

Tests:

```bash
cd volt-control/src-tauri
cargo test                                   # unit tests
cargo test live_ -- --ignored --nocapture    # against real hardware: read-only discovery + harmless writes that are rolled back
```

Settings file: `%APPDATA%\com.knia.voltcontrol\config.json` (MIDI ports and mappings, volume limits, language).

## License

[MIT](LICENSE). This project is not affiliated with Universal Audio. Universal Audio, UA, Volt and UAD are its trademarks.
