# VibeN64

VibeN64 is an N64 emulator. It is a fork of [Gopher64](https://github.com/gopher64/gopher64) by Logan McNaughton and the Gopher64 contributors, and the emulation is their work.

The fork exists for one reason: Gopher64 does not accept AI-assisted contributions, and VibeN64 does.

VibeN64 is not affiliated with or endorsed by the Gopher64 project.

- Please report VibeN64 problems [here](https://github.com/vibeN64/vibeN64/issues), not upstream.
- Please do not send VibeN64 changes to Gopher64 as pull requests. Their policy forbids it.

## what is different from Gopher64

- A redesigned interface: themed cards, buttons and switches in place of the stock widgets.
- Its own name, icon, app identifier and data folders, so it can sit beside a Gopher64 install without sharing settings or saves.
- No donation prompts in the app.
- Nintendo Switch Online N64 controllers work without setup: a built-in `nso-n64` input profile is used automatically for them.
- Builds with a stock Xcode toolchain on macOS (no separate LLVM install), and `scripts/bundle-macos.sh` assembles `VibeN64.app`.

Everything else follows upstream: homebrew support, upscaling, the CRT shader, CPU overclocking, cheats, savestates and RetroAchievements.

## what VibeN64 does not use

VibeN64 does not lean on services that the Gopher64 project runs or owns:

- **Netplay** is switched off by default. VibeN64 has no netplay server of its own and does not connect to Gopher64's. To play online, run the open source [gopher64-netplay-server](https://github.com/gopher64/gopher64-netplay-server) yourself and start VibeN64 with `NETPLAY_SERVER_URL` set to its address. This path has not been tested yet.
- **Discord Rich Presence** is switched off, because the Discord application it used belongs to Gopher64.
- Update checks, source links and problem reports all go to this repository.

The one remaining link is the About page's button to the [Gopher64 wiki](https://github.com/gopher64/gopher64/wiki), which still describes the controls accurately.

## platforms

VibeN64 is developed and tested on Apple Silicon Macs only. The Windows, Linux and Android code inherited from Gopher64 is still in the tree, but it is not built, tested or supported here.

## download

There are no pre-built releases yet. Build from source as described below.

## controls

Keys are mapped according to [these defaults](https://github.com/gopher64/gopher64/wiki/Default-Keyboard-Setup). Xbox-style controllers also have a [default mapping applied](https://github.com/gopher64/gopher64/wiki/Default-Gamepad-Setup).

## portable mode

If you would like to keep all the game data in the same folder as the executable, create a file called "portable.txt" in the same directory as the executable.

## building and usage

1. Install Xcode's command line tools (`xcode-select --install`)
2. [Install rust](https://www.rust-lang.org/tools/install)
3. `git clone --recursive https://github.com/vibeN64/vibeN64.git`
4. `cd vibeN64`
5. `git submodule update --init --recursive`
6. `cargo build --release`
7. `./target/release/viben64 /path/to/rom.z64`

### macOS app bundle

```
brew install molten-vk
./scripts/bundle-macos.sh
open target/VibeN64.app
```

The script finds `llvm-ar` inside the Rust toolchain, so nothing beyond Xcode's command line tools and Rust is needed to compile. When building by hand with plain `cargo build`, set `AR` to that `llvm-ar` yourself; the script shows how.

### working on the interface

Set `VIBEN64_PAGE` to open straight onto a sidebar page, counting from 0. For example `VIBEN64_PAGE=5 ./target/release/viben64` opens Settings.

## keeping up with Gopher64

Upstream fixes are merged in one direction only:

```
git remote add upstream https://github.com/gopher64/gopher64.git
git fetch upstream
git merge upstream/main
```

## contributing

AI-assisted and "vibe coded" pull requests are welcome. So are entirely hand-written ones.

- Say in the PR which tools you used, and what you tested and how.
- You are responsible for what you submit, however it was written. Run it before you send it.
- Keep each PR to one feature or fix.

## license

VibeN64 is licensed under the GPLv3 license, the same as Gopher64. Many portions of Gopher64 have been adapted from mupen64plus and/or ares. The license for mupen64plus can be found [here](https://github.com/mupen64plus/mupen64plus-core/blob/master/LICENSES). The license for ares can be found [here](https://github.com/ares-emulator/ares/blob/master/LICENSE).

## privacy

VibeN64 checks this repository's GitHub releases for a newer version when it starts. It makes no other network connections unless you turn on RetroAchievements or set up netplay with your own server.

If you enable the RetroAchievements feature, some data is sent to their systems. Please see their terms [here](https://retroachievements.org/terms).
