# Retro Control Server

🕹️ **Remote control server for Batocera/Linux** – Control mouse, keyboard, and gamepad from your Android phone with ultra-low latency (~1-5ms on LAN). Perfect for managing Batocera wirelessly and casual gamers or those wanting to experiment with emulated controls on Android over LAN.

## Key Features
- Low-latency binary protocol (UDP for mouse, TCP for keyboard/gamepad).
- Virtual devices via uinput, compatible with all emulators.
- Easy deployment: Static binary, systemd service included.

## Tech Stack
- Rust with Tokio async networking.
- uinput/evdev for virtual input devices.

## Installation
1. Clone the repo: `git clone https://github.com/KaltWulx/retro-control-server.git`
2. Build: `cargo build --release`
3. Copy to Batocera: `scp target/release/retro-control-server root@<IP_BATOCERA>:/userdata/system/`
4. Run: `./retro-control-server`

## Client
- Android client repository: https://github.com/KaltWulx/RetroControlClient.git

For the Android client, check the separate repo.

## License
MIT