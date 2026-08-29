# Flatpak Helper (`fh`)

A fast, intelligent, and seamless CLI wrapper for Flatpak written in Rust. 

Tired of typing or remembering long Application IDs like `org.mozilla.firefox` or `com.valvesoftware.Steam`? `fh` allows you to interact with Flatpak using simple, human-readable keywords. It automatically resolves them, and if there are multiple matches, it provides a sleek, Fish-shell-style interactive selection menu.

## Features

- **Context-Aware Search**: Automatically searches remote repositories (like Flathub) when you run `fh install <keyword>`, and searches only locally installed packages for commands like `run`, `uninstall`, `info`, `mask`, etc.
- **Interactive TUI**: If multiple apps match your keyword, a fast, zero-flicker, minimalistic terminal UI pops up to let you choose exactly what you meant.
- **Seamless & Transparent**: 
  - Automatically identifies flags (`-y`, `--user`) and leaves them untouched.
  - Leaves commands that don't need App ID resolution (e.g., `update`, `build`, `remote-add`) completely unmodified, safely passing them to Flatpak.
- **Zero Overhead**: Written in Rust. It utilizes `execvp` to completely replace its own process with `flatpak`, meaning all native terminal colors, progress bars, and interactive prompts (like `[Y/n]`) work perfectly.

## Cachix (Binary Cache)

To speed up the build process, a public Cachix binary cache is available.

- **URL**: https://flatpak-helper.cachix.org
- **Public Key**: `flatpak-helper.cachix.org-1:H+WAxDwmRUWl2mJuxmAT3MTpbNvzMZJ6nRS14AZSdrQ=`

To use it, you can run:
```bash
cachix use flatpak-helper
```
Or manually add it to your `configuration.nix`:
```nix
nix.settings = {
  substituters = [
    "https://flatpak-helper.cachix.org"
  ];
  trusted-public-keys = [
    "flatpak-helper.cachix.org-1:H+WAxDwmRUWl2mJuxmAT3MTpbNvzMZJ6nRS14AZSdrQ="
  ];
};
```

## Installation

### Try it instantly via `nix run`

You can try `fh` without installing it by running:

```bash
nix run github:ArisoN-ext/flatpak-helper -- search firefox
```

### NixOS Configuration (Flake)

Add the flake to your `flake.nix` inputs:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flatpak-helper.url = "github:ArisoN-ext/flatpak-helper";
  };

  outputs = { self, nixpkgs, flatpak-helper, ... }@inputs: {
    nixosConfigurations."your-hostname" = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        # ...
        ({ pkgs, ... }: {
          environment.systemPackages = [
            flatpak-helper.packages.${pkgs.system}.default
          ];
        })
      ];
    };
  };
}
```

### Arch Linux (AUR)

You can install `flatpak-helper` from the [Arch User Repository (AUR)](https://aur.archlinux.org/packages/flatpak-helper) (Thanks [ventureo](https://github.com/ventureoo)) using your favorite AUR helper, such as `yay` or `paru`:

```bash
yay -S flatpak-helper
# or
paru -S flatpak-helper
```

## Usage Examples

**Install an application (searches remotes):**
```bash
fh install firefox
```

**Run an installed application:**
```bash
fh run telegram
```

**Uninstall multiple applications:**
```bash
fh uninstall discord gimp
```

**Safe passthrough for advanced commands:**
```bash
fh remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
```
