//! Step 3: the project. Cloned from its git URL (or a clone reused, after
//! checking its origin and that it is clean), or a folder already in WSL;
//! then its `pippwin.toml` is read and checked.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::{Ctx, Status, Step};
use crate::cmd::{Cmd, Input};
use crate::error::{Error, Result};
use crate::paths;
use crate::project::{self, Project, ProjectFile};
use crate::state::ProjectRecord;
use crate::validate;
use crate::versions;

pub const PROJECT: Step = Step {
    id: "project",
    title: "The project",
    why: "pippwin builds an app from its source: the project's repository is cloned into your Linux home \
(not under /mnt, where Windows' file system makes builds slow and breaks file permissions), or a clone \
you already have is used. Its pippwin.toml says which folder holds the xtool project, what to run before \
xtool builds it, and which tools it needs.",
    check: check_project,
    run: run_project,
    when_done: None,
};

/// Where `--project` points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Url(String),
    Folder(PathBuf),
}

fn looks_like_url(value: &str) -> bool {
    value.contains("://") || value.starts_with("git@")
}

pub fn parse_source(value: &str, home: &Path) -> Result<Source> {
    if looks_like_url(value) {
        validate::https_git_url(value)
            .map(Source::Url)
            .map_err(Error::InvalidInput)
    } else {
        paths::normalize_user_path(value, home, None)
            .filter(|p| p.is_absolute())
            .map(Source::Folder)
            .ok_or_else(|| {
                Error::InvalidInput(
                    "--project is a git URL (https://…) or a full path to a folder".into(),
                )
            })
    }
}

fn current_branch(cx: &mut Ctx, repo: &Path) -> Option<String> {
    let out = cx
        .run
        .capture(&git(repo).args(["branch", "--show-current"]))
        .ok()?;
    out.success
        .then(|| out.stdout.trim().to_string())
        .filter(|b| !b.is_empty())
}

fn git(repo: &Path) -> Cmd {
    Cmd::new("git").arg("-C").path_arg(repo)
}

fn describe(cx: &mut Ctx, project: &Project) -> String {
    let branch = current_branch(cx, &project.root)
        .map(|b| format!(" ({b})"))
        .unwrap_or_default();
    format!("{} in {}{branch}", project.name(), project.root.display())
}

fn check_project(cx: &mut Ctx) -> Status {
    let request = cx.request.clone();
    let Some(project) = cx.project.clone() else {
        if let (Some(problem), None) = (&cx.project_problem, &request.project) {
            return Status::Problem(problem.clone());
        }
        return Status::Missing("no project chosen yet".into());
    };
    let matches = match request
        .project
        .as_deref()
        .map(|v| parse_source(v, &cx.paths.home))
    {
        None => true,
        Some(Err(_)) => false,
        Some(Ok(Source::Url(url))) => {
            cx.state
                .record(&project.root)
                .is_some_and(|r| validate::same_repository(&r.repository, &url))
                || validate::same_repository(&project.file.project.repository, &url)
        }
        Some(Ok(Source::Folder(folder))) => {
            std::fs::canonicalize(&folder).is_ok_and(|f| f == project.root)
        }
    };
    let branch_ok = match &request.branch {
        None => true,
        Some(branch) => current_branch(cx, &project.root).as_deref() == Some(branch.as_str()),
    };
    if matches && branch_ok {
        Status::Done(describe(cx, &project))
    } else {
        Status::Missing(format!(
            "{} is set up; {} is asked for",
            project.name(),
            request.project.as_deref().unwrap_or("another branch")
        ))
    }
}

/// Which branch to check out.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Branch {
    /// The one `project.branch` names.
    ProjectDefault,
    Named(String),
}

fn run_project(cx: &mut Ctx) -> Result<()> {
    let source = match cx.request.project.clone() {
        Some(value) => parse_source(&value, &cx.paths.home)?,
        None => ask_source(cx)?,
    };
    let root = match source {
        Source::Url(url) => {
            let dest = ask_clone_location(cx, &url)?;
            let branch = match cx.request.branch.clone() {
                Some(name) => Branch::Named(name),
                None => ask_branch(cx)?,
            };
            if dest.join(".git").exists() {
                reuse_clone(cx, &dest, &url, &branch)?;
            } else {
                clone(cx, &url, &dest, &branch)?;
            }
            dest
        }
        Source::Folder(folder) => use_folder(cx, &folder)?,
    };
    let project = Project::load(&root)?;
    show_project(cx, &project);
    let branch = current_branch(cx, &project.root);
    let repository =
        git_origin(cx, &project.root).unwrap_or_else(|| project.file.project.repository.clone());
    cx.state.remember(ProjectRecord {
        name: project.name().to_string(),
        root: project.root.clone(),
        repository,
        branch,
        installed: false,
    });
    cx.project = Some(project);
    cx.project_problem = None;
    cx.save_state()
}

fn git_origin(cx: &mut Ctx, repo: &Path) -> Option<String> {
    let out = cx
        .run
        .capture(&git(repo).args(["remote", "get-url", "origin"]))
        .ok()?;
    if !out.success {
        return None;
    }
    validate::remote_as_https(out.stdout.trim())
}

fn ask_source(cx: &mut Ctx) -> Result<Source> {
    let mut known: Vec<(String, PathBuf)> = cx
        .state
        .projects
        .iter()
        .map(|p| (format!("{} ({})", p.name, p.root.display()), p.root.clone()))
        .collect();
    if let Some(here) = project::find_root(&cx.cwd) {
        if !known.iter().any(|(_, root)| *root == here) {
            known.insert(0, (format!("The project here ({})", here.display()), here));
        }
    }
    let mut options: Vec<String> = known.iter().map(|(label, _)| label.clone()).collect();
    options.push("Clone a project from its git URL…".into());
    options.push("A folder already in WSL…".into());
    let choice = cx.ui.choose("Which project?", &options, 0)?;
    if choice < known.len() {
        return Ok(Source::Folder(known[choice].1.clone()));
    }
    if choice == known.len() {
        let url = cx.ui.ask(
            "Git URL of the project (https://…)",
            None,
            Rc::new(validate::https_git_url),
        )?;
        return Ok(Source::Url(url));
    }
    let home = cx.paths.home.clone();
    let folder = cx.ui.ask(
        "Folder holding the project's pippwin.toml",
        None,
        Rc::new(move |typed: &str| {
            let path = paths::normalize_user_path(typed, &home, None).ok_or("Enter a folder.")?;
            if !Project::file_path(&path).is_file() {
                return Err(format!(
                    "There is no {} in {}.",
                    versions::PROJECT_FILE,
                    path.display()
                ));
            }
            Ok(path.to_string_lossy().into_owned())
        }),
    )?;
    Ok(Source::Folder(PathBuf::from(folder)))
}

/// Why a clone may not go to `path`, if it may not.
pub fn check_clone_location(path: &Path) -> std::result::Result<(), String> {
    if !path.is_absolute() {
        return Err("Enter a full path, like ~/my-app.".into());
    }
    let resolved = paths::resolve_existing(path)?;
    if paths::is_windows_mount(path) || paths::is_windows_mount(&resolved) {
        return Err(
            "Use a folder in your Linux home, not under /mnt (a Windows drive): builds there are slow \
and file permissions do not work."
                .into(),
        );
    }
    Ok(())
}

fn ask_clone_location(cx: &mut Ctx, url: &str) -> Result<PathBuf> {
    let name = validate::repository_name(url).unwrap_or_else(|| "project".into());
    let default = cx.paths.home.join(name);
    let home = cx.paths.home.clone();
    let validate: crate::ui::Validator = Rc::new(move |typed: &str| {
        let path = paths::normalize_user_path(typed, &home, None).ok_or("Enter a folder.")?;
        check_clone_location(&path)?;
        Ok(path.to_string_lossy().into_owned())
    });
    let dest = cx.ui.ask(
        "Where should the repository be?",
        Some(&default.to_string_lossy()),
        validate,
    )?;
    Ok(PathBuf::from(dest))
}

fn ask_branch(cx: &mut Ctx) -> Result<Branch> {
    let options = vec![
        "The branch its pippwin.toml names (usually main)".to_string(),
        "Another branch…".to_string(),
    ];
    match cx.ui.choose("Which branch?", &options, 0)? {
        0 => Ok(Branch::ProjectDefault),
        _ => Ok(Branch::Named(cx.ui.ask(
            "Branch name",
            None,
            Rc::new(validate::branch),
        )?)),
    }
}

pub fn clone_cmd(url: &str, dest: &Path, branch: Option<&str>) -> Cmd {
    let mut cmd = Cmd::new("git").arg("clone");
    if let Some(branch) = branch {
        cmd = cmd.args(["--branch", branch]);
    }
    cmd.args(["--", url]).path_arg(dest)
}

fn clone(cx: &mut Ctx, url: &str, dest: &Path, branch: &Branch) -> Result<()> {
    if dest.exists()
        && std::fs::read_dir(dest)
            .map_err(|e| Error::file(dest, &e))?
            .next()
            .is_some()
    {
        return Err(Error::NotReady(format!(
            "{} exists and is not a git repository; choose another folder",
            dest.display()
        )));
    }
    let named = match branch {
        Branch::Named(name) => Some(name.as_str()),
        Branch::ProjectDefault => None,
    };
    cx.ui
        .note(&format!("Cloning {}.", rich::markup::escape(url)));
    cx.run.interactive(&clone_cmd(url, dest, named))?;
    if *branch == Branch::ProjectDefault {
        switch_to_project_branch(cx, dest)?;
    }
    Ok(())
}

/// The project file as checked out now, for its `project.branch`.
fn project_file_here(repo: &Path) -> Result<ProjectFile> {
    let path = Project::file_path(repo);
    let text = std::fs::read_to_string(&path).map_err(|_| {
        Error::NotReady(format!(
            "{} has no {} at its root; the project's maintainers can add one with `pippwin init`, \
or choose the branch that has it",
            repo.display(),
            versions::PROJECT_FILE
        ))
    })?;
    ProjectFile::parse(&text).map_err(|reason| Error::Project { path, reason })
}

fn switch_to_project_branch(cx: &mut Ctx, repo: &Path) -> Result<()> {
    let wanted = project_file_here(repo)?.project.branch;
    if current_branch(cx, repo).as_deref() != Some(wanted.as_str()) {
        switch(cx, repo, &wanted)?;
    }
    Ok(())
}

/// Fetch `branch` from origin, switch to it and bring it up to date. Only
/// a fast-forward: a local branch with commits of its own stops here, so
/// nothing of the user's is rewritten.
fn switch(cx: &mut Ctx, repo: &Path, branch: &str) -> Result<()> {
    cx.ui.note(&format!(
        "Switching to {} and bringing it up to date.",
        rich::markup::escape(branch)
    ));
    cx.run.interactive(
        &git(repo)
            .args(["fetch", "origin"])
            .arg(format!("+refs/heads/{branch}:refs/remotes/origin/{branch}"))
            .input(Input::Null),
    )?;
    cx.run
        .interactive(&git(repo).args(["switch", branch]).input(Input::Null))?;
    cx.run
        .interactive(
            &git(repo)
                .args(["merge", "--ff-only"])
                .arg(format!("origin/{branch}"))
                .input(Input::Null),
        )
        .map_err(|_| {
            Error::NotReady(format!(
                "{branch} in {} has commits that are not on origin, so it cannot be brought up to date; \
merge or remove them, then run setup again",
                repo.display()
            ))
        })
}

/// The branch origin's HEAD points at: the repository's default branch.
fn remote_default_branch(cx: &mut Ctx, repo: &Path) -> Result<String> {
    let out = cx
        .run
        .capture(&git(repo).args(["ls-remote", "--symref", "origin", "HEAD"]))?;
    let found = out
        .success
        .then(|| crate::parse::symref_head(&out.stdout))
        .flatten();
    let name = found.ok_or_else(|| {
        Error::NotReady(format!(
            "could not ask origin of {} for its default branch; check the network and run setup again",
            repo.display()
        ))
    })?;
    validate::branch(&name).map_err(|_| {
        Error::NotReady(format!(
            "origin of {} names an unusable default branch",
            repo.display()
        ))
    })
}

fn require_clean(cx: &mut Ctx, repo: &Path) -> Result<()> {
    let status = cx.run.capture(&git(repo).args(["status", "--porcelain"]))?;
    if !status.success {
        return Err(Error::NotReady(format!(
            "git cannot read {}; is it a git repository?",
            repo.display()
        )));
    }
    if !status.stdout.trim().is_empty() {
        return Err(Error::NotReady(format!(
            "{} has uncommitted changes; commit or stash them, then run setup again",
            repo.display()
        )));
    }
    Ok(())
}

fn reuse_clone(cx: &mut Ctx, repo: &Path, url: &str, branch: &Branch) -> Result<()> {
    let origin = cx
        .run
        .capture(&git(repo).args(["remote", "get-url", "origin"]))?;
    if !origin.success || !validate::same_repository(origin.stdout.trim(), url) {
        return Err(Error::NotReady(format!(
            "{} is a clone of another repository, not {url}; choose another folder",
            repo.display()
        )));
    }
    require_clean(cx, repo)?;
    // A clone made earlier can be on any branch and behind origin, so it
    // is always fetched and brought up to date, never used as it stands.
    match branch {
        Branch::Named(name) => switch(cx, repo, name)?,
        Branch::ProjectDefault => {
            // The branch checked out may predate pippwin.toml (an older
            // clone): then start from the repository's default branch and
            // read the file there.
            let on = if Project::file_path(repo).is_file() {
                None
            } else {
                let default = remote_default_branch(cx, repo)?;
                switch(cx, repo, &default)?;
                Some(default)
            };
            let wanted = project_file_here(repo)?.project.branch;
            if on.as_deref() != Some(wanted.as_str()) {
                switch(cx, repo, &wanted)?;
            }
        }
    }
    cx.ui.done(&format!(
        "Reusing the clone in {}.",
        rich::markup::escape(&repo.to_string_lossy())
    ));
    Ok(())
}

fn use_folder(cx: &mut Ctx, folder: &Path) -> Result<PathBuf> {
    check_clone_location(folder).map_err(Error::InvalidInput)?;
    let root = std::fs::canonicalize(folder).map_err(|e| Error::file(folder, &e))?;
    if !Project::file_path(&root).is_file() {
        return Err(Error::NotReady(format!(
            "{} has no {}; add one with `pippwin init`",
            root.display(),
            versions::PROJECT_FILE
        )));
    }
    if let Some(branch) = cx.request.branch.clone() {
        if current_branch(cx, &root).as_deref() != Some(branch.as_str()) {
            require_clean(cx, &root)?;
            switch(cx, &root, &branch)?;
        }
    }
    Ok(root)
}

fn show_project(cx: &mut Ctx, project: &Project) {
    let needs = &project.file.needs;
    let mut tools = Vec::new();
    if needs.rust {
        let targets = if needs.rust_targets.is_empty() {
            String::new()
        } else {
            format!(" ({})", needs.rust_targets.join(", "))
        };
        tools.push(format!("Rust{targets}"));
    }
    if let Some(release) = project.file.node_release() {
        tools.push(format!("Node.js {}", release.version));
    }
    if let Some(pnpm) = &needs.pnpm {
        tools.push(format!("pnpm {pnpm}"));
    }
    if !needs.apt.is_empty() {
        tools.push(format!("apt: {}", needs.apt.join(" ")));
    }
    let prepare = project
        .prepare_cmd()
        .map(|c| c.display())
        .unwrap_or_else(|| "nothing".into());
    let e = |s: &str| rich::markup::escape(s);
    cx.ui.explain(
        &format!("{} ({})", project.name(), versions::PROJECT_FILE),
        &format!(
            "[bold]Repository:[/] {}\n[bold]App folder:[/] {}\n[bold]Bundle ID:[/] {}\n\
[bold]Before xtool dev:[/] {}\n[bold]Also needs:[/] {}",
            e(&project.file.project.repository),
            e(&project.app_dir.to_string_lossy()),
            e(project.bundle_id.as_deref().unwrap_or("(not in xtool.yml)")),
            e(&prepare),
            e(&if tools.is_empty() {
                "nothing more".to_string()
            } else {
                tools.join(", ")
            }),
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::fake::FakeRunner;
    use crate::paths::Paths;
    use crate::project::tests::{make_repo, LIMIARA};
    use crate::state::State;
    use crate::steps::Host;
    use crate::ui::testing::{Answer, ScriptUi};

    const URL: &str = "https://github.com/buchochelliq-labs/limiara-core";

    fn ctx<'a>(ui: &'a mut ScriptUi, run: &'a mut FakeRunner, home: &Path) -> Ctx<'a> {
        Ctx::new(
            ui,
            run,
            Paths::from_home(home.to_path_buf()),
            State::default(),
            Host {
                wsl: true,
                os: None,
                arch: None,
            },
        )
    }

    #[test]
    fn a_url_is_cloned_into_home_and_its_project_loaded() {
        let home = tempfile::tempdir().unwrap();
        let dest = home.path().join("limiara-core");
        let mut ui = ScriptUi::new([
            Answer::Text(dest.to_string_lossy().into()),
            Answer::Keys("enter"),
        ]);
        let mut run = FakeRunner::default().answer(
            &format!("git -C {} branch --show-current", dest.display()),
            "main\n",
        );
        let parent = home.path().to_path_buf();
        run.on_interactive = Some(Box::new(move |cmd: &Cmd| {
            if cmd.args.first().map(String::as_str) == Some("clone") {
                let made = make_repo(&parent, LIMIARA);
                std::fs::rename(made, parent.join("limiara-core")).unwrap();
            }
        }));
        {
            let mut cx = ctx(&mut ui, &mut run, home.path());
            cx.request.project = Some(URL.into());
            run_project(&mut cx).unwrap();
            let project = cx.project.clone().unwrap();
            assert_eq!(project.root, std::fs::canonicalize(&dest).unwrap());
            assert_eq!(cx.state.current.as_deref(), Some(project.root.as_path()));
            assert_eq!(cx.state.projects[0].name, "Limiara");
            assert_eq!(cx.state.projects[0].branch.as_deref(), Some("main"));
            assert!(check_project(&mut cx).is_done());
            // The state file was written, 0600.
            use std::os::unix::fs::MetadataExt;
            let mode = std::fs::metadata(cx.paths.state_file()).unwrap().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert_eq!(
            ui.questions,
            ["Where should the repository be?", "Which branch?"]
        );
        // One clone, of exactly the URL, with `--` before it; already on
        // the branch pippwin.toml names, so no switch.
        assert_eq!(run.ran.len(), 1);
        assert_eq!(
            run.ran[0].argv(),
            ["git", "clone", "--", URL, &dest.to_string_lossy()]
        );
        assert!(ui.transcript().contains("app.example.shell.xtool"));
    }

    #[test]
    fn clones_never_go_under_mnt() {
        assert!(check_clone_location(Path::new("/mnt/c/Users/ada/app"))
            .unwrap_err()
            .contains("/mnt"));
        assert!(check_clone_location(Path::new("relative")).is_err());
        let dir = tempfile::tempdir().unwrap();
        assert!(check_clone_location(&dir.path().join("app")).is_ok());
        // Asked for /mnt, the question rejects it.
        let home = tempfile::tempdir().unwrap();
        let mut ui = ScriptUi::new([Answer::Text("/mnt/c/Users/ada/app".into())]);
        let mut run = FakeRunner::default();
        let mut cx = ctx(&mut ui, &mut run, home.path());
        cx.request.project = Some(URL.into());
        assert_eq!(
            run_project(&mut cx).unwrap_err().code(),
            "PW030_INVALID_INPUT"
        );
        assert!(run.ran.is_empty());
    }

    #[test]
    fn a_clone_of_another_repository_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let repo = make_repo(home.path(), LIMIARA);
        let mut ui = ScriptUi::new([
            Answer::Text(repo.to_string_lossy().into()),
            Answer::Keys("enter"),
        ]);
        let mut run = FakeRunner::default().answer(
            &format!("git -C {} remote get-url origin", repo.display()),
            "https://github.com/someone/else\n",
        );
        let mut cx = ctx(&mut ui, &mut run, home.path());
        cx.request.project = Some(URL.into());
        let err = run_project(&mut cx).unwrap_err();
        assert!(err.to_string().contains("another repository"), "{err}");
        assert!(run.ran.is_empty());
    }

    #[test]
    fn a_dirty_clone_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let repo = make_repo(home.path(), LIMIARA);
        let mut ui = ScriptUi::new([Answer::Text(repo.to_string_lossy().into())]);
        let mut run = FakeRunner::default()
            .answer(
                &format!("git -C {} remote get-url origin", repo.display()),
                "git@github.com:buchochelliq-labs/limiara-core.git\n",
            )
            .answer(
                &format!("git -C {} status --porcelain", repo.display()),
                " M prepare.sh\n",
            );
        let mut cx = ctx(&mut ui, &mut run, home.path());
        cx.request.project = Some(URL.into());
        cx.request.branch = Some("main".into());
        let err = run_project(&mut cx).unwrap_err();
        assert!(err.to_string().contains("uncommitted changes"), "{err}");
        assert!(run.ran.is_empty());
    }

    #[test]
    fn a_clean_clone_of_the_same_repository_is_reused_and_switched() {
        let home = tempfile::tempdir().unwrap();
        let repo = make_repo(home.path(), LIMIARA);
        let mut ui = ScriptUi::new([Answer::Text(repo.to_string_lossy().into())]);
        let mut run = FakeRunner::default()
            .answer(
                &format!("git -C {} remote get-url origin", repo.display()),
                "git@github.com:buchochelliq-labs/limiara-core.git\n",
            )
            .answer(&format!("git -C {} status --porcelain", repo.display()), "")
            .answer(
                &format!("git -C {} branch --show-current", repo.display()),
                "main\n",
            );
        let mut cx = ctx(&mut ui, &mut run, home.path());
        cx.request.project = Some(URL.into());
        cx.request.branch = Some("feature/ios".into());
        run_project(&mut cx).unwrap();
        let shown: Vec<String> = run.ran.iter().map(Cmd::display).collect();
        assert_eq!(shown.len(), 3);
        assert!(shown[0]
            .ends_with("fetch origin +refs/heads/feature/ios:refs/remotes/origin/feature/ios"));
        assert!(shown[1].ends_with("switch feature/ios"));
        assert!(shown[2].ends_with("merge --ff-only origin/feature/ios"));
    }

    #[test]
    fn a_reused_clone_on_the_wanted_branch_is_still_brought_up_to_date() {
        let home = tempfile::tempdir().unwrap();
        let repo = make_repo(home.path(), LIMIARA);
        let mut ui = ScriptUi::new([
            Answer::Text(repo.to_string_lossy().into()),
            Answer::Keys("enter"),
        ]);
        let mut run = FakeRunner::default()
            .answer(
                &format!("git -C {} remote get-url origin", repo.display()),
                "https://github.com/buchochelliq-labs/limiara-core\n",
            )
            .answer(&format!("git -C {} status --porcelain", repo.display()), "")
            .answer(
                &format!("git -C {} branch --show-current", repo.display()),
                "main\n",
            );
        let mut cx = ctx(&mut ui, &mut run, home.path());
        cx.request.project = Some(URL.into());
        run_project(&mut cx).unwrap();
        let shown: Vec<String> = run.ran.iter().map(Cmd::display).collect();
        assert_eq!(
            shown,
            [
                format!(
                    "git -C {} fetch origin +refs/heads/main:refs/remotes/origin/main",
                    repo.display()
                ),
                format!("git -C {} switch main", repo.display()),
                format!("git -C {} merge --ff-only origin/main", repo.display()),
            ]
        );
    }

    #[test]
    fn an_older_clone_without_pippwin_toml_moves_to_the_default_branch() {
        // A clone made before the project had pippwin.toml, left on an old
        // branch: pippwin asks origin for its default branch, switches to
        // it, and reads pippwin.toml there.
        let home = tempfile::tempdir().unwrap();
        let repo = make_repo(home.path(), LIMIARA);
        let toml = repo.join("pippwin.toml");
        std::fs::remove_file(&toml).unwrap();
        let mut ui = ScriptUi::new([
            Answer::Text(repo.to_string_lossy().into()),
            Answer::Keys("enter"),
        ]);
        let mut run = FakeRunner::default()
            .answer(
                &format!("git -C {} remote get-url origin", repo.display()),
                "https://github.com/buchochelliq-labs/limiara-core.git\n",
            )
            .answer(&format!("git -C {} status --porcelain", repo.display()), "")
            .answer(
                &format!("git -C {} ls-remote --symref origin HEAD", repo.display()),
                "ref: refs/heads/main\tHEAD\n3d4cfd7f5379dbfdcfe7fedd583b9c78929ae9b1\tHEAD\n",
            )
            .answer(
                &format!("git -C {} branch --show-current", repo.display()),
                "main\n",
            );
        let switched = toml.clone();
        run.on_interactive = Some(Box::new(move |cmd: &Cmd| {
            if cmd.display().ends_with("switch main") {
                std::fs::write(&switched, LIMIARA).unwrap();
            }
        }));
        let mut cx = ctx(&mut ui, &mut run, home.path());
        cx.request.project = Some(URL.into());
        run_project(&mut cx).unwrap();
        assert_eq!(cx.state.projects[0].branch.as_deref(), Some("main"));
        let shown: Vec<String> = run.ran.iter().map(Cmd::display).collect();
        // main is the default branch and the branch pippwin.toml names, so
        // one switch is enough.
        assert_eq!(
            shown,
            [
                format!(
                    "git -C {} fetch origin +refs/heads/main:refs/remotes/origin/main",
                    repo.display()
                ),
                format!("git -C {} switch main", repo.display()),
                format!("git -C {} merge --ff-only origin/main", repo.display()),
            ]
        );
    }

    #[test]
    fn a_default_branch_origin_will_not_name_stops_with_a_clear_message() {
        let home = tempfile::tempdir().unwrap();
        let repo = make_repo(home.path(), LIMIARA);
        std::fs::remove_file(repo.join("pippwin.toml")).unwrap();
        let mut ui = ScriptUi::new([
            Answer::Text(repo.to_string_lossy().into()),
            Answer::Keys("enter"),
        ]);
        let mut run = FakeRunner::default()
            .answer(
                &format!("git -C {} remote get-url origin", repo.display()),
                "https://github.com/buchochelliq-labs/limiara-core\n",
            )
            .answer(&format!("git -C {} status --porcelain", repo.display()), "")
            .answer(
                &format!("git -C {} ls-remote --symref origin HEAD", repo.display()),
                "",
            );
        let mut cx = ctx(&mut ui, &mut run, home.path());
        cx.request.project = Some(URL.into());
        let err = run_project(&mut cx).unwrap_err().to_string();
        assert!(err.contains("default branch"), "{err}");
        assert!(run.ran.is_empty());
    }

    #[test]
    fn a_local_folder_is_used_as_it_is() {
        let home = tempfile::tempdir().unwrap();
        let repo = make_repo(home.path(), LIMIARA);
        let mut ui = ScriptUi::new([]);
        let mut run = FakeRunner::default();
        let mut cx = ctx(&mut ui, &mut run, home.path());
        cx.request.project = Some(repo.to_string_lossy().into());
        run_project(&mut cx).unwrap();
        assert!(cx.project.is_some());
        assert!(check_project(&mut cx).is_done());
        // A folder without pippwin.toml is refused, naming `pippwin init`.
        let empty = home.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        cx.request.project = Some(empty.to_string_lossy().into());
        assert!(run_project(&mut cx)
            .unwrap_err()
            .to_string()
            .contains("pippwin init"));
        assert!(run.ran.is_empty());
    }

    #[test]
    fn sources_are_urls_or_full_paths() {
        let home = Path::new("/home/ada");
        assert_eq!(parse_source(URL, home).unwrap(), Source::Url(URL.into()));
        assert_eq!(
            parse_source("~/app", home).unwrap(),
            Source::Folder("/home/ada/app".into())
        );
        assert!(parse_source("http://x/y", home).is_err());
        assert!(parse_source("git@github.com:a/b", home).is_err());
        assert!(parse_source("relative/dir", home).is_err());
    }
}
