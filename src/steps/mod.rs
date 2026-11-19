//! The setup steps. Each one can tell whether it is done (`check`), and
//! does its work (`run`) only when it is not, so `setup` can be run again
//! at any time and picks up where it stopped.

pub mod build;
pub mod login;
pub mod phone;
pub mod project;
pub mod sdk;
pub mod system;
pub mod tools;

use intuituive::StepState;

use crate::cmd::Runner;
use crate::error::{Error, Result};
use crate::parse::OsRelease;
use crate::paths::Paths;
use crate::project::Project;
use crate::state::State;
use crate::ui::{OverviewItem, Ui};
use crate::versions::Arch;

/// What the machine is.
#[derive(Clone, Debug)]
pub struct Host {
    pub wsl: bool,
    pub os: Option<OsRelease>,
    pub arch: Option<Arch>,
}

impl Host {
    pub fn detect() -> Host {
        let wsl = std::fs::read_to_string("/proc/version")
            .map(|v| crate::parse::is_wsl_kernel(&v))
            .unwrap_or(false);
        let os = std::fs::read_to_string("/etc/os-release")
            .ok()
            .map(|t| crate::parse::os_release(&t));
        Host {
            wsl,
            os,
            arch: Arch::current(),
        }
    }

    pub fn arch(&self) -> Result<Arch> {
        self.arch.ok_or(Error::UnsupportedArch)
    }
}

/// The project asked for on the command line (`--project`, `--branch`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Request {
    /// A git URL or a local folder.
    pub project: Option<String>,
    pub branch: Option<String>,
}

pub struct Ctx<'a> {
    pub ui: &'a mut dyn Ui,
    pub run: &'a mut dyn Runner,
    pub paths: Paths,
    pub state: State,
    pub host: Host,
    /// The project being set up or built, once known.
    pub project: Option<Project>,
    /// Why the remembered project could not be loaded, if it could not.
    pub project_problem: Option<String>,
    pub request: Request,
    /// Where pippwin was started.
    pub cwd: std::path::PathBuf,
}

impl<'a> Ctx<'a> {
    pub fn new(
        ui: &'a mut dyn Ui,
        run: &'a mut dyn Runner,
        paths: Paths,
        state: State,
        host: Host,
    ) -> Ctx<'a> {
        let cwd = paths.home.clone();
        Ctx {
            ui,
            run,
            paths,
            state,
            host,
            project: None,
            project_problem: None,
            request: Request::default(),
            cwd,
        }
    }

    /// Load the project remembered in the state file, if there is one.
    pub fn load_current_project(&mut self) {
        let Some(root) = self.state.current.clone() else {
            return;
        };
        match Project::load(&root) {
            Ok(project) => self.project = Some(project),
            Err(error) => self.project_problem = Some(error.to_string()),
        }
    }

    /// The project's name, or a neutral word before one is chosen.
    pub fn project_name(&self) -> String {
        self.project
            .as_ref()
            .map(|p| p.name().to_string())
            .unwrap_or_else(|| "your app".into())
    }

    pub fn save_state(&self) -> Result<()> {
        self.state.save(&self.paths.state_file())
    }

    /// Show a command that needs `sudo` and ask before running it.
    pub fn confirm_sudo(&mut self, cmds: &[crate::cmd::Cmd], why: &str) -> Result<bool> {
        self.ui.say(&format!(
            "This needs administrator rights ({why}). sudo will ask for [bold]your Linux password[/]; it is typed into sudo, not into this tool. These will run:"
        ));
        for cmd in cmds {
            self.ui.show_command(cmd);
        }
        self.ui.confirm("Run them now?", true)
    }
}

/// The project `run` and `doctor` work on: `explicit`, else the project
/// in the current directory, else the one set up last. A project found
/// this way is recorded in the state file.
pub fn select_project(cx: &mut Ctx, explicit: Option<&std::path::Path>) -> Result<()> {
    let root = match explicit {
        Some(dir) => {
            if !Project::file_path(dir).is_file() {
                return Err(Error::NotReady(format!(
                    "{} has no {}",
                    dir.display(),
                    crate::versions::PROJECT_FILE
                )));
            }
            Some(dir.to_path_buf())
        }
        None => crate::project::find_root(&cx.cwd),
    };
    let Some(root) = root else {
        cx.load_current_project();
        return Ok(());
    };
    let project = Project::load(&root)?;
    if cx.state.record(&project.root).is_none() {
        cx.state.remember(crate::state::ProjectRecord {
            name: project.name().to_string(),
            root: project.root.clone(),
            repository: project.file.project.repository.clone(),
            branch: None,
            installed: false,
        });
    } else {
        cx.state.current = Some(project.root.clone());
    }
    cx.project = Some(project);
    cx.save_state()
}

/// Where a step stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Done(String),
    Missing(String),
    Problem(String),
    /// Cannot be checked here (e.g. outside WSL).
    Unknown(String),
}

impl Status {
    pub fn is_done(&self) -> bool {
        matches!(self, Status::Done(_))
    }

    pub fn detail(&self) -> &str {
        match self {
            Status::Done(d) | Status::Missing(d) | Status::Problem(d) | Status::Unknown(d) => d,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Status::Done(_) => "[green]done[/]",
            Status::Missing(_) => "[yellow]missing[/]",
            Status::Problem(_) => "[red]problem[/]",
            Status::Unknown(_) => "[dim]unknown[/]",
        }
    }
}

type CheckFn = fn(&mut Ctx) -> Status;
type RunFn = fn(&mut Ctx) -> Result<()>;

pub struct Step {
    pub id: &'static str,
    pub title: &'static str,
    /// What it does and why, shown before it runs.
    pub why: &'static str,
    pub check: CheckFn,
    pub run: RunFn,
    /// Run even when done (the login step offers to keep or switch).
    pub when_done: Option<RunFn>,
}

pub fn all() -> Vec<Step> {
    vec![
        system::WSL,
        system::APT,
        project::PROJECT,
        system::PROJECT_APT,
        tools::SWIFT,
        tools::XTOOL,
        tools::RUST,
        tools::NODE,
        sdk::SDK,
        login::LOGIN,
        phone::PHONE,
        build::BUILD,
    ]
}

/// The guided setup: an overview, then each step in turn.
pub fn setup(cx: &mut Ctx) -> Result<()> {
    let steps = all();
    cx.ui
        .heading("pippwin: an iOS app on your iPhone, with xtool");
    cx.ui.say(
        "This sets up WSL to build an iOS project and install it on your iPhone with xtool. \
It can be stopped and run again at any time: finished steps are skipped.",
    );
    let statuses: Vec<Status> = {
        // A spinner only on a terminal; it writes straight to stdout.
        let spinner = cx.ui.console().is_terminal().then(|| {
            cx.ui.console().status(rich::status::Status::new(
                "Checking what is already set up…",
            ))
        });
        let statuses = steps.iter().map(|s| (s.check)(cx)).collect();
        if let Some(spinner) = spinner {
            let _ = spinner.stop();
        }
        statuses
    };
    let items: Vec<OverviewItem> = steps
        .iter()
        .zip(&statuses)
        .map(|(step, status)| OverviewItem {
            title: step.title.to_string(),
            state: match status {
                Status::Done(_) => StepState::Done,
                Status::Problem(_) => StepState::Failed,
                _ => StepState::Pending,
            },
            detail: status.detail().to_string(),
        })
        .collect();
    if !cx.ui.overview(&items)? {
        return Err(Error::Cancelled);
    }
    for (index, step) in steps.iter().enumerate() {
        run_step(cx, step, index + 1, steps.len())?;
    }
    cx.ui.heading("All done");
    let folder = cx
        .project
        .as_ref()
        .map(|p| p.root.display().to_string())
        .unwrap_or_default();
    cx.ui.say(&format!(
        "Next time, [bold]pippwin run[/] in {} rebuilds and reinstalls {}.",
        rich::markup::escape(&folder),
        rich::markup::escape(&cx.project_name())
    ));
    Ok(())
}

pub fn run_step(cx: &mut Ctx, step: &Step, number: usize, total: usize) -> Result<()> {
    let status = (step.check)(cx);
    if let Status::Done(detail) = &status {
        cx.ui.done(&format!(
            "[bold]{number}/{total} {}[/] [dim]{}[/]",
            step.title,
            rich::markup::escape(detail)
        ));
        if let Some(when_done) = step.when_done {
            when_done(cx)?;
        }
        cx.state.mark_done(step.id);
        return cx.save_state();
    }
    cx.ui.heading(&format!("{number}/{total} {}", step.title));
    cx.ui.say(step.why);
    if !status.detail().is_empty() {
        cx.ui
            .note(&format!("Now: {}", rich::markup::escape(status.detail())));
    }
    (step.run)(cx)?;
    let after = (step.check)(cx);
    match after {
        Status::Done(detail) => {
            cx.ui.done(&format!(
                "{} [dim]{}[/]",
                step.title,
                rich::markup::escape(&detail)
            ));
            cx.state.mark_done(step.id);
            cx.save_state()
        }
        other => Err(Error::NotReady(format!(
            "{} is not finished: {}",
            step.title,
            other.detail()
        ))),
    }
}

/// `doctor`: every check, in a table.
pub fn doctor(cx: &mut Ctx) -> Result<bool> {
    let mut rows: Vec<(String, Status)> = Vec::new();
    for step in all() {
        let status = if !cx.host.wsl && matches!(step.id, "phone") {
            Status::Unknown("needs WSL (usbipd runs on Windows)".into())
        } else {
            (step.check)(cx)
        };
        rows.push((step.title.to_string(), status));
        if step.id == "project" {
            if let Some(project) = &cx.project {
                rows.push((
                    "App folder".into(),
                    Status::Done(format!(
                        "{} ({})",
                        project.app_dir.display(),
                        project
                            .bundle_id
                            .as_deref()
                            .unwrap_or("no bundleID in xtool.yml")
                    )),
                ));
            }
        }
        if step.id == "xtool" {
            rows.push(("~/.local/bin on PATH".into(), tools::path_check(cx)));
        }
        if step.id == "login" {
            rows.push((
                "Login token permissions".into(),
                login::token_permissions(cx),
            ));
            rows.push(("API key file".into(), login::key_file_check(cx)));
        }
        if step.id == "phone" && cx.host.wsl {
            rows.push(("usbipd (Windows)".into(), phone::usbipd_check(cx)));
            rows.push(("usbmuxd".into(), phone::usbmuxd_check(cx)));
            rows.push(("Automatic reconnect".into(), phone::auto_attach_check(cx)));
        }
    }
    let mut table = rich::table::Table::new().title("pippwin doctor");
    table
        .add_column("Check")
        .add_column("Status")
        .add_column("Detail");
    let mut healthy = true;
    for (name, status) in &rows {
        healthy &= matches!(status, Status::Done(_) | Status::Unknown(_));
        let label = cx.ui.console().build_text(status.label());
        table.add_row_text(vec![
            rich::text::Text::new(name.as_str()),
            label,
            rich::text::Text::new(status.detail()),
        ]);
    }
    cx.ui.console().print(&table);
    if !cx.host.wsl {
        cx.ui.warn(
            "Not running in WSL",
            "This machine is not WSL, so the Windows-side checks (usbipd, the iPhone) were skipped. \
On Windows, run pippwin's [bold]scripts/setup-windows.ps1[/] (see the README) to get a WSL Ubuntu.",
        );
    }
    Ok(healthy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::fake::FakeRunner;
    use crate::ui::testing::ScriptUi;

    #[test]
    fn doctor_degrades_outside_wsl() {
        let home = tempfile::tempdir().unwrap();
        let mut ui = ScriptUi::new([]);
        let mut run = FakeRunner::default();
        let mut cx = Ctx::new(
            &mut ui,
            &mut run,
            Paths::from_home(home.path().to_path_buf()),
            State::default(),
            Host {
                wsl: false,
                os: Some(crate::parse::os_release(
                    "ID=debian\nPRETTY_NAME=\"Debian 12\"",
                )),
                arch: None,
            },
        );
        let healthy = doctor(&mut cx).unwrap();
        assert!(!healthy);
        let text = ui.transcript();
        assert!(text.contains("Debian 12, not WSL"));
        assert!(text.contains("needs WSL"));
        assert!(text.contains("Not running in WSL"));
        // Nothing ran on the terminal, and usbipd was never called.
        assert!(run.ran.is_empty());
        assert!(!run.captured.iter().any(|c| c.program.contains("usbipd")));
    }

    #[test]
    fn doctor_flags_loose_token_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let paths = Paths::from_home(home.path().to_path_buf());
        std::fs::create_dir_all(paths.xtool_data()).unwrap();
        std::fs::write(paths.xtool_token(), b"{}").unwrap();
        std::fs::set_permissions(paths.xtool_token(), std::fs::Permissions::from_mode(0o644))
            .unwrap();
        let mut ui = ScriptUi::new([]);
        let mut run = FakeRunner::default();
        let mut cx = Ctx::new(
            &mut ui,
            &mut run,
            paths,
            State::default(),
            Host {
                wsl: false,
                os: None,
                arch: None,
            },
        );
        assert!(
            matches!(login::token_permissions(&mut cx), Status::Problem(p) if p.contains("0644"))
        );
        login::secure_token(&mut cx).unwrap();
        assert!(login::token_permissions(&mut cx).is_done());
    }
}
