# Troubleshooting

Start with `pippwin doctor`: it shows every check and what is wrong.

## `AFCClient.Error.muxError`

xtool's own guide covers this: when xtool fails with
`Error: AFCClient.Error.muxError`, follow the steps in
[this issue comment](https://github.com/xtool-org/xtool/issues/19#issuecomment-2898986718).
That workaround relays the device connection from iTunes on Windows instead
of relying on usbipd.

## The iPhone is not found after reconnecting it

usbipd-win **binds** a device once, but it must be **attached** to WSL again
every time it is plugged in, after the PC sleeps, or after WSL restarts.
Run

```sh
pippwin phone
```

or, in Windows (no administrator rights needed once it is bound):

```powershell
usbipd list
usbipd attach --wsl --busid <BUSID>
```

Then unlock the iPhone. Check that WSL sees it with `idevice_id -l`. If
`usbmuxd` is not running, `pippwin phone` starts it (with sudo).

## "Trust This Computer?" / "Please tap 'trust' on your device..."

Unlock the iPhone, tap **Trust** and enter its passcode. xtool waits for it.

## The app does not open: "Untrusted Developer"

The first time an app signed with your Apple ID is installed, open
**Settings → General → VPN & Device Management** on the iPhone, tap your
developer profile and trust it.

## The app stopped opening after a week

Apps signed with a **free Apple ID** expire after **7 days**. Run
`pippwin run` again in the project's folder to reinstall it (attach the
iPhone first). With a paid Apple Developer Program membership they last a
year.

## "The following certificates must be revoked"

Apple allows only a few development certificates per account. Answering
yes lets xtool revoke the listed ones and make a new one; apps already
signed with them, on any device, stop opening until reinstalled. Answer no
to stop.

## Push notifications or other entitlements do not work

Entitlements such as `aps-environment` (push) need a paid developer team,
and the project's xtool configuration must ask for them. A free Apple ID
cannot sign them.

## The login expired or you want another account

```sh
pippwin login
```

## Xcode.xip is "too small" or the SDK install fails

A complete Xcode.xip is 7 GB or more; a smaller file is an unfinished
download. Extracting needs about 40 GB free in WSL. Check free space with
`df -h ~`. If the SDK is out of date after an xtool update, `pippwin sdk`
follows xtool's own advice (rebuild, or install again from Xcode.xip).

## `~/.local/bin` is not on `PATH`

Ubuntu adds `~/.local/bin` to `PATH` in `~/.profile` once the folder
exists. Open a new terminal (or run `source ~/.profile`).

## A step says "not finished"

Every step checks its own result after it runs. Read the detail, fix what
it names, and run `pippwin setup` again: finished steps are skipped.

## Error codes

| Code | Meaning |
|---|---|
| `PW001_NOT_WSL` | Not running inside WSL. |
| `PW002_UNSUPPORTED_DISTRO` | Not Ubuntu 22.04 or newer. |
| `PW003_UNSUPPORTED_ARCH` | Not x86_64 or aarch64. |
| `PW010_SPAWN` | A program could not be started. |
| `PW011_COMMAND_FAILED` | A program exited with an error; its own output says why. |
| `PW020_DOWNLOAD` | A download failed. |
| `PW021_CHECKSUM_MISMATCH` | A download did not match its checksum and was deleted. |
| `PW022_CHECKSUM_UNAVAILABLE` | No checksum to verify a download, so it was not installed. |
| `PW023_SIGNATURE_INVALID` | swiftly's signature could not be verified. |
| `PW030_INVALID_INPUT` | An answer or option was not accepted. |
| `PW031_INVALID_KEY` | The key file is not a private key. |
| `PW032_UNSAFE_KEY_LOCATION` | The key would end up under `/mnt` or in a git working tree. |
| `PW040_FILE` | A file could not be read or written. |
| `PW041_STATE` | The state file is unreadable; delete `~/.config/pippwin/state.toml` to start afresh. |
| `PW050_CANCELLED` | You stopped it. |
| `PW051_TERMINAL` | The terminal could not be used. |
| `PW060_NOT_READY` | Something needed first is missing; the message says what. |
| `PW070_PROJECT_FILE` | The project's `pippwin.toml` is not valid; see the [reference](pippwin-toml.md#validation-rules). |
