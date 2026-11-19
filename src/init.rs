//! `pippwin init` (write a commented `pippwin.toml` into the current
//! repository) and `pippwin check` (check one).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::cmd::{Cmd, Runner};
use crate::error::{Error, Result};
use crate::paths;
use crate::project::{self, InitAnswers, Project, ProjectFile};
use crate::ui::Ui;
use crate::validate;
use crate::versions;

/// Why `typed` is not the xtool project's folder inside `root`; else the
/// folder relative to `root`.
pub fn app_folder(root: &Path, typed: &str) -> std::result::Result<String, String> {
    let typed = typed.trim().trim_end_matches('/');
    let typed = if typed.is_empty() { "." } else { typed };
    let candidate = Path::new(typed);
    let relative = if candidate.is_absolute() {
        let resolved =
            std::fs::canonicalize(candidate).map_err(|_| format!("There is no folder {typed}."))?;
        let root = std::fs::canonicalize(root).map_err(|_| "The repository cannot be read.")?;
        resolved
            .strip_prefix(&root)
            .map_err(|_| "The folder must be inside this repository.".to_string())?
            .to_string_lossy()
            .into_owned()
    } else {
        typed.to_string()
    };
    let relative = if relative.is_empty() {
        ".".into()
    } else {
        relative
    };
    let dir = root.join(&relative);
    if !dir.is_dir() {
        return Err(format!("There is no folder {relative} in the repository."));
    }
    for needed in ["xtool.yml", "Package.swift"] {
        if !dir.join(needed).is_file() {
            return Err(format!(
                "{relative} has no {needed}; choose the folder of the xtool project."
            ));
        }
    }
    Ok(relative)
}

fn capture(run: &mut dyn Runner, root: &Path, args: &[&str]) -> Option<String> {
    let out = run
        .capture(
            &Cmd::new("git")
                .arg("-C")
                .path_arg(root)
                .args(args.iter().copied()),
        )
        .ok()?;
    out.success
        .then(|| out.stdout.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// What `init` can tell about the repository by itself.
pub fn detect(run: &mut dyn Runner, root: &Path, app_path: &str) -> InitAnswers {
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "My app".into());
    let repository = capture(run, root, &["remote", "get-url", "origin"])
        .and_then(|r| validate::remote_as_https(&r))
        .map(|url| url.strip_suffix(".git").map(str::to_string).unwrap_or(url))
        .unwrap_or_default();
    let branch = capture(
        run,
        root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .and_then(|b| b.strip_prefix("origin/").map(str::to_string))
    .or_else(|| capture(run, root, &["branch", "--show-current"]))
    .filter(|b| validate::branch(b).is_ok())
    .unwrap_or_else(|| "main".into());
    let app = root.join(app_path);
    let prepare = if app.join("prepare.sh").is_file() {
        vec!["./prepare.sh".to_string()]
    } else {
        Vec::new()
    };
    let rust = ["rust-toolchain.toml", "rust-toolchain", "Cargo.toml"]
        .iter()
        .any(|f| root.join(f).is_file());
    let package_json = std::fs::read_to_string(root.join("package.json")).ok();
    let node = package_json.as_ref().and_then(|_| {
        versions::NODE_RELEASES
            .iter()
            .map(|r| r.major)
            .max()
            .map(|m| m.to_string())
    });
    let pnpm = package_json
        .as_deref()
        .and_then(project::pnpm_from_package_json);
    InitAnswers {
        name,
        repository,
        branch,
        app_path: app_path.to_string(),
        prepare,
        rust,
        rust_targets: if rust {
            vec!["aarch64-apple-ios".into()]
        } else {
            Vec::new()
        },
        node,
        pnpm,
    }
}

fn choose_app(ui: &mut dyn Ui, root: &Path) -> Result<String> {
    let found = project::xtool_folders(root);
    let mut options: Vec<String> = found.clone();
    options.push("Type the folder…".into());
    let question = if found.is_empty() {
        "No xtool.yml found. Which folder holds the xtool project?"
    } else {
        "Which folder holds the xtool project (xtool.yml and Package.swift)?"
    };
    let choice = ui.choose(question, &options, 0)?;
    let root_owned = root.to_path_buf();
    if choice < found.len() {
        return app_folder(root, &found[choice]).map_err(Error::InvalidInput);
    }
    ui.ask(
        "App folder, relative to the repository's root",
        None,
        Rc::new(move |typed: &str| app_folder(&root_owned, typed)),
    )
}

pub fn run(ui: &mut dyn Ui, runner: &mut dyn Runner, cwd: &Path, app: Option<&str>) -> Result<()> {
    let Some(root) = paths::git_work_tree(cwd) else {
        return Err(Error::NotReady(
            "run `pippwin init` inside the project's git repository".into(),
        ));
    };
    let target = Project::file_path(&root);
    if std::fs::symlink_metadata(&target).is_ok() {
        return Err(Error::NotReady(format!(
            "{} already exists; `pippwin init` never overwrites it (`pippwin check` checks it)",
            target.display()
        )));
    }
    ui.heading("A pippwin.toml for this repository");
    let app_path = match app {
        Some(typed) => app_folder(&root, typed).map_err(Error::InvalidInput)?,
        None => choose_app(ui, &root)?,
    };
    let mut answers = detect(runner, &root, &app_path);
    if answers.repository.is_empty() {
        answers.repository = ui.ask(
            "The repository's https git URL",
            None,
            Rc::new(validate::https_git_url),
        )?;
    }
    let text = project::init_text(&answers);
    // What init writes must be what pippwin reads.
    ProjectFile::parse(&text).map_err(|reason| Error::Project {
        path: target.clone(),
        reason,
    })?;
    write_new(&target, &text)?;
    ui.done(&format!(
        "Wrote {}.",
        rich::markup::escape(&target.to_string_lossy())
    ));
    ui.say_verbatim(&text);
    match Project::load(&root) {
        Ok(_) => ui.say(
            "Review it (the name, the branch, build.prepare and needs were guessed), commit it, and \
anyone can then run [bold]pippwin setup --project <its git URL>[/].",
        ),
        Err(error) => ui.warn(
            "Check the new file",
            &rich::markup::escape(&error.to_string()),
        ),
    }
    Ok(())
}

/// Create `path` with `text`, failing if anything is already there.
fn write_new(path: &Path, text: &str) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| Error::file(path, &e))?;
    file.write_all(text.as_bytes())
        .map_err(|e| Error::file(path, &e))
}

/// `pippwin check`: load the project and say what it would do.
pub fn check(ui: &mut dyn Ui, cwd: &Path, explicit: Option<&Path>) -> Result<()> {
    let root: PathBuf = match explicit {
        Some(dir) => dir.to_path_buf(),
        None => project::find_root(cwd).ok_or_else(|| {
            Error::NotReady(format!(
                "no {} here or above (up to the git root); `pippwin init` writes one",
                versions::PROJECT_FILE
            ))
        })?,
    };
    let project = Project::load(&root)?;
    let e = |s: &str| rich::markup::escape(s);
    ui.done(&format!(
        "{} is valid: {} ({}), app folder {}, bundle ID {}.",
        e(&Project::file_path(&project.root).to_string_lossy()),
        e(project.name()),
        e(&project.file.project.repository),
        e(&project.file.app.path),
        e(project
            .bundle_id
            .as_deref()
            .unwrap_or("(none in xtool.yml)"))
    ));
    for cmd in project.build_cmds() {
        ui.show_command(&cmd);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::fake::FakeRunner;
    use crate::project::tests::{make_repo, LIMIARA};
    use crate::ui::testing::{Answer, ScriptUi};

    #[test]
    fn init_writes_a_file_that_loads_and_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let repo = make_repo(dir.path(), LIMIARA);
        std::fs::remove_file(repo.join("pippwin.toml")).unwrap();
        std::fs::write(repo.join("rust-toolchain.toml"), "[toolchain]\n").unwrap();
        std::fs::write(
            repo.join("package.json"),
            r#"{"packageManager": "pnpm@9.12.0"}"#,
        )
        .unwrap();
        let mut ui = ScriptUi::new([Answer::Keys("enter")]);
        let mut run = FakeRunner::default()
            .answer(
                &format!("git -C {} remote get-url origin", repo.display()),
                "git@github.com:example-org/my-app.git\n",
            )
            .answer(
                &format!(
                    "git -C {} symbolic-ref --short refs/remotes/origin/HEAD",
                    repo.display()
                ),
                "origin/trunk\n",
            );
        run_init(&mut ui, &mut run, &repo.join("apps"));
        let project = Project::load(&repo).unwrap();
        let file = &project.file;
        assert_eq!(file.project.name, "repo");
        assert_eq!(
            file.project.repository,
            "https://github.com/example-org/my-app"
        );
        assert_eq!(file.project.branch, "trunk");
        assert_eq!(file.app.path, "apps/device-shell/ios-xtool");
        assert_eq!(file.build.prepare, ["./prepare.sh"]);
        assert!(file.needs.rust);
        assert_eq!(file.needs.node.as_deref(), Some("22"));
        assert_eq!(file.needs.pnpm.as_deref(), Some("9.12.0"));
        assert_eq!(
            ui.questions,
            ["Which folder holds the xtool project (xtool.yml and Package.swift)?"]
        );

        let before = std::fs::read_to_string(repo.join("pippwin.toml")).unwrap();
        let mut ui = ScriptUi::new([]);
        let err = super::run(
            &mut ui,
            &mut run,
            &repo,
            Some("apps/device-shell/ios-xtool"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("never overwrites"));
        assert_eq!(
            std::fs::read_to_string(repo.join("pippwin.toml")).unwrap(),
            before
        );
    }

    fn run_init(ui: &mut ScriptUi, run: &mut FakeRunner, cwd: &Path) {
        super::run(ui, run, cwd, None).unwrap();
    }

    #[test]
    fn init_asks_for_the_url_when_origin_is_not_https() {
        let dir = tempfile::tempdir().unwrap();
        let repo = make_repo(dir.path(), LIMIARA);
        std::fs::remove_file(repo.join("pippwin.toml")).unwrap();
        let mut ui = ScriptUi::new([Answer::Text("https://example.org/team/app".into())]);
        let mut run = FakeRunner::default();
        super::run(
            &mut ui,
            &mut run,
            &repo,
            Some("apps/device-shell/ios-xtool"),
        )
        .unwrap();
        let file = Project::load(&repo).unwrap().file;
        assert_eq!(file.project.repository, "https://example.org/team/app");
        assert_eq!(file.project.branch, "main");
    }

    #[test]
    fn app_folders_must_be_xtool_projects_in_the_repository() {
        let dir = tempfile::tempdir().unwrap();
        let repo = make_repo(dir.path(), LIMIARA);
        assert_eq!(
            app_folder(&repo, "apps/device-shell/ios-xtool/").unwrap(),
            "apps/device-shell/ios-xtool"
        );
        let absolute = repo.join("apps/device-shell/ios-xtool");
        assert_eq!(
            app_folder(&repo, &absolute.to_string_lossy()).unwrap(),
            "apps/device-shell/ios-xtool"
        );
        assert!(app_folder(&repo, "apps").unwrap_err().contains("xtool.yml"));
        assert!(app_folder(&repo, "missing").is_err());
        assert!(app_folder(&repo, &dir.path().to_string_lossy()).is_err());
    }

    #[test]
    fn init_outside_a_repository_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut ui = ScriptUi::new([]);
        let mut run = FakeRunner::default();
        assert!(super::run(&mut ui, &mut run, dir.path(), None)
            .unwrap_err()
            .to_string()
            .contains("git repository"));
    }
}
