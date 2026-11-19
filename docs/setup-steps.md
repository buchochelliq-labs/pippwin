# Setup steps

`pippwin setup` runs these steps in order. Each one first checks whether it
is already done; a finished step is skipped, so setup can be stopped and run
again at any time. `pippwin doctor` runs every check without changing
anything.

Where xtool will ask a question or print a warning, pippwin shows xtool's
words first, exactly, so you know what is coming and what it means. The
quotes below are the texts pippwin shows; they come from xtool's own source.

## 1. WSL with Ubuntu

xtool runs on Linux. On Windows that Linux is WSL with Ubuntu, where
everything below is installed. This checks that pippwin runs in WSL
(`/proc/version` names Microsoft's kernel) on Ubuntu 22.04 or newer. If
not, it tells you how to get there (the Windows script, or
`wsl --install -d Ubuntu-24.04`) and stops.

## 2. System packages

From Ubuntu's own archive, with apt: a C toolchain (`clang`, `llvm`,
`build-essential`), `git`, `curl`, `rsync`, `xz-utils` and `gnupg` for
downloads and checks, `usbmuxd` and `libimobiledevice-utils` to talk to
the iPhone, FUSE 2 for xtool's AppImage (`libfuse2t64` on Ubuntu 24.04,
`libfuse2` before), and the libraries swift.org lists for its toolchain.

pippwin shows the exact `sudo apt-get` commands and asks before running
them. sudo asks for **your Linux password**; it is typed into sudo, never
into pippwin.

## 3. The project

The project's repository is cloned into your Linux home, or a clone you
already have is used. pippwin asks:

- **Which project?** Projects set up before, the project in the current
  folder, a git URL to clone, or a folder already in WSL. `--project` on
  the command line answers this.
- **Where should the repository be?** By default `~/<repository name>`.
  A folder under `/mnt` (a Windows drive) is refused: builds there are slow
  and Linux file permissions do not work.
- **Which branch?** The branch the project's `pippwin.toml` names (usually
  `main`), or another one. `--branch` answers this.

An existing clone is reused only when its `origin` is the same repository
and it has no uncommitted changes. Then `pippwin.toml` is read and checked
(see [the reference](pippwin-toml.md)), and pippwin shows what it found:
the repository, the app folder, the bundle ID from `xtool.yml`, what runs
before `xtool dev`, and which tools the project needs.

## 4. The project's own packages

Extra Ubuntu packages the project lists in `needs.apt`, installed with apt
the same way as in step 2. Each name was checked against Ubuntu's
package-name rules when `pippwin.toml` was read.

## 5. Swift toolchain

xtool builds the Swift half of the app with the Swift toolchain for Linux;
xtool's [Linux guide](https://github.com/xtool-org/xtool/blob/main/Documentation/xtool.docc/Installation-Linux.md)
names Swift 6.4. It is installed with swiftly, swift.org's own installer:
swiftly's download is checked against its signature by a pinned swift.org
key before it runs, and swiftly checks the toolchain's signature itself.
The extra packages swiftly says the toolchain needs are installed with apt
(pippwin reads the package names from swiftly's script; it never runs the
script).

## 6. xtool

xtool's AppImage, a pinned release, is downloaded from xtool's GitHub
release into `~/.local/bin/xtool` and checked against the SHA-256 GitHub
records for that release asset before it is made executable.

## 7. Rust and its targets

Only when the project has `needs.rust = true`. rustup installs the
toolchain the project's own `rust-toolchain.toml` names (or the stable
toolchain when it has none), and the targets in `needs.rust_targets`. When
rustup is missing, `rustup-init` (a pinned version) is checked against the
pinned and the published SHA-256 first.

## 8. Node.js and pnpm

Only when the project has `needs.node`. pippwin installs its own pinned
release of that Node.js major from nodejs.org, checked against the
release's `SHASUMS256.txt` and the pinned checksum, into `~/.local`. When
the project names `needs.pnpm`, that exact pnpm version is activated
through corepack, which ships with Node.js and checks what it downloads.

## 9. iOS SDK from Xcode.xip

Apple's iOS SDK comes only inside Xcode. pippwin tells you which Xcode
version xtool's guide names, offers to open Apple's download page in your
Windows browser (you sign in and accept Apple's licence there; it cannot be
downloaded with curl), then looks for `Xcode*.xip` in your Windows and
Linux Downloads folders. A file under 1 GB is treated as an unfinished
download; another version than the one xtool names is allowed after a
warning. A file on the Windows drive can be copied into Linux first, which
is usually faster to extract.

xtool's own messages, which pippwin shows first:

```text
Please download Xcode from http://developer.apple.com/download/all/?q=Xcode
and enter the path to the downloaded Xcode.xip.
```

```text
Darwin SDK is out of date. Rebuilding...
```

```text
Darwin SDK is out of date, and was installed in 'slim' mode.

Slim SDKs take less disk space, but can't be auto-updated.
Please install a new SDK with
    xtool sdk install [--slim]
```

```text
Darwin SDK is incompatible: built with an older version of xtool.

Requesting re-install. This is a one-time rebuild; after this,
xtool will be able to resolve incompatibilities automatically.
```

Then `xtool sdk install <path>` extracts the SDK into
`~/.swiftpm/swift-sdks/darwin.artifactbundle` (10 to 30 minutes).

## 10. Apple login for xtool

xtool logs in to Apple to sign the app. pippwin asks xtool's own question
with xtool's own two answers:

```text
Select login mode
  API Key (requires paid Apple Developer Program membership)
  Password (works with any Apple ID but uses private APIs)
```

=== "API key"

    Recommended with a paid membership: it uses the public App Store
    Connect API. In App Store Connect, *Users and Access → Integrations →
    Team Keys*, create a key with the **App Manager** role; note its
    **Key ID** and the **Issuer ID**, and download the key
    (`AuthKey_<KEYID>.p8`). Apple lets you download it **only once**: keep
    a backup in a password manager.

    pippwin asks for the Key ID and the Issuer ID (both checked), finds the
    key file in your Downloads, checks that it looks like a private key
    without showing any of it, copies it to `~/.config/pippwin/keys/`
    (only you can read it) and offers to delete the original. It then gives
    xtool the Key ID, the Issuer ID and the key's path.

=== "Password"

    For any Apple ID. pippwin shows xtool's warning first:

    ```text
    Heads up:
      This works by emulating a virtual Mac to interact with Apple services.
      We use a third-party service to create a randomized device ID token
      (Anisette Data) but your credentials are only ever sent to Apple.
      Still, consider using a secondary Apple ID since this is a private API.
    ```

    It asks for your Apple ID only, then runs
    `xtool auth login --mode password -u <Apple ID>` on your terminal. xtool
    asks, in this order: **Password:** (nothing shows as you type),
    **Code:** (the two-factor code Apple shows on your other devices or
    sends by text), and a team if your Apple ID has more than one. These
    are typed into xtool, not into pippwin.

If xtool is already logged in, pippwin offers to keep the login or log in
again. Afterwards it makes xtool's login token private (see
[Security](security.md#xtools-login-token)).

## 11. iPhone connection

WSL does not see USB devices by itself. usbipd-win, on Windows, passes the
iPhone through: it is **bound** once (an administrator action, so Windows
shows a User Account Control prompt for `usbipd bind --busid <id>`) and
**attached** each time you plug it in (`usbipd attach --wsl --busid <id>`).
Inside WSL, `usbmuxd` talks to the phone. Where WSL runs systemd, its udev
rule starts it when the phone is attached; otherwise pippwin starts it with
sudo after showing the command.

pippwin then offers to **reconnect automatically**
(`usbipd attach --wsl --busid <id> --auto-attach`, in a minimized Windows
window), so a replugged iPhone is attached again by itself until that window
is closed or Windows restarts. Without it, `pippwin run` notices a replugged
iPhone and offers to connect it.

Unlock the iPhone. If it asks **Trust This Computer?**, tap Trust and enter
its passcode (xtool shows "Please tap 'trust' on your device..." while it
waits for that).

## 12. Build and install the app

The project's `build.prepare` runs first, in the app folder, if it has one.
Then `xtool dev` builds the app, signs it with your login and installs it.
While it runs xtool may say:

- **Please tap 'trust' on your device...**: tap Trust on the iPhone and
  enter its passcode.
- **Please unlock your device...**: unlock the iPhone.
- **The following certificates must be revoked:** followed by
  **Continue? (yes/no)**: Apple allows only a few development certificates
  per account. Answering yes revokes the ones listed so xtool can make a new
  one; apps already signed with them (on any device) stop opening until
  reinstalled. Answer no to stop instead.

Both commands run under `umask 077`, so what they write is private to you.
The first time, trust the developer profile on the iPhone under
**Settings → General → VPN & Device Management**.
