# pippwin

An installer for WSL, xtool and iOS on Windows: pippwin takes a Windows PC
from nothing to an [xtool](https://github.com/xtool-org/xtool)-based iOS
project built and installed on your iPhone, through WSL.

**Documentation: <https://buchochelliq-labs.github.io/pippwin/>**

It installs WSL with Ubuntu, usbipd-win, Swift, xtool and the tools the
project asks for in its [`pippwin.toml`](docs/pippwin-toml.md); walks you
through Apple's Xcode download, the Apple login and connecting the iPhone;
then builds and installs the app. Every step can be re-run; every download
is verified; passwords and two-factor codes go only to xtool or sudo.

pippwin is an installer and guide around xtool. It is **not affiliated with
Apple or with xtool**, and it does not replace macOS and Xcode: supported
builds of any app should still be made there.

## Quick start

**On Windows:** download
[`scripts/setup-windows.ps1`](scripts/setup-windows.ps1), read it, then in
PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File .\setup-windows.ps1 -Project https://github.com/owner/app
```

**Already in WSL (Ubuntu 22.04 or newer):**

```sh
git clone https://github.com/buchochelliq-labs/pippwin ~/.local/src/pippwin
less ~/.local/src/pippwin/scripts/bootstrap.sh   # read it first
~/.local/src/pippwin/scripts/bootstrap.sh --project https://github.com/owner/app
```

You need Windows 11 (or 10 with WSL 2), an Apple ID (a paid membership for
API-key login and for entitlements such as push), the iPhone and a USB
cable, and about 40 GB of free disk. See
[Getting started](https://buchochelliq-labs.github.io/pippwin/getting-started/).

## Commands

`pippwin setup`, `run`, `doctor`, `login`, `sdk`, `phone`, `init`, `check`:
see [Commands](https://buchochelliq-labs.github.io/pippwin/commands/).
Project maintainers add pippwin support with `pippwin init`.

## More

- [Security](https://buchochelliq-labs.github.io/pippwin/security/) (and
  [SECURITY.md](SECURITY.md) to report a vulnerability)
- [Troubleshooting](https://buchochelliq-labs.github.io/pippwin/troubleshooting/)
- [Maintaining pippwin](https://buchochelliq-labs.github.io/pippwin/maintaining/)

Licensed under the [Apache License 2.0](LICENSE).
