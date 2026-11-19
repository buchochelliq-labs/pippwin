//! Steps 5 to 8: Swift, xtool, and the Rust and Node.js the project
//! needs, each pinned and verified before it is installed (see
//! `versions.rs`).

use std::path::{Path, PathBuf};

use super::{Ctx, Status, Step};
use crate::checksum;
use crate::cmd::{Cmd, Input};
use crate::download;
use crate::error::{Error, Result};
use crate::parse;
use crate::perms;
use crate::versions::{self, Arch};

// ---------------------------------------------------------------- Swift

pub const SWIFT: Step = Step {
    id: "swift",
    title: "Swift toolchain",
    why: "xtool builds the Swift half of the app with the Swift toolchain for Linux; its guide names \
Swift 6.4. It is installed with swiftly, swift.org's own installer, whose download is checked against \
swift.org's signature first. swiftly checks the toolchain's signature itself.",
    check: check_swift,
    run: run_swift,
    when_done: None,
};

fn check_swift(cx: &mut Ctx) -> Status {
    let Ok(out) = cx.run.capture(&Cmd::new("swift").arg("--version")) else {
        return Status::Missing("swift is not installed".into());
    };
    match parse::swift_version(&out.text()) {
        Some(v) if parse::version_matches(&v, versions::SWIFT_VERSION) => {
            Status::Done(format!("Swift {v}"))
        }
        Some(v) => Status::Problem(format!(
            "Swift {v} is installed; xtool needs {}",
            versions::SWIFT_VERSION
        )),
        None => Status::Missing("swift is not installed".into()),
    }
}

fn run_swift(cx: &mut Ctx) -> Result<()> {
    let arch = cx.host.arch()?;
    let temp = download::temp_dir()?;
    if !cx.run.exists("swiftly") {
        install_swiftly(cx, arch, temp.path())?;
    }
    cx.ui.note(&format!(
        "xtool's Linux guide: {}",
        versions::XTOOL_LINUX_GUIDE
    ));
    let post_install = temp.path().join("post-install.sh");
    cx.ui.note(&format!(
        "Installing Swift {} with swiftly.",
        versions::SWIFT_VERSION
    ));
    cx.run.interactive(
        &Cmd::new("swiftly")
            .args([
                "install",
                versions::SWIFT_VERSION,
                "--use",
                "--assume-yes",
                "--post-install-file",
            ])
            .path_arg(&post_install),
    )?;
    // swiftly writes the system packages the toolchain still needs as an
    // apt-get script; install those packages ourselves rather than run it.
    if let Ok(script) = std::fs::read_to_string(&post_install) {
        let packages = parse::apt_packages_in_script(&script);
        let missing = super::system::missing_packages(cx, &packages);
        super::system::apt_install(cx, &missing, "the packages swiftly says Swift needs")?;
    }
    Ok(())
}

fn install_swiftly(cx: &mut Ctx, arch: Arch, temp: &Path) -> Result<()> {
    let url = versions::swiftly_url(arch);
    let name = format!("swiftly-{}.tar.gz", arch.uname());
    cx.ui
        .note("Downloading swiftly and its signature from download.swift.org.");
    let tarball = download::file(cx.run, &url, temp, &name)?;
    let signature = download::file(cx.run, &format!("{url}.sig"), temp, &format!("{name}.sig"))?;
    let keys = download::file(cx.run, versions::SWIFT_KEYS_URL, temp, "swift-keys.asc")?;
    verify_swift_signature(cx, temp, &keys, &signature, &tarball)?;
    cx.ui
        .done("swiftly's signature is good (swift.org release key).");
    let unpacked = temp.join("swiftly");
    perms::ensure_private_dir(&unpacked)?;
    cx.run.interactive(
        &Cmd::new("tar")
            .arg("-xzf")
            .path_arg(&tarball)
            .arg("-C")
            .path_arg(&unpacked)
            .input(Input::Null),
    )?;
    cx.run
        .interactive(&Cmd::new(unpacked.join("swiftly").to_string_lossy()).args([
            "init",
            "--quiet-shell-followup",
            "--assume-yes",
            "--skip-install",
        ]))
}

fn verify_swift_signature(
    cx: &mut Ctx,
    temp: &Path,
    keys: &Path,
    signature: &Path,
    file: &Path,
) -> Result<()> {
    let home = temp.join("gnupg");
    perms::ensure_private_dir(&home)?;
    let gpg = || {
        Cmd::new("gpg")
            .args(["--batch", "--no-tty", "--homedir"])
            .path_arg(&home)
    };
    let imported = cx.run.capture(&gpg().arg("--import").path_arg(keys))?;
    let what = "swiftly".to_string();
    if !imported.success {
        return Err(Error::SignatureInvalid { what });
    }
    let verified = cx.run.capture(
        &gpg()
            .args(["--status-fd", "1", "--verify"])
            .path_arg(signature)
            .path_arg(file),
    )?;
    let signer = verified
        .success
        .then(|| parse::gpg_valid_signature(&verified.stdout))
        .flatten();
    match signer {
        Some(fpr) if versions::SWIFT_SIGNING_FINGERPRINTS.contains(&fpr.as_str()) => Ok(()),
        _ => {
            let _ = std::fs::remove_file(file);
            Err(Error::SignatureInvalid { what })
        }
    }
}

// ---------------------------------------------------------------- xtool

pub const XTOOL: Step = Step {
    id: "xtool",
    title: "xtool",
    why: "xtool replaces Xcode here: it builds, signs and installs the app. Its AppImage is downloaded \
from xtool's GitHub release (a pinned version) into ~/.local/bin, and checked against the SHA-256 \
recorded for that release before it is made executable.",
    check: check_xtool,
    run: run_xtool,
    when_done: None,
};

pub fn xtool_path(cx: &Ctx) -> PathBuf {
    cx.paths.local_bin().join("xtool")
}

fn check_xtool(cx: &mut Ctx) -> Status {
    let Ok(out) = cx.run.capture(&Cmd::new("xtool").arg("--version")) else {
        return Status::Missing("xtool is not installed".into());
    };
    if !out.success {
        return Status::Missing("xtool is not installed or does not start".into());
    }
    let text = out.text();
    let version = text.split_whitespace().last().unwrap_or("").to_string();
    if version == versions::XTOOL_VERSION {
        Status::Done(format!("xtool {version}"))
    } else {
        Status::Problem(format!(
            "xtool {version} is installed; this tool is tested with {}",
            versions::XTOOL_VERSION
        ))
    }
}

fn run_xtool(cx: &mut Ctx) -> Result<()> {
    let arch = cx.host.arch()?;
    let asset = versions::xtool_asset(arch);
    let mut expected: Vec<String> = Vec::new();
    if let Some(pinned) = arch.xtool_sha256() {
        expected.push(pinned.to_string());
    }
    match download::text(
        cx.run,
        &versions::xtool_release_api(),
        "xtool's release metadata",
    )
    .ok()
    .and_then(|json| parse::github_asset_digest(&json, &asset))
    {
        Some(digest) => expected.push(digest),
        None if expected.is_empty() => {
            return Err(Error::ChecksumUnavailable {
                what: format!("xtool {}", versions::XTOOL_VERSION),
            })
        }
        None => cx
            .ui
            .note("GitHub's record of the checksum could not be read; using the pinned one."),
    }
    let temp = download::temp_dir()?;
    cx.ui.note(&format!(
        "Downloading xtool {} ({asset}).",
        versions::XTOOL_VERSION
    ));
    let file = download::file(cx.run, &versions::xtool_url(arch), temp.path(), &asset)?;
    let refs: Vec<&str> = expected.iter().map(String::as_str).collect();
    let sum = checksum::verify(&file, "the xtool AppImage", &refs)?;
    cx.ui.done(&format!("SHA-256 {sum} matches."));
    let bin = cx.paths.local_bin();
    std::fs::create_dir_all(&bin).map_err(|e| Error::file(&bin, &e))?;
    let target = xtool_path(cx);
    let staged = bin.join(".xtool.new");
    std::fs::copy(&file, &staged).map_err(|e| Error::file(&staged, &e))?;
    perms::set_mode(&staged, 0o755)?;
    std::fs::rename(&staged, &target).map_err(|e| Error::file(&target, &e))?;
    if !path_check(cx).is_done() {
        cx.ui.note(
            "~/.local/bin is not on your PATH yet; Ubuntu adds it when you open a new terminal.",
        );
    }
    Ok(())
}

/// Whether `~/.local/bin` is on the user's own `PATH`.
pub fn path_check(cx: &mut Ctx) -> Status {
    let bin = cx.paths.local_bin();
    let on_path = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d == bin))
        .unwrap_or(false);
    if on_path {
        Status::Done(String::new())
    } else {
        Status::Problem("open a new terminal (Ubuntu's ~/.profile adds it once it exists)".into())
    }
}

// ---------------------------------------------------------------- Rust

pub const RUST: Step = Step {
    id: "rust",
    title: "Rust and its targets",
    why: "When the project's pippwin.toml says it needs Rust, rustup installs the toolchain the project's \
own rust-toolchain.toml names, and the targets it lists (aarch64-apple-ios for iPhones). If rustup is \
missing, its installer is downloaded (a pinned version) and checked against the pinned and the published \
SHA-256 first.",
    check: check_rust,
    run: run_rust,
    when_done: None,
};

/// The project, when it needs Rust.
fn rust_project(cx: &Ctx) -> Option<(PathBuf, Vec<String>)> {
    let project = cx.project.as_ref()?;
    project.file.needs.rust.then(|| {
        (
            project.root.clone(),
            project.file.needs.rust_targets.clone(),
        )
    })
}

fn has_toolchain_file(root: &Path) -> bool {
    root.join("rust-toolchain.toml").is_file() || root.join("rust-toolchain").is_file()
}

/// `rustup` run in the project's folder (so its rust-toolchain.toml
/// applies), never installing anything by itself.
fn rustup_probe(root: &Path) -> Cmd {
    Cmd::new("rustup").cwd(root).env("RUSTUP_AUTO_INSTALL", "0")
}

fn check_rust(cx: &mut Ctx) -> Status {
    let Some(project) = cx.project.as_ref() else {
        return Status::Unknown("depends on the project".into());
    };
    let name = project.name().to_string();
    let Some((root, targets)) = rust_project(cx) else {
        return Status::Done(format!("{name} does not need Rust"));
    };
    if !cx.run.exists("rustup") {
        return Status::Missing("rustup is not installed".into());
    }
    let installed = cx
        .run
        .capture(&rustup_probe(&root).args(["target", "list", "--installed"]));
    let source = if has_toolchain_file(&root) {
        "the project's rust-toolchain.toml"
    } else {
        "rustup's default toolchain"
    };
    match installed {
        Ok(out) if out.success => {
            let have: Vec<&str> = out.stdout.lines().map(str::trim).collect();
            let missing: Vec<&String> = targets
                .iter()
                .filter(|t| !have.contains(&t.as_str()))
                .collect();
            if missing.is_empty() {
                let with = if targets.is_empty() {
                    String::new()
                } else {
                    format!(" with {}", targets.join(", "))
                };
                Status::Done(format!("Rust from {source}{with}"))
            } else {
                Status::Missing(format!(
                    "targets not installed: {}",
                    missing
                        .iter()
                        .map(|t| t.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
        }
        _ => Status::Missing(format!("the toolchain from {source} is not installed")),
    }
}

/// The commands that install the project's toolchain and targets, run in
/// its folder.
pub fn rust_install_cmds(root: &Path, targets: &[String], toolchain_file: bool) -> Vec<Cmd> {
    let mut cmds = Vec::new();
    if toolchain_file {
        // rustup 1.28 and later install the toolchain the file names.
        cmds.push(Cmd::new("rustup").args(["toolchain", "install"]).cwd(root));
    } else {
        cmds.push(Cmd::new("rustup").args(["default", "stable"]).cwd(root));
    }
    if !targets.is_empty() {
        cmds.push(
            Cmd::new("rustup")
                .args(["target", "add"])
                .args(targets.iter().cloned())
                .cwd(root),
        );
    }
    cmds
}

fn run_rust(cx: &mut Ctx) -> Result<()> {
    let Some((root, targets)) = rust_project(cx) else {
        return Ok(());
    };
    if !cx.run.exists("rustup") {
        let arch = cx.host.arch()?;
        let temp = download::temp_dir()?;
        let url = versions::rustup_url(arch);
        let published = download::text(cx.run, &format!("{url}.sha256"), "rustup-init's checksum")?;
        let published = parse::sha256_file(&published).ok_or(Error::ChecksumUnavailable {
            what: "rustup-init".into(),
        })?;
        let file = download::file(cx.run, &url, temp.path(), "rustup-init")?;
        checksum::verify(&file, "rustup-init", &[arch.rustup_sha256(), &published])?;
        cx.ui.done("rustup-init's SHA-256 matches.");
        perms::set_mode(&file, 0o700)?;
        cx.run.interactive(&Cmd::new(file.to_string_lossy()).args([
            "-y",
            "--profile",
            "minimal",
            "--default-toolchain",
            "none",
        ]))?;
    }
    for cmd in rust_install_cmds(&root, &targets, has_toolchain_file(&root)) {
        cx.run.interactive(&cmd)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- Node

pub const NODE: Step = Step {
    id: "node",
    title: "Node.js and pnpm",
    why: "When the project's pippwin.toml names a Node.js major, pippwin installs its own pinned release \
of that major from nodejs.org's official tarball, checked against the release's SHASUMS256.txt and the \
pinned checksum, into ~/.local. pnpm, when asked for, comes through corepack, which ships with Node.js \
and checks what it downloads.",
    check: check_node,
    run: run_node,
    when_done: None,
};

fn node_ok(cx: &mut Ctx, major: u32) -> Option<String> {
    let out = cx.run.capture(&Cmd::new("node").arg("--version")).ok()?;
    (out.success && parse::node_major(&out.stdout) == Some(major))
        .then(|| out.stdout.trim().to_string())
}

fn pnpm_ok(cx: &mut Ctx, version: &str) -> bool {
    cx.run
        .capture(
            &Cmd::new("pnpm")
                .arg("--version")
                .env("COREPACK_ENABLE_DOWNLOAD_PROMPT", "0"),
        )
        .is_ok_and(|o| o.success && o.stdout.trim() == version)
}

/// The Node.js release and pnpm version the project needs.
fn node_needs(cx: &Ctx) -> Option<(Option<&'static versions::NodeRelease>, Option<String>)> {
    let project = cx.project.as_ref()?;
    Some((project.file.node_release(), project.file.needs.pnpm.clone()))
}

fn check_node(cx: &mut Ctx) -> Status {
    let Some((release, pnpm)) = node_needs(cx) else {
        return Status::Unknown("depends on the project".into());
    };
    let Some(release) = release else {
        return Status::Done(format!("{} does not need Node.js", cx.project_name()));
    };
    let Some(node) = node_ok(cx, release.major) else {
        return Status::Missing(format!("Node.js {} is not installed", release.major));
    };
    match pnpm {
        None => Status::Done(format!("Node.js {node}")),
        Some(pnpm) if pnpm_ok(cx, &pnpm) => Status::Done(format!("Node.js {node}, pnpm {pnpm}")),
        Some(pnpm) => Status::Missing(format!("Node.js {node}; pnpm {pnpm} missing")),
    }
}

fn run_node(cx: &mut Ctx) -> Result<()> {
    let Some((Some(release), pnpm)) = node_needs(cx) else {
        return Ok(());
    };
    if node_ok(cx, release.major).is_none() {
        install_node(cx, release)?;
    }
    let Some(pnpm) = pnpm else {
        return Ok(());
    };
    let bin = cx.paths.local_bin();
    cx.run.interactive(
        &Cmd::new("corepack")
            .args(["enable", "--install-directory"])
            .path_arg(&bin)
            .arg("pnpm"),
    )?;
    cx.run.interactive(
        &Cmd::new("corepack")
            .args(["prepare", &format!("pnpm@{pnpm}"), "--activate"])
            .env("COREPACK_ENABLE_DOWNLOAD_PROMPT", "0"),
    )
}

fn install_node(cx: &mut Ctx, release: &versions::NodeRelease) -> Result<()> {
    let arch = cx.host.arch()?;
    let tarball = versions::node_tarball(release, arch);
    let shasums = download::text(
        cx.run,
        &versions::node_url(release, "SHASUMS256.txt"),
        "Node.js' SHASUMS256.txt",
    )?;
    let listed = parse::shasums_entry(&shasums, &tarball).ok_or(Error::ChecksumUnavailable {
        what: tarball.clone(),
    })?;
    let temp = download::temp_dir()?;
    cx.ui
        .note(&format!("Downloading Node.js {}.", release.version));
    let file = download::file(
        cx.run,
        &versions::node_url(release, &tarball),
        temp.path(),
        &tarball,
    )?;
    checksum::verify(&file, &tarball, &[arch.node_sha256(release), &listed])?;
    cx.ui
        .done("Node.js' SHA-256 matches SHASUMS256.txt and the pinned value.");
    let lib = cx.paths.local.join("lib/nodejs");
    std::fs::create_dir_all(&lib).map_err(|e| Error::file(&lib, &e))?;
    cx.run.interactive(
        &Cmd::new("tar")
            .arg("-xJf")
            .path_arg(&file)
            .arg("-C")
            .path_arg(&lib)
            .input(Input::Null),
    )?;
    let dir = lib.join(tarball.trim_end_matches(".tar.xz"));
    let bin = cx.paths.local_bin();
    std::fs::create_dir_all(&bin).map_err(|e| Error::file(&bin, &e))?;
    for tool in ["node", "npm", "npx", "corepack"] {
        link_tool(&dir.join("bin").join(tool), &bin.join(tool))?;
    }
    Ok(())
}

/// Point `link` at `target`, replacing an older link but never a file.
fn link_tool(target: &Path, link: &Path) -> Result<()> {
    match std::fs::symlink_metadata(link) {
        Ok(meta) if meta.file_type().is_symlink() => {
            std::fs::remove_file(link).map_err(|e| Error::file(link, &e))?;
        }
        Ok(_) => {
            return Err(Error::File {
                path: link.to_path_buf(),
                reason: "a file is already there; move it away and run setup again".into(),
            })
        }
        Err(_) => {}
    }
    std::os::unix::fs::symlink(target, link).map_err(|e| Error::file(link, &e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::fake::FakeRunner;
    use crate::paths::Paths;
    use crate::project::tests::{make_repo, LIMIARA};
    use crate::project::Project;
    use crate::state::State;
    use crate::steps::Host;
    use crate::ui::testing::ScriptUi;

    #[test]
    fn rust_follows_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let repo = make_repo(dir.path(), LIMIARA);
        std::fs::write(
            repo.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.95.0\"\n",
        )
        .unwrap();
        let project = Project::load(&repo).unwrap();
        let root = project.root.clone();
        let mut ui = ScriptUi::new([]);
        let mut run = FakeRunner::default();
        run.programs.push("rustup".into());
        let mut run = run.answer(
            "rustup target list --installed",
            "x86_64-unknown-linux-gnu\n",
        );
        let mut cx = Ctx::new(
            &mut ui,
            &mut run,
            Paths::from_home(dir.path().to_path_buf()),
            State::default(),
            Host {
                wsl: true,
                os: None,
                arch: None,
            },
        );
        assert!(matches!(check_rust(&mut cx), Status::Unknown(_)));
        cx.project = Some(project);
        assert_eq!(
            check_rust(&mut cx),
            Status::Missing("targets not installed: aarch64-apple-ios".into())
        );
        run_rust(&mut cx).unwrap();
        // Probes never install a toolchain by themselves, and run where the
        // project's rust-toolchain.toml applies.
        let probe = cx_probe(&run.captured);
        assert_eq!(probe.cwd.as_deref(), Some(root.as_path()));
        assert!(probe
            .env
            .contains(&("RUSTUP_AUTO_INSTALL".into(), "0".into())));
        let shown: Vec<String> = run.ran.iter().map(Cmd::display).collect();
        assert_eq!(
            shown,
            [
                "rustup toolchain install",
                "rustup target add aarch64-apple-ios"
            ]
        );
        assert!(run
            .ran
            .iter()
            .all(|c| c.cwd.as_deref() == Some(root.as_path())));
    }

    fn cx_probe(captured: &[Cmd]) -> Cmd {
        captured
            .iter()
            .find(|c| c.program == "rustup")
            .cloned()
            .unwrap()
    }

    #[test]
    fn without_a_toolchain_file_stable_is_the_default() {
        let cmds = rust_install_cmds(Path::new("/p"), &[], false);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].display(), "rustup default stable");
    }

    #[test]
    fn tools_a_project_does_not_need_are_done() {
        let dir = tempfile::tempdir().unwrap();
        let minimal = "schema = 1\n[project]\nname = \"Plain\"\nrepository = \"https://example.org/a/plain\"\n[app]\npath = \"apps/device-shell/ios-xtool\"\n";
        let repo = make_repo(dir.path(), minimal);
        let mut ui = ScriptUi::new([]);
        let mut run = FakeRunner::default();
        let mut cx = Ctx::new(
            &mut ui,
            &mut run,
            Paths::from_home(dir.path().to_path_buf()),
            State::default(),
            Host {
                wsl: true,
                os: None,
                arch: None,
            },
        );
        cx.project = Some(Project::load(&repo).unwrap());
        assert_eq!(
            check_rust(&mut cx),
            Status::Done("Plain does not need Rust".into())
        );
        assert_eq!(
            check_node(&mut cx),
            Status::Done("Plain does not need Node.js".into())
        );
        run_rust(&mut cx).unwrap();
        run_node(&mut cx).unwrap();
        assert!(run.ran.is_empty() && run.captured.is_empty());
    }

    #[test]
    fn links_replace_links_not_files() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("node");
        std::fs::write(&target, b"").unwrap();
        let link = dir.path().join("bin-node");
        link_tool(&target, &link).unwrap();
        link_tool(&target, &link).unwrap();
        assert_eq!(std::fs::read_link(&link).unwrap(), target);
        let file = dir.path().join("plain");
        std::fs::write(&file, b"keep").unwrap();
        assert!(link_tool(&target, &file).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"keep");
    }
}
