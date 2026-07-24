<p align="center">
  <img src="https://img.shields.io/badge/language-rust-green?style=flat-square" alt="Language">
  <img src="https://img.shields.io/badge/yroz-v0.1.0-blue?style=flat-square" alt="Version">
</p>

<h1 align="center">yroz</h1>

<p align="center">A universal software manager for Linux, written in Rust</p>

***

**Yroz:** is a command-line tool to manage packages across any Linux distribution, unified under a single command, without dependencies.

### Features:

**Universal Support** - Supports package managers and universal formats across all major Linux distributions:
*   <img src="https://img.shields.io/badge/-Debian-D10A34?style=flat-square&logo=debian&logoColor=white" alt="Debian"> <img src="https://img.shields.io/badge/-Ubuntu-E95420?style=flat-square&logo=ubuntu&logoColor=white" alt="Ubuntu"> **Debian, Ubuntu, Linux Mint, Pop!_OS** (via `APT`)
*   <img src="https://img.shields.io/badge/-Arch_Linux-1793D1?style=flat-square&logo=archlinux&logoColor=white" alt="Arch"> **Arch Linux, Manjaro** (via `Pacman` and `AUR/yay`)
*   <img src="https://img.shields.io/badge/-Fedora-3C6EB4?style=flat-square&logo=fedora&logoColor=white" alt="Fedora"> **Fedora, RedHat, CentOS** (via `DNF`)
*   <img src="https://img.shields.io/badge/-Gentoo-121011?style=flat-square&logo=gentoo&logoColor=white" alt="Gentoo"> **Gentoo** (via `Portage`)
*   <img src="https://img.shields.io/badge/-Void_Linux-478061?style=flat-square&logo=voidlinux&logoColor=white" alt="Void"> **Void Linux** (via `XBPS`)
*   <img src="https://img.shields.io/badge/-openSUSE-73BA25?style=flat-square&logo=opensuse&logoColor=white" alt="openSUSE"> **openSUSE** (via `Zypper`)
*   <img src="https://img.shields.io/badge/-Alpine-0D597F?style=flat-square&logo=alpinelinux&logoColor=white" alt="Alpine"> **Alpine Linux** (via `APK`)
*   <img src="https://img.shields.io/badge/-Solus-5277C3?style=flat-square&logo=solus&logoColor=white" alt="Solus"> **Solus OS** (via `eopkg`)
*   <img src="https://img.shields.io/badge/-Flatpak-3B5998?style=flat-square&logo=flatpak&logoColor=white" alt="Flatpak"> <img src="https://img.shields.io/badge/-Snap-820C30?style=flat-square&logo=canonical&logoColor=white" alt="Snap"> <img src="https://img.shields.io/badge/-Nix-5277C3?style=flat-square&logo=nixos&logoColor=white" alt="Nix"> **Universal Formats** (via `Flatpak`, `Snap`, `Nix`, and `AppImage`)

**Transactional AppImages** - Atomic AppImage installation. Downloads to a `.tmp` file and automatically rolls back (deletes files and shortcuts) if any step (download, chmod, desktop shortcuts) fails.

**Fuzzy AppImage Resolving** - Install AppImages by name (e.g. `prismlauncher.appimage`). Yroz resolves the repository, CPU architecture, and GitHub releases automatically.

**Priority Order** - Automatically detects and installs native packages first, with smart fallbacks to Flatpak and Snap.

**Parallel Search** - Concurrent queries in all active package managers with real-time feedback.

**Configuration** - Turn off specific backends or customize installation priorities using a simple TOML file.

**Self-Update** - Keep Yroz updated automatically with one single command.

---

### How to Install:

**Quick Install (Precompiled Binary):**
```bash
curl -fsSL https://raw.githubusercontent.com/Yrozxm/Yroz-cli/main/install.sh | sh
```

**Build from Source:**
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
