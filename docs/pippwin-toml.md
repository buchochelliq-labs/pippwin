# `pippwin.toml` reference

A project tells pippwin how to build it with a `pippwin.toml` at the root of
its repository. `pippwin init` writes a commented one for you to review;
`pippwin check` checks one.

## Example

```toml
schema = 1

[project]
name = "My App"
repository = "https://github.com/owner/my-app"
branch = "main"

[app]
path = "ios"

[build]
prepare = ["./prepare.sh"]

[needs]
rust = true
rust_targets = ["aarch64-apple-ios"]
node = "22"
pnpm = "9.12.0"
apt = []
```

## Fields

### `schema` (required)

The file's format version. This pippwin reads `schema = 1`; any other
number is refused with a request to update pippwin.

### `[project]` (required)

| Field | Required | Meaning |
|---|---|---|
| `name` | yes | Shown in pippwin's screens. 1 to 64 characters, no control characters. |
| `repository` | yes | The https git URL pippwin clones, such as `https://github.com/owner/repo`. |
| `branch` | no, default `"main"` | The branch checked out by default. |

### `[app]` (required)

| Field | Required | Meaning |
|---|---|---|
| `path` | yes | The folder holding the xtool project (`xtool.yml` and `Package.swift`), relative to the folder holding `pippwin.toml`. `"."` when they are at the root. |

pippwin reads `bundleID` from `xtool.yml` to show it; it does not change the
xtool project.

### `[build]` (optional)

| Field | Required | Meaning |
|---|---|---|
| `prepare` | no | A program and its arguments, run in `app.path` before `xtool dev`. Left out or `[]`: nothing runs before `xtool dev`. |

`prepare` is an **argument vector, not a shell command line**: pippwin runs
the program directly, with each list item as one argument, so spaces,
quotes, `;`, `&&` or `$(…)` in an argument are passed on as they are and
never interpreted. To run several commands, put them in a script inside
`app.path` and name the script.

### `[needs]` (optional)

| Field | Default | Meaning |
|---|---|---|
| `rust` | `false` | Install rustup. The project's own `rust-toolchain.toml` (at the folder holding `pippwin.toml`) picks the toolchain; without one, rustup's stable toolchain is used. |
| `rust_targets` | `[]` | Rust targets to add, such as `"aarch64-apple-ios"`. Needs `rust = true`. |
| `node` | none | A Node.js major, as a string (`"22"`). pippwin installs its own pinned, checksummed release of that major. Supported majors: **22** (Node.js 22.23.5). |
| `pnpm` | none | An exact pnpm version (`"9.12.0"`), activated through corepack. Needs `node`. |
| `apt` | `[]` | Extra Ubuntu packages the build needs, installed with apt after asking. |

Swift, xtool, the iOS SDK and the system packages xtool needs are always
installed; they are not listed here.

## Validation rules

pippwin refuses the file, with error `PW070_PROJECT_FILE` and a sentence
naming the field, when:

- **a key is unknown**, at the top level or in any table (a typo is an
  error, not ignored);
- `schema` is not `1`, or a required field is missing;
- `project.repository` is not `https://host/path`, or holds a user name or
  token (`https://user:token@…`), a query or a fragment;
- `project.branch` is not a plain branch name (letters, digits, `/`, `-`,
  `_`, `.`; not starting with `-`, no `..`);
- `app.path` is absolute, uses `..`, `~` or backslashes, does not exist,
  is not a folder, has no `xtool.yml` or `Package.swift`, or **leads
  outside the repository once symlinks are followed**;
- `build.prepare` is a string rather than a list, or its first item is not
  one program: it must be a relative path inside `app.path` such as
  `"./prepare.sh"` (no `..`, and still inside `app.path` once symlinks are
  followed) or a bare command name such as `"make"`, made only of letters,
  digits, `.`, `_`, `-`, `+` and `/`. Shells and command runners (`sh`,
  `bash`, `zsh`, `env`, `xargs`, `sudo` and the like) are refused as the
  program;
- a `needs.apt` entry is not an Ubuntu package name: lowercase letters,
  digits, `+`, `-` and `.`, at least two characters, starting with a letter
  or digit (so no options such as `-y`, no versions such as `pkg=1.0`);
- a `needs.rust_targets` entry is not a target name, or `rust_targets` is
  set without `rust = true`;
- `needs.node` is not a supported major (pippwin says which majors it
  supports), or is a number instead of a string;
- `needs.pnpm` is not an exact `x.y.z` version, or is set without
  `needs.node`;
- the file is larger than 64 KB, or a list has more than 64 entries.

## Worked example: Limiara

[Limiara](https://github.com/buchochelliq-labs/limiara-core) builds its iOS
shell with xtool from `apps/device-shell/ios-xtool`. Its shell is written in
Rust and embeds a web shell built with Node.js and pnpm, so its file is:

```toml
schema = 1

[project]
name = "Limiara"
repository = "https://github.com/buchochelliq-labs/limiara-core"
branch = "main"

[app]
# The xtool project: xtool.yml, Package.swift and prepare.sh.
path = "apps/device-shell/ios-xtool"

[build]
# Builds the web shell and the Rust shell for aarch64-apple-ios, and
# copies the Swift packages, before `xtool dev`.
prepare = ["./prepare.sh"]

[needs]
# rust-toolchain.toml at the repository root picks the toolchain.
rust = true
rust_targets = ["aarch64-apple-ios"]
node = "22"
pnpm = "9.12.0"
```

With it, `pippwin setup --project https://github.com/buchochelliq-labs/limiara-core`
clones Limiara into `~/limiara-core`, installs rustup with the toolchain
Limiara's `rust-toolchain.toml` pins and the `aarch64-apple-ios` target,
Node.js 22.23.5 and pnpm 9.12.0, and `pippwin run` runs
`./prepare.sh` and then `xtool dev` in `apps/device-shell/ios-xtool`.
