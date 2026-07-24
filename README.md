<p align="center">
  <img src="https://img.shields.io/badge/language-rust-green?style=flat-square" alt="Language">
  <img src="https://img.shields.io/badge/yroz-v0.1.0-blue?style=flat-square" alt="Version">
</p>

<h1 align="center">yroz</h1>

<p align="center">A universal software manager for Linux, written in Rust</p>

***

**Yroz:** is a command-line tool to manage packages across any Linux distribution, unified under a single command, without dependencies.

### Features:

**Universal Support** - APT, Pacman, DNF, Portage, XBPS, Zypper, APK, AUR/yay, Nix, Flatpak, and Snap.

**Priority Order** - Automatically detects and installs native packages first, with smart fallbacks to Flatpak and Snap.

**Parallel Search** - Concurrent queries in all active package managers with real-time feedback.

**Configuration** - Turn off specific backends or customize installation priorities using a simple TOML file.

**Self-Update** - Keep Yroz updated automatically with one single command.

---

### How to Install:

```bash
cargo build --release
sudo cp target/release/yroz /usr/local/bin/yroz
```

### Commands:

```bash
yroz status
yroz search <query>
yroz install <package>
yroz remove <package>
yroz update
yroz info <package>
yroz list
yroz self-update
```

### Configuration:

Create a file in `~/.config/yroz/config.toml`:

```toml
disabled_backends = ["Snap"]
priority = ["Nix", "Flatpak", "APT"]
```

---

License: MIT
