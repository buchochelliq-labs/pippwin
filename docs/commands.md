# Commands

```text
pippwin [--yes] <command>
```

`--yes` (`-y`) answers yes/no confirmations with their default. It never
answers for a password, a code or a key: you always type those, into
xtool or sudo.

Errors carry a stable code (`PW0xx_…`) and a plain sentence. They never
quote a key, a password or a command's full arguments.

## `pippwin setup`

```text
pippwin setup [--project <git URL or folder>] [--branch <name>]
```

The whole guided setup: an overview, then every [step](setup-steps.md) in
turn. Finished steps are skipped, so it can be run again at any time.

- `--project`: the project's https git URL (cloned, by default into
  `~/<repository name>`) or a folder in WSL that holds its `pippwin.toml`.
  Asked for when left out.
- `--branch`: the branch to check out. Asked for when left out; the default
  is the branch the project's `pippwin.toml` names.

## `pippwin run`

```text
pippwin run [--project <folder>]
```

Builds and installs the app again: the project's `build.prepare`, then
`xtool dev`, both in the app folder. The project is the one in the current
folder (pippwin looks upwards, up to the root of the git repository), or
`--project`, or else the one set up last. Run it at least every 7 days with
a free Apple ID.

Before building, it checks that WSL can see the iPhone. If the iPhone is
plugged in but not connected (after a replug, say), pippwin asks
whether to connect it now, and does what `pippwin phone` does. If no iPhone
is on USB, it offers to look again, to continue anyway, or to stop. Nothing
is connected without asking.

## `pippwin doctor`

```text
pippwin doctor [--fix]
```

A table of every check: WSL, packages, the project and its app folder,
Swift, xtool, `~/.local/bin` on `PATH`, Rust, Node.js, the SDK, the login,
the login token's permissions, the API key file, usbipd, usbmuxd, whether
the iPhone reconnects automatically, and the iPhone. Outside WSL the Windows-side checks are skipped. `--fix` also
tightens the permissions of xtool's login token and of pippwin's own files
(see [Security](security.md)).

## `pippwin login`

Logs xtool in to Apple again: to switch account or login mode, or after a
password login's token expired.

## `pippwin sdk`

Installs or updates the iOS SDK from Xcode.xip (step 9 on its own).

## `pippwin phone`

Connects the iPhone to WSL (step 11 on its own): finds it with usbipd,
binds it if needed, attaches it and starts usbmuxd. It then offers to
**reconnect automatically**: usbipd-win's `--auto-attach`, in a minimized
Windows window, attaches the iPhone again whenever it is plugged in, until
that window is closed or Windows restarts. Without it, `pippwin run` offers
to connect a replugged iPhone, or run `pippwin phone` again.

## `pippwin init`

```text
pippwin init [--app <folder>]
```

For project maintainers: writes a commented `pippwin.toml` at the root of
the current git repository. It asks which folder holds the xtool project
(it lists the folders with an `xtool.yml`), or takes `--app`. The name,
repository URL (from `origin`), branch, `build.prepare` (`./prepare.sh` if
the app folder has one) and needs (Rust when there is a
`rust-toolchain.toml` or `Cargo.toml`, Node.js and pnpm from
`package.json`) are guessed for you to review. It asks for the URL when
`origin` is not a GitHub-style URL it can turn into https. It refuses to
overwrite an existing `pippwin.toml`.

## `pippwin check`

```text
pippwin check [--project <folder>]
```

Checks a `pippwin.toml` and the folders it names exactly as `setup` and
`run` would, and shows the commands `run` would run. Useful in a project's
CI.
