# Maintaining pippwin

## Layout

| Path | What |
|---|---|
| `src/` | The `pippwin` binary. `versions.rs` holds every pin; `project.rs` the `pippwin.toml` schema; `steps/` the setup steps. |
| `scripts/setup-windows.ps1` | The Windows bootstrap. |
| `scripts/bootstrap.sh` | The WSL bootstrap. |
| `docs/`, `zensical.toml` | This site. |
| `.github/workflows/ci.yml` | fmt, clippy, tests, shellcheck, an `init` round trip. |
| `.github/workflows/docs.yml` | Builds this site; deploys it from `main`. |

pippwin's UI uses the rs-rich `=0.0.17` and rs-rich-intuituive `=0.0.12`
crates, pinned exactly. Two gaps in rs-rich are worked around in
`src/ui.rs` and tracked upstream:
[rs-rich-cli#912](https://github.com/buchochelliq-labs/rs-rich-cli/issues/912)
(no hidden input; pippwin never needs it, since it takes no secret itself)
and [rs-rich-cli#913](https://github.com/buchochelliq-labs/rs-rich-cli/issues/913)
(no validation hook on `Prompt`; the line-prompt fallback re-asks itself).

## Checks to run

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
shellcheck scripts/*.sh
actionlint .github/workflows/*.yml
pip install -r docs/requirements.txt && zensical build --strict
```

`rust-toolchain.toml` pins Rust 1.95.0 with clippy and rustfmt;
`Cargo.toml` declares `rust-version = "1.90"`. `Cargo.lock` is committed and
every build uses `--locked`. The test `limiaras_real_file_parses` checks a
real project file when one is on the machine (`PIPPWIN_LIMIARA_TOML`, or a
limiara-core checkout at the path in the test) and is skipped otherwise.

What tests cannot cover: `setup-windows.ps1`, WSL interop (`usbipd.exe`,
`explorer.exe`, `powershell.exe`), Apple's login and a real iPhone. Try
those by hand on Windows before a release.

## Updating the pins

All pins are in `src/versions.rs`; change them there only, in a reviewed
change. `scripts/bootstrap.sh` reads the rustup pins from that file.

- **Node.js.** Each supported major has one entry in `NODE_RELEASES`. Take
  the latest release of that major from <https://nodejs.org/dist/index.json>,
  and the `node-v<version>-linux-x64.tar.xz` and
  `node-v<version>-linux-arm64.tar.xz` lines of that release's
  `https://nodejs.org/dist/v<version>/SHASUMS256.txt`. Check the
  `SHASUMS256.txt` signature (`SHASUMS256.txt.sig`) against the Node.js
  release keys before you copy the sums. Adding a major means adding an
  entry; the docs list the supported majors.
- **rustup.** The version from
  <https://static.rust-lang.org/rustup/release-stable.toml>, and each
  `https://static.rust-lang.org/rustup/archive/<version>/<target>/rustup-init.sha256`
  for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`.
- **Swift.** `SWIFT_VERSION` follows xtool's
  [Linux guide](https://github.com/xtool-org/xtool/blob/main/Documentation/xtool.docc/Installation-Linux.md).
  The signing-key fingerprints in `SWIFT_SIGNING_FINGERPRINTS` come from
  <https://www.swift.org/keys/all-keys.asc>
  (`gpg --show-keys --with-fingerprint all-keys.asc`); keep only the keys
  that sign swiftly releases.
- **xtool.** Set `XTOOL_VERSION` to a release tag, then record each
  AppImage's SHA-256 in `XTOOL_SHA256_X86_64` and `XTOOL_SHA256_AARCH64`:
  download it, run `sha256sum xtool-x86_64.AppImage`, and compare with the
  asset's `digest` in
  `https://api.github.com/repos/xtool-org/xtool/releases/tags/<tag>`. Also
  update `XTOOL_SDK_VERSION` from that tag's
  `Sources/XToolSupport/SDKBuilder.swift` (`currentSDKVersion`),
  `XCODE_VERSION` from its Linux guide, and check that the xtool texts
  pippwin repeats (in `src/steps/login.rs`, `sdk.rs` and `build.rs`) still
  match xtool's source word for word.
- **Rust for pippwin itself.** `rust-toolchain.toml`; keep `rust-version`
  in `Cargo.toml` at or below it.

## Follow-up

- The xtool AppImage's SHA-256 is not pinned yet (`XTOOL_SHA256_*` are
  `None`): it could not be downloaded where pippwin was written. Until it
  is, pippwin relies on the digest GitHub records for the release asset,
  and refuses to install xtool when that cannot be read.
- `setup-windows.ps1`, the WSL-only steps, Apple's login and the phone are
  exercised by hand only.

## Releases

pippwin is built from source by the bootstrap scripts, so a release is a
git tag:

1. Make sure `main` is green (CI and docs).
2. Bump `version` in `Cargo.toml`, run `cargo build --locked` so
   `Cargo.lock` follows, and commit.
3. Tag it (`git tag -s v0.2.0`) and push the tag. Users can build a tag
   with `setup-windows.ps1 -PippwinRef v0.2.0` or
   `bootstrap.sh --pippwin-ref v0.2.0`.

## The documentation site

The site is built with [Zensical](https://zensical.org) (pinned in
`docs/requirements.txt`) from `docs/` and `zensical.toml`, and published to
<https://buchochelliq-labs.github.io/pippwin/> by
`.github/workflows/docs.yml`: pull requests that touch the docs build it with
`zensical build --strict`; pushes to `main` (and manual runs) build and
deploy it through the `github-pages` environment.

GitHub Pages has to be turned on **once** for the repository: **Settings →
Pages → Build and deployment → Source: GitHub Actions**.

To preview locally:

```sh
python3 -m venv .venv && .venv/bin/pip install -r docs/requirements.txt
.venv/bin/zensical serve
```
