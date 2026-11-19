# Security

pippwin installs tools, handles an Apple login and runs a project's build,
so this page says exactly what it asks for, what it keeps, where, and how
downloads are checked. To report a vulnerability, see
[SECURITY.md](https://github.com/buchochelliq-labs/pippwin/blob/main/SECURITY.md).

## What pippwin never sees

- **Your Apple ID password and two-factor code.** In password mode pippwin
  asks only for the Apple ID, shows xtool's warning, and runs
  `xtool auth login --mode password -u <Apple ID>` on your terminal: you
  type the password (hidden), the code and the team into xtool itself.
  xtool's `-p` option is never used. No pippwin screen is active while
  xtool or sudo has the terminal. From xtool, these go only to Apple.
- **Your Linux password.** It is typed into `sudo`, which pippwin uses only
  for `apt-get` and `usbmuxd`. Every sudo command is shown and confirmed
  first.

## What is stored, where

| What | Where | Permissions | Holds |
|---|---|---|---|
| pippwin's state | `~/.config/pippwin/state.toml` | file `0600`, folder `0700` | the projects set up (name, folder, repository URL, branch, whether installed), the Apple ID, the login mode, the API key file's location, the finished steps. **No secrets**: no password, code, key contents or token. A state file with any other field is refused. |
| The API key | `~/.config/pippwin/keys/AuthKey_<KEYID>.p8` | file `0600`, folder `0700` | the App Store Connect private key, in API-key mode only. |
| xtool's login token | `~/.config/xtool/data/XTLAuthToken` | file `0600`, folders `0700` (set by pippwin) | xtool's login. In API-key mode it contains the private key. |
| Tools | `~/.local/bin`, `~/.local/lib/nodejs`, `~/.local/share/swiftly`, `~/.cargo`, `~/.rustup` | defaults | xtool, Node.js, Swift, Rust. |
| pippwin's source | `~/.local/src/pippwin` | defaults | the clone the bootstrap scripts build. |

## The API key

In API-key mode pippwin asks for the Key ID and the Issuer ID (both
validated) and where the `.p8` file is (it looks in the Windows Downloads
folders). It checks the file looks like a PEM private key without printing
any of it, then copies it to `~/.config/pippwin/keys/`. It refuses to keep
a key under `/mnt` or inside any git working tree, also when a symlinked
folder (a dotfiles `~/.config`, say) would carry it there, and offers to
delete the original: a key on the Windows drive (NTFS under `/mnt`) is
readable by Windows apps, sync and backup tools. Apple lets you download a
key once, so keep your backup in a password manager.

xtool is then given the Key ID, the Issuer ID and the key's **path** on its
standard input. The key's contents never go through command lines,
environment variables, logs or the state file, and pippwin overwrites its
in-memory copy after the check.

## xtool's login token

xtool writes its token with the default permissions (usually readable by
other users). pippwin therefore runs every xtool command that can write it
(`auth login`, `auth status`, `dev`) under `umask 077`, and afterwards sets
`~/.config/xtool` and its `data` folder to `0700` and the token to `0600`.
`pippwin doctor` reports anything looser; `pippwin doctor --fix` tightens
it. A path owned by another user is reported, not changed.

## The project's build

`build.prepare` from the project's `pippwin.toml` is run as an argument
vector, without a shell, in the project's app folder, under `umask 077`;
its first item must be a script inside the app folder or a plain command
name, and shells and `sudo` are refused (see the
[validation rules](pippwin-toml.md#validation-rules)). It is still the
project's own code: only set up projects you trust, as with any build.

A project's repository URL must be https with no credentials in it. A
clone is reused only when its `origin` is that repository and it has no
uncommitted changes, and it is only ever fast-forwarded to `origin`'s
branch, never reset or rebased. Clones under `/mnt` are refused.

## Downloads

Every download uses HTTPS only (also after redirects, TLS 1.2 or newer)
into a private temporary folder, and is checked before anything runs:

| Download | Checked against |
|---|---|
| Node.js | the pinned SHA-256 **and** the release's `SHASUMS256.txt` |
| rustup-init | the pinned SHA-256 **and** its published `.sha256` |
| swiftly | its detached signature, by one of the pinned swift.org signing keys (fingerprints); swiftly then verifies the toolchain |
| xtool AppImage | the SHA-256 GitHub records for the pinned release's asset (xtool publishes no checksum file), and a pinned SHA-256 once one is recorded; with neither, xtool is not installed |
| pnpm | corepack's own verification |

A file that does not match is deleted. Nothing downloaded is piped into a
shell, and `setup-windows.ps1` does no `Invoke-Expression` of remote
content and changes no execution policy (`-ExecutionPolicy Bypass` applies
to its one process). Download and read the scripts before running them.

On Windows, pippwin runs only usbipd-win, and only when you say yes. It
shares the iPhone once through a User Account Control prompt (`usbipd bind`)
and attaches it without administrator rights. If you turn on
**reconnect automatically**, it starts `usbipd attach --wsl --busid <id>
--auto-attach` in a minimized window you can see and close. It installs no
service, scheduled task or startup entry, so nothing keeps running after
that window closes or Windows restarts.

Every pin is in [`src/versions.rs`](https://github.com/buchochelliq-labs/pippwin/blob/main/src/versions.rs);
[Maintaining pippwin](maintaining.md#updating-the-pins) says how each one
is obtained.
