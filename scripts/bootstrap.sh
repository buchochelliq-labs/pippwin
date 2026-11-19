#!/usr/bin/env bash
# Builds pippwin inside WSL's Ubuntu, installs it to ~/.local/bin/pippwin
# and starts its guided setup.
#
#   bootstrap.sh [--project URL|PATH] [--branch NAME] [--pippwin-ref REF] [--yes]
#
# Run it from a clone of pippwin (scripts/bootstrap.sh), or on its own: it
# then clones https://github.com/buchochelliq-labs/pippwin into
# ~/.local/src/pippwin (or updates that clone) and builds there. It
# installs only what building pippwin needs (git, curl, a C compiler,
# rustup) and leaves everything else to pippwin itself.
#
# Nothing is piped from the network into a shell: rustup-init is
# downloaded to a temporary file and checked against the SHA-256 pinned
# in src/versions.rs and the one rustup publishes before it runs.
set -euo pipefail

pippwin_url="https://github.com/buchochelliq-labs/pippwin"
pippwin_ref="main"
src_dir="${PIPPWIN_SRC:-$HOME/.local/src/pippwin}"
assume_yes=0
forward=()

fail() { printf 'bootstrap: %s\n' "$*" >&2; exit 1; }
say() { printf '\033[1m==> %s\033[0m\n' "$*"; }
confirm() {
    [ "$assume_yes" = 1 ] && return 0
    local answer
    read -r -p "$1 [Y/n] " answer
    case "$answer" in ""|y|Y|yes|Yes) return 0 ;; *) return 1 ;; esac
}
# Names passed to git: plain characters only, never option-like.
plain_name() {
    case "$1" in
        ""|-*|*..*|*[!A-Za-z0-9._/-]*) return 1 ;;
        *) return 0 ;;
    esac
}

while [ $# -gt 0 ]; do
    case "$1" in
        --project)
            value="${2:?--project needs a git URL or a folder}"
            case "$value" in -*) fail "not a project: $value" ;; esac
            forward+=(--project "$value"); shift 2 ;;
        --branch)
            value="${2:?--branch needs a name}"
            plain_name "$value" || fail "not a branch name: $value"
            forward+=(--branch "$value"); shift 2 ;;
        --pippwin-ref)
            pippwin_ref="${2:?--pippwin-ref needs a branch or tag}"
            plain_name "$pippwin_ref" || fail "not a branch or tag: $pippwin_ref"
            shift 2 ;;
        --yes|-y) assume_yes=1; forward+=(--yes); shift ;;
        *) fail "unknown option: $1 (see the comment at the top of this script)" ;;
    esac
done

[ "$(id -u)" != 0 ] || fail "run this as your own Linux user, not root"

# 1. What building pippwin needs, from Ubuntu's archive.
missing=()
for pkg in git curl build-essential ca-certificates; do
    dpkg-query -W -f '${Status}' "$pkg" 2>/dev/null | grep -qx 'install ok installed' || missing+=("$pkg")
done
if [ "${#missing[@]}" -gt 0 ]; then
    say "These packages are needed to build pippwin: ${missing[*]}"
    echo "  sudo apt-get update"
    echo "  sudo apt-get install -y ${missing[*]}"
    echo "sudo asks for your Linux password; it is typed into sudo, not into this script."
    confirm "Run them now?" || fail "stopped"
    sudo apt-get update
    sudo apt-get install -y "${missing[@]}"
fi

# 2. pippwin's source: this clone, or ~/.local/src/pippwin (cloned or
#    updated, fast-forward only).
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "$here/../src/versions.rs" ] && grep -qx 'name = "pippwin"' "$here/../Cargo.toml" 2>/dev/null; then
    src="$(cd "$here/.." && pwd)"
else
    src="$src_dir"
    case "$src" in /mnt/*) fail "keep pippwin's source in your Linux home, not under /mnt" ;; esac
    if [ -e "$src/.git" ]; then
        origin="$(git -C "$src" remote get-url origin)"
        case "$origin" in
            "$pippwin_url"|"$pippwin_url.git") ;;
            *) fail "$src is a clone of $origin, not $pippwin_url" ;;
        esac
        [ -z "$(git -C "$src" status --porcelain)" ] || fail "$src has local changes; commit or remove them first"
        say "Updating pippwin in $src ($pippwin_ref)"
        git -C "$src" fetch origin -- "$pippwin_ref"
        git -C "$src" checkout --quiet --detach FETCH_HEAD
    else
        say "Cloning $pippwin_url ($pippwin_ref) into $src"
        mkdir -p "$(dirname "$src")"
        git clone --branch "$pippwin_ref" -- "$pippwin_url" "$src"
    fi
fi

# 3. rustup, verified, when cargo is missing.
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
if ! command -v rustup >/dev/null 2>&1; then
    versions="$src/src/versions.rs"
    rustup_version="$(sed -n 's/^pub const RUSTUP_VERSION: &str = "\(.*\)";$/\1/p' "$versions")"
    case "$(uname -m)" in
        x86_64) arch=x86_64; pin_name=RUSTUP_SHA256_X86_64 ;;
        aarch64) arch=aarch64; pin_name=RUSTUP_SHA256_AARCH64 ;;
        *) fail "unsupported architecture $(uname -m)" ;;
    esac
    pinned="$(grep -A1 "^pub const $pin_name" "$versions" | grep -oE '[0-9a-f]{64}' | head -n 1)"
    if [ -z "$rustup_version" ] || [ -z "$pinned" ]; then
        fail "no rustup pin in $versions"
    fi
    url="https://static.rust-lang.org/rustup/archive/$rustup_version/$arch-unknown-linux-gnu/rustup-init"
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    say "Downloading rustup $rustup_version"
    curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSL -o "$tmp/rustup-init" "$url"
    published="$(curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSL "$url.sha256" | grep -oE '^[0-9a-f]{64}')"
    actual="$(sha256sum "$tmp/rustup-init" | cut -d ' ' -f 1)"
    if [ "$actual" != "$pinned" ] || [ "$actual" != "$published" ]; then
        fail "rustup-init's SHA-256 does not match the pinned and published values"
    fi
    chmod 700 "$tmp/rustup-init"
    "$tmp/rustup-init" -y --profile minimal --default-toolchain none
fi

# 4. Build pippwin with the toolchain its rust-toolchain.toml pins,
#    install it, and start the setup.
say "Building pippwin"
# (`rustup toolchain install` with no name reads rust-toolchain.toml from
# rustup 1.28 on; an older rustup installs it on `rustup show`.)
(cd "$src" && { rustup toolchain install || rustup show; } && cargo build --release --locked)
target="${CARGO_TARGET_DIR:-$src/target}"
mkdir -p "$HOME/.local/bin"
install -m 0755 "$target/release/pippwin" "$HOME/.local/bin/pippwin"
say "Installed $HOME/.local/bin/pippwin; starting the guided setup"
exec "$HOME/.local/bin/pippwin" setup ${forward[@]+"${forward[@]}"}
