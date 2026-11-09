<p align="center">
  <img src="assets/app-icon/icon-macos-1024.png" width="128" alt="Sophon icon">
</p>

<h1 align="center">Sophon</h1>

<p align="center">
  One brain, every device: a native client for your coding agents on macOS and iOS.
</p>

<p align="center">
  <a href="https://github.com/caoxing9/sophon/actions/workflows/ci.yml"><img src="https://github.com/caoxing9/sophon/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/caoxing9/sophon/releases/latest"><img src="https://img.shields.io/github/v/release/caoxing9/sophon" alt="Release"></a>
  <a href="README.zh-CN.md">简体中文</a>
</p>

---

Sophon is based on [Paseo](https://paseo.sh): your coding agents (Claude Code, Codex and others) run in its daemon on
your computer, and Sophon is the window onto them: start agents, follow their work, review the diffs, answer their questions, from your
Mac or from your phone, all talking to the same daemon. It is written in Rust with
[NSH](https://github.com/longbridge/nsh), so it is a real native app on each platform, not a web view.

> **The name.** In *The Three-Body Problem*, a sophon (智子) is a proton unfolded into a supercomputer: a speck of
> intelligence that is everywhere at once, kept in sync across any distance. Sophon does that for one person: the same
> agents, the same context, on every device you own.

## Features

- **Projects and workspaces**: local folders or git worktrees, new branches, cloning from GitHub; pin, rename, archive
- **Agents as tabs**: several agents per workspace; model, mode and thinking pickers; slash commands, `@` file mentions,
  image attachments; queue, steer or interrupt
- **Live timeline**: streaming output, tool calls, plans, permission and question cards, sub-agents, background work,
  rewind and fork
- **Review**: changes list, split/unified diffs, line comments sent back to the agent; commits; commit, push, pull,
  merge, stash; create and merge pull requests with their checks
- **Terminals and files**: a VS Code-style terminal panel under the workspace (⌃` or ⌘J), a file tree and an editor;
  download any file of the workspace to the device you are on, from the Mac (a save panel) or the iPhone (Sophon's
  folder in the Files app), on the LAN or through the relay
- **Schedules, history and usage**: cron schedules, archived agents, per-provider usage windows
- **Everywhere**: pair your iPhone by QR code; connects on the LAN directly or anywhere through the relay,
  end-to-end encrypted (Noise)
- **Keyboard first**: command palette (⌘K) and shortcuts, also while you type in the composer; desktop notifications
  when an agent needs you
- **Self-updating**: a sidebar button appears when a new release is out

What is done and what is still missing, item by item: [docs/parity.md](docs/parity.md).

## Install

You need the daemon running on the computer where your code lives (Sophon connects to it on its own once it is up:
`paseo daemon start`).

### macOS (14 Sonoma or later, Apple silicon)

**Homebrew**

```sh
brew tap caoxing9/sophon https://github.com/caoxing9/sophon
brew install --cask sophon
```

**One line**

```sh
curl -fsSL https://raw.githubusercontent.com/caoxing9/sophon/main/install.sh | sh
```

It installs into `/Applications` (or `~/Applications` when that is not writable). Options:
`SOPHON_VERSION=0.1.0` installs a given version, `SOPHON_DIR=…` another directory, `SOPHON_NO_OPEN=1` does not
launch it afterwards.

**By hand**: download `Sophon-<version>-macos-aarch64.dmg` from
[Releases](https://github.com/caoxing9/sophon/releases/latest).

> Sophon is signed ad hoc, not notarized yet. If macOS refuses to open it, run this once (Homebrew and the script do it
> for you):
>
> ```sh
> xattr -dr com.apple.quarantine /Applications/Sophon.app
> ```

**Updates**: Sophon checks GitHub Releases shortly after launch and every 6 hours. When a newer version is out, an
*Update to x.y.z* button appears at the bottom of the sidebar; one press downloads it, verifies its checksum, replaces
the app and restarts it. `brew upgrade --cask sophon` works too.

### iOS

There is no App Store or TestFlight build yet. Each release has `Sophon-<version>-ios-simulator.zip` for the iOS
Simulator. To run it on your own iPhone with a free Apple ID: sign in to Xcode (*Settings → Accounts*), plug in the
iPhone, turn on *Developer Mode* (*Settings → Privacy & Security*), set up the build tools (below) and run

```sh
scripts/ios-device.sh
```

It makes a development certificate and profile, builds and signs Sophon and installs it. The first time, trust your
Apple ID on the iPhone (*Settings → General → VPN & Device Management*). Free signing lasts 7 days: run it again then.
On the Mac, open *Settings → Pair a device* in Sophon and scan the QR code with the iPhone's camera.

## Build from source

Requirements: macOS with Xcode, [Rust](https://rustup.rs) (stable) and Git.

Sophon links NSH at the commit `Cargo.toml` pins, and the `nsh` CLI must come from the same commit (the released
`nsh` can be older than the pin). This script builds it and prints the directory of the binary:

```sh
export PATH="$(scripts/install-nsh.sh):$PATH"
```

Then:

```sh
nsh run                          # run on this Mac; Rust edits rebuild and restart it
nsh run --target ios             # run in the iOS Simulator
nsh check                        # cargo check + type check of App.test.tsx
cargo test --locked              # Rust tests
nsh test App.test.tsx            # headless UI tests
nsh package --target macos       # .nsh/out/Sophon.app
nsh package --target ios-simulator --icon assets/app-icon/icon-ios-1024.png
```

`cargo run` also starts the app directly on the native desktop backend (AppKit on macOS).

### Project layout

| Path | What it is |
|---|---|
| `src/daemon/` | The daemon connection: WebSocket protocol, relay end-to-end encryption, the LAN bridge for phones |
| `src/store.rs` | UI state in signals |
| `src/ui/kit/` | The design system: controls, icons, menus, tokens |
| `src/ui/shared/` | Components both platforms use: the timeline, the composer, the agent pane, the project list, dialogs, settings sections |
| `src/ui/desktop/` | The desktop's own shell and screens: sidebar, tabs, explorer, git, terminals, new workspaces, history, schedules, shortcuts |
| `src/ui/mobile/` | The phone's: pairing, the swipeable panels (workspaces, agent, changes), its settings |
| `src/update.rs` | Self-update from GitHub Releases |
| `App.test.tsx` | Headless UI tests of the root component, run by `nsh test` |
| `assets/app-icon/` | The app icon: SVG sources, `build.sh` regenerates every size |
| `nsh.json` | App name, id, version, icon, URL scheme, updater |
| `Casks/`, `install.sh`, `scripts/` | Homebrew cask, installer, versioning and release helpers |

## Release

```sh
scripts/set-version.sh 0.2.0
git commit -am "Release 0.2.0"
git tag v0.2.0
git push origin main v0.2.0
```

The [release workflow](.github/workflows/release.yml) builds the macOS apps (Apple silicon and Intel) and the iOS
Simulator app, publishes the GitHub Release with `latest.json` (what installed apps update from), and updates the
Homebrew cask. Each release has:

| Asset | For |
|---|---|
| `Sophon-<v>-macos-aarch64.dmg` | Installing by hand |
| `Sophon-<v>-macos-aarch64.tar.gz` | The installer, Homebrew and in-app updates |
| `Sophon-<v>-ios-simulator.zip` | The iOS Simulator |
| `latest.json` | The update feed |

With App Store signing secrets set (`IOS_CERT_P12`, `IOS_CERT_PASSWORD`, `IOS_PROFILE`, `ASC_APP_ID`,
`ASC_ISSUER_ID`, `ASC_KEY_ID`, `ASC_KEY_P8`), it also builds a signed `Sophon.ipa` and uploads it to TestFlight.

[CI](.github/workflows/ci.yml) runs on every push and pull request: check, Rust and UI tests, and packages for macOS and
the iOS Simulator (downloadable from the run's artifacts).

## Acknowledgements

- [NSH](https://github.com/longbridge/nsh) is the native UI framework Sophon is built on.
- The name comes from Liu Cixin's *The Three-Body Problem*. Sophon is a fan project, not affiliated with the book's
  rights holders or NSH.
