# Getting started

## What you need

| | |
|---|---|
| **Windows** | Windows 11, or Windows 10 with WSL 2 (version 2004 or newer). Administrator rights once, to install WSL and usbipd-win. |
| **Disk space** | About **40 GB** free: Xcode.xip is 7 GB or more, and xtool extracts the iOS SDK from it. |
| **An Apple ID** | Any Apple ID works with password login. A **paid Apple Developer Program membership** is needed for API-key login, and for apps that need entitlements such as push notifications. |
| **The iPhone and a USB cable** | The iPhone is connected over USB and passed through to WSL by usbipd-win. |
| **A project** | Any xtool project whose repository has a [`pippwin.toml`](pippwin-toml.md). |

## On Windows

1. **Download** [`scripts/setup-windows.ps1`](https://github.com/buchochelliq-labs/pippwin/blob/main/scripts/setup-windows.ps1)
   (on that page: *Download raw file*).
2. **Read it.** It is short, and it will run as administrator. It never
   downloads and runs anything from PowerShell itself (no
   `Invoke-Expression` of remote content).
3. **Run it** in PowerShell, from the folder you saved it to:

    ```powershell
    powershell -ExecutionPolicy Bypass -File .\setup-windows.ps1
    ```

    `-ExecutionPolicy Bypass` applies to that one PowerShell process only;
    your execution policy is not changed. Optional parameters:

    | Parameter | Meaning |
    |---|---|
    | `-Project <git URL>` | The project to set up. Asked for when left out. |
    | `-Branch <name>` | The project's branch. Asked for when left out (the default is the branch its `pippwin.toml` names). |
    | `-Distro <name>` | The WSL distribution, `Ubuntu-24.04` by default. |
    | `-PippwinRef <branch or tag>` | Which pippwin to build, `main` by default. |

What the script does:

1. Starts itself again as administrator if it is not (Windows asks for
   permission).
2. Installs WSL with Ubuntu 24.04 if it is missing. **If Windows asks you
   to restart, restart**, open *Ubuntu 24.04* from the Start menu once to
   create your Linux user name and password, and run the script again.
3. Installs [usbipd-win](https://github.com/dorssel/usbipd-win) with
   winget if it is missing.
4. Inside Ubuntu, clones pippwin into `~/.local/src/pippwin` (or updates
   that clone, refusing if it has local changes or another origin), and
   runs its `scripts/bootstrap.sh`, which:
    - installs `git`, `curl`, `build-essential` and `ca-certificates` with
      apt (after showing the commands and asking);
    - installs rustup if it is missing, after checking `rustup-init`
      against the SHA-256 pinned in pippwin and the one rustup publishes;
    - builds pippwin with `cargo build --release --locked`, using the Rust
      version pippwin's `rust-toolchain.toml` pins;
    - installs the binary to `~/.local/bin/pippwin`;
    - starts `pippwin setup`, passing `-Project` and `-Branch` on.

## Already in WSL

If you already have WSL with Ubuntu (22.04 or newer), you can skip the
Windows script. You still need usbipd-win on Windows for the iPhone
(`winget install --interactive --exact dorssel.usbipd-win`).

```sh
git clone https://github.com/buchochelliq-labs/pippwin ~/.local/src/pippwin
less ~/.local/src/pippwin/scripts/bootstrap.sh   # read it first
~/.local/src/pippwin/scripts/bootstrap.sh
```

`bootstrap.sh` takes `--project <URL or folder>`, `--branch <name>`,
`--pippwin-ref <branch or tag>` and `--yes`. It can also be run on its
own, outside a clone: it then clones pippwin into `~/.local/src/pippwin`
first (or updates that clone).

## The first run

`pippwin setup` starts with an overview of every step and what is already
done, then goes through them one by one. Each step says what it is about to
do and why before it does it; the [setup steps](setup-steps.md) page has
them all. The steps that take long:

- **Xcode.xip.** Apple requires you to download Xcode yourself, signed in
  with your Apple ID, in a browser. pippwin opens the download page in your
  Windows browser and then finds the file in your Windows Downloads folder.
  Extracting the SDK takes 10 to 30 minutes.
- **The first build.** The project's own preparation (Rust, web assets) can
  take a while the first time.

You can stop at any time (Esc, or Ctrl+C while a command runs) and run
`pippwin setup` again later: finished steps are skipped.

On the iPhone, the first time an app is installed, open **Settings →
General → VPN & Device Management**, tap your developer profile and trust
it.

## Next times

```sh
cd ~/my-app        # the project's folder
pippwin run
```

`pippwin run` runs the project's preparation and `xtool dev` again. With a
free Apple ID, do this at least every 7 days: Apple's free signing expires
after a week. Plug the iPhone in first and attach it to WSL again (see
[Troubleshooting](troubleshooting.md#the-iphone-is-not-found-after-reconnecting-it)).
