//! A project's `pippwin.toml`: what to clone, which folder holds the xtool
//! project, what to run before `xtool dev`, and which tools it needs.
//!
//! The file is checked strictly: unknown keys are refused, `app.path` must
//! stay inside the repository after symlinks are followed, and
//! `build.prepare` is an argument vector run without a shell. See
//! docs/pippwin-toml.md.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cmd::Cmd;
use crate::error::{Error, Result};
use crate::validate;
use crate::versions::{self, NodeRelease};

/// The only schema version this pippwin reads.
pub const SCHEMA: u32 = 1;

/// A `pippwin.toml` larger than this is refused.
const MAX_FILE_BYTES: u64 = 64 * 1024;
const MAX_LIST: usize = 64;

/// Programs `build.prepare` may not start with: shells and command
/// runners would turn the argument vector back into a shell command line,
/// and the build never needs root.
const REFUSED_PROGRAMS: &[&str] = &[
    "sh", "bash", "dash", "zsh", "ksh", "mksh", "fish", "csh", "tcsh", "busybox", "env", "eval",
    "exec", "xargs", "sudo", "doas", "su", "pkexec",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectFile {
    pub schema: u32,
    pub project: ProjectSection,
    pub app: AppSection,
    #[serde(default)]
    pub build: BuildSection,
    #[serde(default)]
    pub needs: Needs,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSection {
    /// Shown in the UI.
    pub name: String,
    /// The https git URL pippwin clones.
    pub repository: String,
    /// The branch checked out by default.
    #[serde(default = "default_branch")]
    pub branch: String,
}

fn default_branch() -> String {
    "main".into()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppSection {
    /// The folder with `xtool.yml` and `Package.swift`, relative to the
    /// folder holding `pippwin.toml`.
    pub path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildSection {
    /// An argument vector run in `app.path` before `xtool dev`.
    #[serde(default)]
    pub prepare: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Needs {
    #[serde(default)]
    pub rust: bool,
    #[serde(default)]
    pub rust_targets: Vec<String>,
    #[serde(default)]
    pub node: Option<String>,
    #[serde(default)]
    pub pnpm: Option<String>,
    #[serde(default)]
    pub apt: Vec<String>,
}

impl ProjectFile {
    /// Parse and check `text`. Every message names the field.
    pub fn parse(text: &str) -> std::result::Result<ProjectFile, String> {
        // Friendlier messages for the two likeliest mistakes, before serde
        // gives its generic "invalid type".
        if let Ok(value) = text.parse::<toml::Table>() {
            if value
                .get("build")
                .and_then(|b| b.get("prepare"))
                .is_some_and(toml::Value::is_str)
            {
                return Err(
                    "build.prepare is a list of arguments such as [\"./prepare.sh\"], not a shell \
command line"
                        .into(),
                );
            }
            if value
                .get("needs")
                .and_then(|n| n.get("node"))
                .is_some_and(toml::Value::is_integer)
            {
                return Err("needs.node is a string, such as \"22\"".into());
            }
        }
        let file: ProjectFile = toml::from_str(text).map_err(|e| describe_toml_error(text, &e))?;
        file.check()?;
        Ok(file)
    }

    /// The rules serde cannot express.
    pub fn check(&self) -> std::result::Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!(
                "schema = {} is not known to this pippwin, which reads schema {SCHEMA}; update pippwin",
                self.schema
            ));
        }
        let name = self.project.name.trim();
        if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
            return Err(
                "project.name must be 1 to 64 characters, with no control characters".into(),
            );
        }
        validate::https_git_url(&self.project.repository)
            .map_err(|_| "project.repository must be an https git URL such as https://github.com/owner/repo, with no user name or token in it".to_string())?;
        validate::branch(&self.project.branch)
            .map_err(|_| "project.branch is not a usable branch name".to_string())?;
        check_relative_path(&self.app.path).map_err(|why| format!("app.path {why}"))?;
        self.check_prepare()?;
        self.check_needs()
    }

    fn check_prepare(&self) -> std::result::Result<(), String> {
        let argv = &self.build.prepare;
        if argv.is_empty() {
            return Ok(());
        }
        if argv.len() > MAX_LIST {
            return Err(format!("build.prepare has more than {MAX_LIST} arguments"));
        }
        if argv.iter().any(|a| a.contains('\0')) {
            return Err("build.prepare holds a NUL character".into());
        }
        let program = &argv[0];
        let plain = !program.is_empty()
            && program.bytes().all(|b| {
                b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+' | b'/')
            });
        if !plain {
            return Err(format!(
                "build.prepare[0] ({program:?}) must be one program, such as \"./prepare.sh\" or \
\"make\": no spaces or shell syntax (it is run without a shell)"
            ));
        }
        if program.contains('/') {
            check_relative_path(program).map_err(|why| format!("build.prepare[0] {why}"))?;
        } else if program.starts_with('-') || program.starts_with('.') {
            return Err(format!(
                "build.prepare[0] ({program:?}) must be a relative path such as \"./prepare.sh\" or a command name"
            ));
        }
        let base = program.rsplit('/').next().unwrap_or(program);
        if REFUSED_PROGRAMS.contains(&base) {
            return Err(format!(
                "build.prepare[0] may not be {base}: put the commands in a script inside app.path and \
name the script (pippwin runs prepare without a shell, and never as root)"
            ));
        }
        Ok(())
    }

    fn check_needs(&self) -> std::result::Result<(), String> {
        let needs = &self.needs;
        if needs.rust_targets.len() > MAX_LIST || needs.apt.len() > MAX_LIST {
            return Err(format!("a needs list has more than {MAX_LIST} entries"));
        }
        if let Some(bad) = needs
            .rust_targets
            .iter()
            .find(|t| !validate::rust_target(t))
        {
            return Err(format!(
                "needs.rust_targets: {bad:?} is not a Rust target name"
            ));
        }
        if !needs.rust_targets.is_empty() && !needs.rust {
            return Err("needs.rust_targets needs needs.rust = true".into());
        }
        if let Some(bad) = needs.apt.iter().find(|p| !validate::apt_package(p)) {
            return Err(format!(
                "needs.apt: {bad:?} is not an Ubuntu package name (lowercase letters, digits, '+', '-' and '.')"
            ));
        }
        if let Some(node) = &needs.node {
            if self.node_release().is_none() {
                return Err(format!(
                    "needs.node = {node:?} is not supported; pippwin supports Node.js majors: {}",
                    versions::node_majors()
                ));
            }
        }
        if let Some(pnpm) = &needs.pnpm {
            if !validate::release_version(pnpm) {
                return Err(format!(
                    "needs.pnpm = {pnpm:?} must be an exact version such as \"9.12.0\""
                ));
            }
            if needs.node.is_none() {
                return Err(
                    "needs.pnpm needs needs.node (pnpm comes through Node.js' corepack)".into(),
                );
            }
        }
        Ok(())
    }

    /// The pinned Node.js release for `needs.node`.
    pub fn node_release(&self) -> Option<&'static NodeRelease> {
        let major: u32 = self.needs.node.as_deref()?.trim().parse().ok()?;
        versions::node_release(major)
    }
}

fn describe_toml_error(text: &str, error: &toml::de::Error) -> String {
    let message = error.message().trim().to_string();
    match error.span() {
        Some(span) => {
            let line = text[..span.start.min(text.len())].lines().count().max(1);
            format!("line {line}: {message}")
        }
        None => message,
    }
}

/// A path that stays where it is written: relative, with no `..`, no `~`
/// and no backslashes.
fn check_relative_path(value: &str) -> std::result::Result<(), String> {
    if value.is_empty() {
        return Err("is empty".into());
    }
    if value.contains('\\') || value.contains('\0') || value.starts_with('~') {
        return Err(format!(
            "({value:?}) must be a plain relative path with '/'"
        ));
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(format!("({value:?}) must be relative, not absolute"));
    }
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!("({value:?}) may not use '..'"));
    }
    Ok(())
}

/// A project on disk: its checked `pippwin.toml` and the resolved folders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// The folder holding `pippwin.toml` (resolved).
    pub root: PathBuf,
    pub file: ProjectFile,
    /// `app.path`, resolved, inside `root`.
    pub app_dir: PathBuf,
    /// `bundleID` from `xtool.yml`, for display.
    pub bundle_id: Option<String>,
}

impl Project {
    pub fn file_path(root: &Path) -> PathBuf {
        root.join(versions::PROJECT_FILE)
    }

    /// Read and check `root/pippwin.toml`, then the folders it names.
    pub fn load(root: &Path) -> Result<Project> {
        let path = Project::file_path(root);
        let fail = |reason: String| Error::Project {
            path: path.clone(),
            reason,
        };
        let meta = std::fs::metadata(&path).map_err(|e| Error::file(&path, &e))?;
        if meta.len() > MAX_FILE_BYTES {
            return Err(fail("the file is too large".into()));
        }
        let text = std::fs::read_to_string(&path).map_err(|e| Error::file(&path, &e))?;
        let file = ProjectFile::parse(&text).map_err(fail)?;
        let root = std::fs::canonicalize(root).map_err(|e| Error::file(root, &e))?;
        let app_dir = std::fs::canonicalize(root.join(&file.app.path)).map_err(|_| {
            fail(format!(
                "app.path ({}) does not exist in the repository",
                file.app.path
            ))
        })?;
        if !app_dir.starts_with(&root) {
            return Err(fail(format!(
                "app.path ({}) leads outside the repository once symlinks are followed",
                file.app.path
            )));
        }
        if !app_dir.is_dir() {
            return Err(fail(format!(
                "app.path ({}) is not a folder",
                file.app.path
            )));
        }
        for needed in ["xtool.yml", "Package.swift"] {
            if !app_dir.join(needed).is_file() {
                return Err(fail(format!(
                    "app.path ({}) has no {needed}; it must be the xtool project's folder",
                    file.app.path
                )));
            }
        }
        if let Some(program) = file.build.prepare.first().filter(|p| p.contains('/')) {
            let resolved = std::fs::canonicalize(app_dir.join(program)).map_err(|_| {
                fail(format!(
                    "build.prepare[0] ({program}) does not exist in app.path"
                ))
            })?;
            if !resolved.starts_with(&app_dir) || !resolved.is_file() {
                return Err(fail(format!(
                    "build.prepare[0] ({program}) must be a file inside app.path, also once symlinks are followed"
                )));
            }
        }
        let bundle_id = std::fs::read_to_string(app_dir.join("xtool.yml"))
            .ok()
            .and_then(|yaml| xtool_bundle_id(&yaml));
        Ok(Project {
            root,
            file,
            app_dir,
            bundle_id,
        })
    }

    pub fn name(&self) -> &str {
        self.file.project.name.trim()
    }

    /// `build.prepare`, run in `app.path` under `umask 077`, with no shell
    /// (the umask wrapper passes the arguments through as they are).
    pub fn prepare_cmd(&self) -> Option<Cmd> {
        let (program, args) = self.file.build.prepare.split_first()?;
        Some(
            Cmd::new(program.as_str())
                .args(args.iter().cloned())
                .cwd(&self.app_dir)
                .private_umask(),
        )
    }

    pub fn xtool_dev_cmd(&self) -> Cmd {
        Cmd::new("xtool")
            .arg("dev")
            .cwd(&self.app_dir)
            .private_umask()
    }

    /// What `pippwin run` runs, in order.
    pub fn build_cmds(&self) -> Vec<Cmd> {
        self.prepare_cmd()
            .into_iter()
            .chain(std::iter::once(self.xtool_dev_cmd()))
            .collect()
    }
}

/// The nearest folder from `start` upwards that holds `pippwin.toml`,
/// stopping at the root of the git working tree.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        if Project::file_path(dir).is_file() {
            return Some(dir.to_path_buf());
        }
        if dir.join(".git").exists() {
            return None;
        }
    }
    None
}

/// `bundleID` from an `xtool.yml` (a top-level `bundleID: value` line).
pub fn xtool_bundle_id(yaml: &str) -> Option<String> {
    yaml.lines().find_map(|line| {
        let value = line.strip_prefix("bundleID:")?;
        let value = value.split(" #").next()?.trim();
        let value = value.trim_matches(|c| c == '"' || c == '\'');
        (!value.is_empty()
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-')))
        .then(|| value.to_string())
    })
}

// ------------------------------------------------------------- pippwin init

/// What `pippwin init` writes into a new `pippwin.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitAnswers {
    pub name: String,
    pub repository: String,
    pub branch: String,
    pub app_path: String,
    pub prepare: Vec<String>,
    pub rust: bool,
    pub rust_targets: Vec<String>,
    pub node: Option<String>,
    pub pnpm: Option<String>,
}

fn quote(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

fn quote_list(values: &[String]) -> String {
    let items: Vec<String> = values.iter().map(|v| quote(v)).collect();
    format!("[{}]", items.join(", "))
}

/// A commented `pippwin.toml` for `answers`.
pub fn init_text(answers: &InitAnswers) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# pippwin builds this repository's iOS app in WSL and installs it\n\
# on an iPhone with xtool. Reference: {}\n\
schema = {SCHEMA}\n\n",
        versions::PROJECT_FILE_DOCS
    ));
    out.push_str("[project]\n# Shown in pippwin's screens.\n");
    out.push_str(&format!("name = {}\n", quote(&answers.name)));
    out.push_str("# The https git URL pippwin clones, and the branch it checks out.\n");
    out.push_str(&format!("repository = {}\n", quote(&answers.repository)));
    out.push_str(&format!("branch = {}\n\n", quote(&answers.branch)));
    out.push_str(
        "[app]\n# The folder holding xtool.yml and Package.swift, relative to this file.\n",
    );
    out.push_str(&format!("path = {}\n\n", quote(&answers.app_path)));
    out.push_str(
        "[build]\n# Optional: a program and its arguments, run in app.path before `xtool dev`.\n\
# It runs without a shell: put several commands in a script and name it here.\n",
    );
    if answers.prepare.is_empty() {
        out.push_str("# prepare = [\"./prepare.sh\"]\n\n");
    } else {
        out.push_str(&format!("prepare = {}\n\n", quote_list(&answers.prepare)));
    }
    out.push_str("[needs]\n# rustup; the repository's rust-toolchain.toml picks the toolchain.\n");
    out.push_str(&format!("rust = {}\n", answers.rust));
    if answers.rust_targets.is_empty() {
        out.push_str("# rust_targets = [\"aarch64-apple-ios\"]\n");
    } else {
        out.push_str(&format!(
            "rust_targets = {}\n",
            quote_list(&answers.rust_targets)
        ));
    }
    out.push_str(&format!(
        "# A Node.js major ({} supported) and an exact pnpm version (through corepack).\n",
        versions::node_majors()
    ));
    match &answers.node {
        Some(node) => out.push_str(&format!("node = {}\n", quote(node))),
        None => out.push_str("# node = \"22\"\n"),
    }
    match &answers.pnpm {
        Some(pnpm) => out.push_str(&format!("pnpm = {}\n", quote(pnpm))),
        None => out.push_str("# pnpm = \"9.12.0\"\n"),
    }
    out.push_str("# Extra Ubuntu packages the build needs.\napt = []\n");
    out
}

/// Folders under `root` (a few levels deep) that hold an `xtool.yml`,
/// relative to `root`, for `init` to offer.
pub fn xtool_folders(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, depth: usize, found: &mut Vec<String>) {
        if dir.join("xtool.yml").is_file() {
            if let Ok(rel) = dir.strip_prefix(root) {
                let rel = rel.to_string_lossy();
                found.push(if rel.is_empty() {
                    ".".into()
                } else {
                    rel.into_owned()
                });
            }
        }
        if depth == 0 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                !name.starts_with('.')
                    && !matches!(name.as_ref(), "node_modules" | "target" | "build")
            })
            .map(|e| e.path())
            .collect();
        dirs.sort();
        for sub in dirs {
            walk(root, &sub, depth - 1, found);
        }
    }
    let mut found = Vec::new();
    walk(root, root, 4, &mut found);
    found
}

/// `"packageManager": "pnpm@9.12.0"` in a `package.json`.
pub fn pnpm_from_package_json(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let manager = value.get("packageManager")?.as_str()?;
    let version = manager.strip_prefix("pnpm@")?;
    let version = version.split('+').next()?;
    validate::release_version(version).then(|| version.to_string())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const LIMIARA: &str = r#"schema = 1

[project]
name = "Limiara"                                        # shown in the UI
repository = "https://github.com/buchochelliq-labs/limiara-core"   # https git URL, used when pippwin clones it
branch = "main"                                         # default branch to check out

[app]
path = "apps/device-shell/ios-xtool"   # folder holding xtool.yml and Package.swift, relative to pippwin.toml

[build]
prepare = ["./prepare.sh"]             # optional argv run in app.path before `xtool dev`; no shell

[needs]
rust = true                            # install rustup; the project's rust-toolchain.toml picks the toolchain
rust_targets = ["aarch64-apple-ios"]
node = "22"                            # optional Node major; pippwin maps it to its own pinned, checksummed release
pnpm = "9.12.0"                        # optional; via corepack
apt = []                               # optional extra Ubuntu packages, each validated against a strict package-name pattern
"#;

    fn with(edit: impl Fn(&str) -> String) -> std::result::Result<ProjectFile, String> {
        ProjectFile::parse(&edit(LIMIARA))
    }

    /// A repository on disk with an xtool project at `app`.
    pub(crate) fn make_repo(dir: &Path, toml: &str) -> PathBuf {
        let repo = dir.join("repo");
        let app = repo.join("apps/device-shell/ios-xtool");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::write(
            app.join("xtool.yml"),
            "version: 1\n# comment\nbundleID: app.example.shell.xtool\nproduct: Shell\n",
        )
        .unwrap();
        std::fs::write(app.join("Package.swift"), "// swift-tools-version: 6.0\n").unwrap();
        std::fs::write(app.join("prepare.sh"), "#!/bin/sh\n").unwrap();
        std::fs::write(repo.join("pippwin.toml"), toml).unwrap();
        repo
    }

    #[test]
    fn the_schema_example_parses() {
        let file = ProjectFile::parse(LIMIARA).unwrap();
        assert_eq!(file.project.name, "Limiara");
        assert_eq!(file.project.branch, "main");
        assert_eq!(file.app.path, "apps/device-shell/ios-xtool");
        assert_eq!(file.build.prepare, ["./prepare.sh"]);
        assert!(file.needs.rust);
        assert_eq!(file.needs.rust_targets, ["aarch64-apple-ios"]);
        assert_eq!(file.node_release().unwrap().version, "22.23.5");
        assert_eq!(file.needs.pnpm.as_deref(), Some("9.12.0"));
        assert!(file.needs.apt.is_empty());
    }

    #[test]
    fn optional_sections_may_be_left_out() {
        let file = ProjectFile::parse(
            "schema = 1\n[project]\nname = \"App\"\nrepository = \"https://example.org/a/app\"\n[app]\npath = \".\"\n",
        )
        .unwrap();
        assert_eq!(file.project.branch, "main");
        assert!(file.build.prepare.is_empty());
        assert_eq!(file.needs, Needs::default());
    }

    #[test]
    fn unknown_keys_are_refused() {
        let top = with(|s| format!("{s}\nextra = 1\n")).unwrap_err();
        assert!(top.contains("unknown field `extra`"), "{top}");
        let nested = with(|s| s.replace("rust = true", "rust = true\nswift = \"6\"")).unwrap_err();
        assert!(nested.contains("unknown field `swift`"), "{nested}");
        assert!(nested.starts_with("line "), "{nested}");
        let table = with(|s| format!("{s}\n[deploy]\nx = 1\n")).unwrap_err();
        assert!(table.contains("deploy"), "{table}");
    }

    #[test]
    fn schema_and_required_fields() {
        assert!(with(|s| s.replace("schema = 1", "schema = 2"))
            .unwrap_err()
            .contains("schema = 2"));
        assert!(with(|s| s.replace("schema = 1", "")).is_err());
        assert!(with(|s| s.replace("name = \"Limiara\"", "name = \"\"")).is_err());
        assert!(with(|s| s.replace("name = \"Limiara\"", "name = \"a\\nb\"")).is_err());
    }

    #[test]
    fn repository_and_branch_are_checked() {
        for bad in [
            "http://github.com/a/b",
            "git@github.com:a/b.git",
            "https://user:ghp_token@github.com/a/b",
            "--upload-pack=touch /tmp/x",
        ] {
            let err = with(|s| s.replace("https://github.com/buchochelliq-labs/limiara-core", bad))
                .unwrap_err();
            assert!(err.starts_with("project.repository"), "{bad}: {err}");
            assert!(!err.contains("ghp_token"));
        }
        assert!(with(|s| s.replace("branch = \"main\"", "branch = \"-x\""))
            .unwrap_err()
            .contains("project.branch"));
    }

    #[test]
    fn app_path_must_stay_in_the_repository() {
        for bad in [
            "/etc",
            "../elsewhere",
            "apps/../../x",
            "~/x",
            "apps\\\\x",
            "",
        ] {
            let err = with(|s| s.replace("apps/device-shell/ios-xtool\"", &format!("{bad}\"")))
                .unwrap_err();
            assert!(err.starts_with("app.path"), "{bad}: {err}");
        }
    }

    #[test]
    fn app_path_escaping_through_a_symlink_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("xtool.yml"), "bundleID: x\n").unwrap();
        std::fs::write(outside.join("Package.swift"), "").unwrap();
        let repo = make_repo(
            dir.path(),
            &LIMIARA.replace("apps/device-shell/ios-xtool\"", "linked\""),
        );
        std::os::unix::fs::symlink(&outside, repo.join("linked")).unwrap();
        let err = Project::load(&repo).unwrap_err();
        assert_eq!(err.code(), "PW070_PROJECT_FILE");
        assert!(err.to_string().contains("outside the repository"), "{err}");

        // A symlink that stays inside is fine.
        let repo2 = dir.path().join("repo");
        std::os::unix::fs::symlink(
            repo2.join("apps/device-shell/ios-xtool"),
            repo2.join("inside"),
        )
        .unwrap();
        std::fs::write(
            repo2.join("pippwin.toml"),
            LIMIARA.replace("apps/device-shell/ios-xtool\"", "inside\""),
        )
        .unwrap();
        assert!(Project::load(&repo2).is_ok());
    }

    #[test]
    fn prepare_must_be_one_program_without_a_shell() {
        let set = |prepare: &str| with(|s| s.replace("[\"./prepare.sh\"]", prepare));
        for bad in [
            r#"["sh", "-c", "./prepare.sh"]"#,
            r#"["bash", "prepare.sh"]"#,
            r#"["/bin/sh", "-c", "x"]"#,
            r#"["env", "FOO=1", "./prepare.sh"]"#,
            r#"["sudo", "./prepare.sh"]"#,
            r#"["./prepare.sh && xtool dev"]"#,
            r#"["./prepare.sh; rm -rf ~"]"#,
            r#"["$(curl x)"]"#,
            r#"["../outside.sh"]"#,
            r#"["/usr/bin/make"]"#,
            r#"["-rf"]"#,
            r#"[""]"#,
        ] {
            let err = set(bad).unwrap_err();
            assert!(err.starts_with("build.prepare"), "{bad}: {err}");
        }
        let string = set(r#""./prepare.sh && xtool dev""#).unwrap_err();
        assert!(string.contains("not a shell command line"), "{string}");
        for good in [
            r#"["./prepare.sh"]"#,
            r#"["scripts/build.sh", "--release", "a b; c"]"#,
            r#"["make", "ios"]"#,
            "[]",
        ] {
            assert!(set(good).is_ok(), "{good}");
        }
    }

    #[test]
    fn prepare_script_must_be_inside_the_app_folder_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let repo = make_repo(
            dir.path(),
            &LIMIARA.replace("[\"./prepare.sh\"]", "[\"./missing.sh\"]"),
        );
        assert!(Project::load(&repo)
            .unwrap_err()
            .to_string()
            .contains("does not exist"));
        let app = repo.join("apps/device-shell/ios-xtool");
        std::fs::write(repo.join("evil.sh"), "").unwrap();
        std::os::unix::fs::symlink(repo.join("evil.sh"), app.join("linked.sh")).unwrap();
        std::fs::write(
            repo.join("pippwin.toml"),
            LIMIARA.replace("[\"./prepare.sh\"]", "[\"./linked.sh\"]"),
        )
        .unwrap();
        assert!(Project::load(&repo)
            .unwrap_err()
            .to_string()
            .contains("inside app.path"));
    }

    #[test]
    fn apt_names_are_strict() {
        for bad in ["\"-y\"", "\"Foo\"", "\"a b\"", "\"curl;id\"", "\"pkg=1\""] {
            let err = with(|s| s.replace("apt = []", &format!("apt = [{bad}]"))).unwrap_err();
            assert!(err.starts_with("needs.apt"), "{bad}: {err}");
        }
        assert!(with(|s| s.replace("apt = []", "apt = [\"libssl-dev\", \"g++\"]")).is_ok());
    }

    #[test]
    fn node_majors_are_mapped_or_refused() {
        let err = with(|s| s.replace("node = \"22\"", "node = \"18\"")).unwrap_err();
        assert!(err.contains("pippwin supports Node.js majors: 22"), "{err}");
        assert!(with(|s| s.replace("node = \"22\"", "node = \"lts\"")).is_err());
        assert!(with(|s| s.replace("node = \"22\"", "node = 22"))
            .unwrap_err()
            .contains("is a string"));
        assert!(with(|s| s.replace("pnpm = \"9.12.0\"", "pnpm = \"latest\"")).is_err());
        let no_node = with(|s| s.replace("node = \"22\"", "")).unwrap_err();
        assert!(no_node.contains("needs.pnpm needs needs.node"), "{no_node}");
    }

    #[test]
    fn rust_targets_are_checked() {
        assert!(with(|s| s.replace(
            "rust_targets = [\"aarch64-apple-ios\"]",
            "rust_targets = [\"--x\"]"
        ))
        .is_err());
        assert!(with(|s| s.replace("rust = true", "rust = false"))
            .unwrap_err()
            .contains("needs.rust = true"));
    }

    #[test]
    fn a_project_loads_with_its_bundle_id_and_build_commands() {
        let dir = tempfile::tempdir().unwrap();
        let repo = make_repo(dir.path(), LIMIARA);
        let project = Project::load(&repo).unwrap();
        assert_eq!(project.name(), "Limiara");
        assert_eq!(
            project.bundle_id.as_deref(),
            Some("app.example.shell.xtool")
        );
        let app = std::fs::canonicalize(repo.join("apps/device-shell/ios-xtool")).unwrap();
        assert_eq!(project.app_dir, app);
        let cmds = project.build_cmds();
        assert_eq!(cmds.len(), 2);
        for cmd in &cmds {
            assert_eq!(cmd.cwd.as_deref(), Some(app.as_path()));
            assert!(cmd.private_umask);
        }
        assert_eq!(
            cmds[0].argv(),
            [
                "sh",
                "-c",
                "umask 077 && exec \"$0\" \"$@\"",
                "./prepare.sh"
            ]
        );
        assert_eq!(
            cmds[1].argv(),
            [
                "sh",
                "-c",
                "umask 077 && exec \"$0\" \"$@\"",
                "xtool",
                "dev"
            ]
        );
    }

    #[test]
    fn a_missing_xtool_project_is_named() {
        let dir = tempfile::tempdir().unwrap();
        let repo = make_repo(dir.path(), LIMIARA);
        std::fs::remove_file(repo.join("apps/device-shell/ios-xtool/Package.swift")).unwrap();
        assert!(Project::load(&repo)
            .unwrap_err()
            .to_string()
            .contains("no Package.swift"));
    }

    #[test]
    fn roots_are_found_upwards_within_the_work_tree() {
        let dir = tempfile::tempdir().unwrap();
        let repo = make_repo(dir.path(), LIMIARA);
        let deep = repo.join("apps/device-shell/ios-xtool");
        assert_eq!(find_root(&deep), Some(repo.clone()));
        std::fs::remove_file(repo.join("pippwin.toml")).unwrap();
        // Stops at the work tree: a pippwin.toml above it is not used.
        std::fs::write(dir.path().join("pippwin.toml"), LIMIARA).unwrap();
        assert_eq!(find_root(&deep), None);
    }

    #[test]
    fn bundle_ids_from_xtool_yml() {
        assert_eq!(
            xtool_bundle_id("version: 1\nbundleID: app.limiara.shell.xtool\n").as_deref(),
            Some("app.limiara.shell.xtool")
        );
        assert_eq!(
            xtool_bundle_id("bundleID: \"com.example.app\" # note\n").as_deref(),
            Some("com.example.app")
        );
        assert_eq!(xtool_bundle_id("  bundleID: nested\n"), None);
        assert_eq!(xtool_bundle_id("bundleID: $(evil)\n"), None);
    }

    fn answers() -> InitAnswers {
        InitAnswers {
            name: "My \"App\"".into(),
            repository: "https://github.com/example/my-app".into(),
            branch: "main".into(),
            app_path: "ios".into(),
            prepare: vec!["./prepare.sh".into(), "--release".into()],
            rust: true,
            rust_targets: vec!["aarch64-apple-ios".into()],
            node: Some("22".into()),
            pnpm: Some("9.12.0".into()),
        }
    }

    #[test]
    fn init_output_round_trips() {
        let a = answers();
        let text = init_text(&a);
        let file = ProjectFile::parse(&text).unwrap();
        assert_eq!(file.schema, SCHEMA);
        assert_eq!(file.project.name, a.name);
        assert_eq!(file.project.repository, a.repository);
        assert_eq!(file.project.branch, a.branch);
        assert_eq!(file.app.path, a.app_path);
        assert_eq!(file.build.prepare, a.prepare);
        assert_eq!(file.needs.rust, a.rust);
        assert_eq!(file.needs.rust_targets, a.rust_targets);
        assert_eq!(file.needs.node, a.node);
        assert_eq!(file.needs.pnpm, a.pnpm);
        assert!(text.lines().any(|l| l.starts_with('#')));

        // The minimal form, with everything optional commented out.
        let minimal = InitAnswers {
            prepare: vec![],
            rust: false,
            rust_targets: vec![],
            node: None,
            pnpm: None,
            ..a
        };
        let file = ProjectFile::parse(&init_text(&minimal)).unwrap();
        assert!(file.build.prepare.is_empty());
        assert_eq!(file.needs, Needs::default());
    }

    #[test]
    fn init_finds_xtool_folders_and_pnpm() {
        let dir = tempfile::tempdir().unwrap();
        let repo = make_repo(dir.path(), LIMIARA);
        std::fs::create_dir_all(repo.join("node_modules/x")).unwrap();
        std::fs::write(repo.join("node_modules/x/xtool.yml"), "").unwrap();
        assert_eq!(xtool_folders(&repo), ["apps/device-shell/ios-xtool"]);
        assert_eq!(
            pnpm_from_package_json(r#"{"packageManager": "pnpm@9.12.0+sha512.abc"}"#).as_deref(),
            Some("9.12.0")
        );
        assert_eq!(
            pnpm_from_package_json(r#"{"packageManager": "yarn@4.0.0"}"#),
            None
        );
    }

    /// Limiara's real pippwin.toml (in a limiara-core checkout), when this
    /// machine has one: set PIPPWIN_LIMIARA_TOML to its path, or keep the
    /// checkout at /home/user/limiara-core-xtool.
    #[test]
    fn limiaras_real_file_parses() {
        let path = std::env::var_os("PIPPWIN_LIMIARA_TOML")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/home/user/limiara-core-xtool/pippwin.toml"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!(
                "note: {} is not here; skipping the check of Limiara's real pippwin.toml",
                path.display()
            );
            return;
        };
        let file = ProjectFile::parse(&text).unwrap();
        assert_eq!(file.project.name, "Limiara");
        assert!(validate::same_repository(
            &file.project.repository,
            "https://github.com/buchochelliq-labs/limiara-core"
        ));
        // With the checkout next to it, the folders it names check out too.
        let root = path.parent().unwrap();
        if root.join(&file.app.path).is_dir() {
            let project = Project::load(root).unwrap();
            assert!(project.bundle_id.is_some());
        }
    }
}
