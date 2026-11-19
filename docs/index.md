# pippwin

**pippwin takes a Windows PC from nothing to an iOS app built and installed
on your iPhone, through WSL.** It works with any project built with
[xtool](https://github.com/xtool-org/xtool), the tool that builds, signs and
installs iOS apps without Xcode. The project says what it needs in a small
file at the root of its repository, [`pippwin.toml`](pippwin-toml.md).

The name: Pippin, the apple, on Windows.

## What it is

- **An installer and a guide around xtool.** pippwin installs WSL with
  Ubuntu, usbipd-win, the Swift toolchain, xtool and the tools the project
  needs; it walks you through Apple's Xcode download, the Apple login and
  connecting the iPhone; then it builds and installs the app.
- **Careful with what it touches.** Every download is checked before it
  runs, passwords and two-factor codes are typed only into xtool or sudo,
  and the login token xtool keeps is made private to you. See
  [Security](security.md).
- **Safe to run again.** Each step checks whether it is already done and
  skips it, so you can stop at any point and carry on later.

## What it is not

- **Not affiliated with Apple or with xtool.** It runs xtool and repeats
  xtool's own questions and warnings so you know what is coming.
- **Not a replacement for macOS and Xcode.** Builds made this way are for
  trying an app on your own iPhone. Supported builds of any app, App Store
  and TestFlight uploads, should still be made on macOS with Xcode.
- **Not a way around Apple's rules.** You need your own Apple ID, and Apple
  decides what it allows: a free Apple ID's apps stop opening after 7 days,
  and entitlements such as push notifications need a paid membership.

## Quick start

=== "Windows"

    1. Download [`setup-windows.ps1`](https://github.com/buchochelliq-labs/pippwin/blob/main/scripts/setup-windows.ps1)
       and **read it** before you run it.
    2. In PowerShell, in the folder you saved it to:

        ```powershell
        powershell -ExecutionPolicy Bypass -File .\setup-windows.ps1 -Project https://github.com/owner/app
        ```

    `-Project` is optional: leave it out and pippwin asks. The details are in
    [Getting started](getting-started.md).

=== "Already in WSL"

    ```sh
    git clone https://github.com/buchochelliq-labs/pippwin ~/.local/src/pippwin
    less ~/.local/src/pippwin/scripts/bootstrap.sh   # read it first
    ~/.local/src/pippwin/scripts/bootstrap.sh --project https://github.com/owner/app
    ```

After the first setup, `pippwin run` in the project's folder rebuilds and
reinstalls the app.

## For project maintainers

To let people install your xtool project this way, run `pippwin init` in
your repository: it writes a commented `pippwin.toml` that you review and
commit. The [reference](pippwin-toml.md) describes every field, with
[Limiara](https://github.com/buchochelliq-labs/limiara-core) as a worked
example.
