//! `pippwin`: a guided setup that takes a Windows machine from nothing to
//! an xtool-based iOS project built and installed on an iPhone, inside
//! WSL. The project is described by a `pippwin.toml` at the root of its
//! repository. See README.md and docs/.

#![forbid(unsafe_code)]

mod checksum;
mod cmd;
mod download;
mod error;
mod init;
mod parse;
mod paths;
mod perms;
mod project;
mod state;
mod steps;
mod ui;
mod validate;
mod versions;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::error::{Error, Result};
use crate::steps::{Ctx, Host, Request};
use crate::ui::{TermUi, Ui};

#[derive(Parser)]
#[command(
    name = "pippwin",
    version,
    about = "Set up WSL to build an xtool-based iOS project and install it on an iPhone",
    long_about = "Set up WSL to build an xtool-based iOS project and install it on an iPhone.\n\n\
The project is described by a pippwin.toml at the root of its repository (`pippwin init` writes \
one). Passwords and two-factor codes are never asked for by pippwin: they are typed straight into \
xtool or sudo. pippwin is not affiliated with Apple or with xtool."
)]
struct Cli {
    /// Answer yes/no confirmations with their default (never used for
    /// passwords, codes or keys, which are always typed by you).
    #[arg(long, short = 'y', global = true)]
    yes: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// The whole guided setup. Safe to run again: finished steps are skipped.
    Setup {
        /// The project: its https git URL, or a folder in WSL holding its
        /// pippwin.toml. Asked for when not given.
        #[arg(long, value_name = "URL or PATH")]
        project: Option<String>,
        /// The branch to check out. Asked for when not given (default: the
        /// one the project's pippwin.toml names).
        #[arg(long, value_name = "NAME", value_parser = parse_branch)]
        branch: Option<String>,
    },
    /// Check every step and show what is done, missing or wrong.
    Doctor {
        /// Also tighten the permissions of xtool's login token and the key.
        #[arg(long)]
        fix: bool,
    },
    /// Log xtool in to Apple (also to switch account or login mode).
    Login,
    /// Install the iOS SDK from Xcode.xip.
    Sdk,
    /// Connect the iPhone to WSL.
    Phone,
    /// Build the app (the project's build.prepare) and install it (xtool dev).
    Run {
        /// The project's folder (default: the project in the current
        /// directory, else the one set up last).
        #[arg(long, value_name = "PATH")]
        project: Option<PathBuf>,
    },
    /// Write a commented pippwin.toml into the current repository.
    Init {
        /// The folder holding xtool.yml and Package.swift, relative to the
        /// repository's root. Asked for when not given.
        #[arg(long, value_name = "PATH")]
        app: Option<String>,
    },
    /// Check a project's pippwin.toml and the folders it names.
    Check {
        /// The project's folder (default: the project in the current
        /// directory).
        #[arg(long, value_name = "PATH")]
        project: Option<PathBuf>,
    },
}

fn parse_branch(value: &str) -> std::result::Result<String, String> {
    validate::branch(value)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut ui = TermUi::new(cli.yes);
    match run(&cli, &mut ui) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::Cancelled) => {
            ui.say("[yellow]Stopped.[/] Run the same command again to carry on.");
            ExitCode::from(130)
        }
        Err(error) => {
            ui.say(&format!(
                "[bold red]error[/] [dim]{}[/]: {}",
                error.code(),
                rich::markup::escape(&error.to_string())
            ));
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli, ui: &mut dyn Ui) -> Result<()> {
    let paths = paths::Paths::detect().ok_or(Error::NotReady("HOME is not set".into()))?;
    let cwd = std::env::current_dir().map_err(|e| Error::file(".", &e))?;
    if let Command::Init { app } = &cli.command {
        let mut runner = cmd::System::new(&[]);
        return init::run(ui, &mut runner, &cwd, app.as_deref());
    }
    let state = state::State::load(&paths.state_file())?;
    let mut runner = cmd::System::new(&paths.tool_dirs());
    let mut cx = Ctx::new(ui, &mut runner, paths, state, Host::detect());
    cx.cwd = cwd;
    match &cli.command {
        Command::Setup { project, branch } => {
            cx.request = Request {
                project: project.clone(),
                branch: branch.clone(),
            };
            cx.load_current_project();
            steps::setup(&mut cx)
        }
        Command::Doctor { fix } => {
            steps::select_project(&mut cx, None)?;
            if *fix {
                let left = perms::fix(&cx.paths)?;
                for finding in left {
                    cx.ui.say(&format!(
                        "[red]✗[/] {}",
                        rich::markup::escape(&finding.describe())
                    ));
                }
            }
            steps::doctor(&mut cx).map(|_| ())
        }
        Command::Login => {
            cx.ui.heading("Apple login for xtool");
            steps::login::run_login(&mut cx)
        }
        Command::Sdk => steps::run_step(&mut cx, &steps::sdk::SDK, 1, 1),
        Command::Phone => steps::run_step(&mut cx, &steps::phone::PHONE, 1, 1),
        Command::Run { project } => {
            steps::select_project(&mut cx, project.as_deref())?;
            if cx.project.is_none() {
                if let Some(problem) = cx.project_problem.clone() {
                    return Err(Error::NotReady(problem));
                }
            }
            let name = cx.project_name();
            cx.ui.heading(&format!("Build and install {name}"));
            steps::build::run(&mut cx)
        }
        Command::Check { project } => init::check(cx.ui, &cx.cwd, project.as_deref()),
        Command::Init { .. } => Ok(()),
    }
}
