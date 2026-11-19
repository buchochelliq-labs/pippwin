//! The last step, and `pippwin run`: the project's `build.prepare` (when it
//! has one), then `xtool dev`, both in `app.path` under `umask 077`.

use super::{Ctx, Status, Step};
use crate::error::{Error, Result};

pub const BUILD: Step = Step {
    id: "build",
    title: "Build and install the app",
    why: "First the project's own preparation runs (build.prepare in its pippwin.toml, if it has one: \
the first time can take a while); then xtool builds the app, signs it with your login and installs it on \
the iPhone.",
    check: check_build,
    run,
    when_done: None,
};

fn check_build(cx: &mut Ctx) -> Status {
    let Some(project) = cx.project.as_ref() else {
        return Status::Missing("choose the project first".into());
    };
    if cx.state.record(&project.root).is_some_and(|r| r.installed) {
        Status::Done(format!(
            "{} installed before; `pippwin run` rebuilds it",
            project.name()
        ))
    } else {
        Status::Missing("not built yet".into())
    }
}

pub fn run(cx: &mut Ctx) -> Result<()> {
    let Some(project) = cx.project.clone() else {
        return Err(Error::NotReady(
            "no project is set up; run `pippwin setup`, or `pippwin run` in a project's folder"
                .into(),
        ));
    };
    // Before the (possibly long) preparation: a replugged iPhone is noticed
    // now, not when xtool fails to find it.
    super::phone::connect_if_needed(cx)?;
    if let Some(prepare) = project.prepare_cmd() {
        cx.ui.note(&format!(
            "Running {} in {}.",
            rich::markup::escape(&prepare.display()),
            rich::markup::escape(&project.app_dir.to_string_lossy())
        ));
        cx.run.interactive(&prepare)?;
    }
    cx.ui.explain(
        "Next, in xtool",
        "xtool builds, signs and installs the app. While it runs it may say:\n\
  [bold]Please tap 'trust' on your device...[/]  tap Trust on the iPhone and enter its passcode.\n\
  [bold]Please unlock your device...[/]  unlock the iPhone.\n\
  [bold]The following certificates must be revoked:[/] followed by [bold]Continue? (yes/no)[/]  Apple \
allows only a few development certificates per account. Answering yes revokes the ones listed so xtool \
can make a new one; apps already signed with them (on any device) stop opening until reinstalled. \
Answer no to stop instead.",
    );
    cx.run.interactive(&project.xtool_dev_cmd())?;
    super::login::secure_token(cx)?;
    cx.ui.explain(
        "On the iPhone",
        "The first time, open [bold]Settings → General → VPN & Device Management[/], tap your developer \
profile and trust it; the app then opens.\n\
With a free Apple ID the app stops opening after [bold]7 days[/]: run [bold]pippwin run[/] again \
to reinstall it. With a paid membership it lasts a year.",
    );
    cx.state.mark_installed(&project.root);
    cx.state.mark_done("build");
    cx.save_state()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::fake::FakeRunner;
    use crate::cmd::Input;
    use crate::paths::Paths;
    use crate::project::tests::{make_repo, LIMIARA};
    use crate::project::Project;
    use crate::state::{ProjectRecord, State};
    use crate::steps::Host;
    use crate::ui::testing::ScriptUi;

    #[test]
    fn run_is_exactly_prepare_then_xtool_dev_in_the_app_folder_under_umask_077() {
        let dir = tempfile::tempdir().unwrap();
        let repo = make_repo(
            dir.path(),
            &LIMIARA.replace(
                "[\"./prepare.sh\"]",
                "[\"./prepare.sh\", \"--release\", \"a b\"]",
            ),
        );
        let project = Project::load(&repo).unwrap();
        let app = project.app_dir.clone();
        let root = project.root.clone();
        let mut ui = ScriptUi::new([]);
        let mut run_ = FakeRunner::default();
        {
            let mut cx = Ctx::new(
                &mut ui,
                &mut run_,
                Paths::from_home(dir.path().join("home")),
                State::default(),
                Host {
                    wsl: true,
                    os: None,
                    arch: None,
                },
            );
            cx.state.remember(ProjectRecord {
                name: "Limiara".into(),
                root: root.clone(),
                repository: "https://github.com/buchochelliq-labs/limiara-core".into(),
                branch: None,
                installed: false,
            });
            cx.project = Some(project);
            assert!(!check_build(&mut cx).is_done());
            run(&mut cx).unwrap();
            assert!(check_build(&mut cx).is_done());
            assert!(cx.state.record(&root).unwrap().installed);
        }
        let wrapper = ["sh", "-c", "umask 077 && exec \"$0\" \"$@\""];
        assert_eq!(run_.ran.len(), 2);
        let mut prepare: Vec<&str> = wrapper.to_vec();
        prepare.extend(["./prepare.sh", "--release", "a b"]);
        assert_eq!(run_.ran[0].argv(), prepare);
        let mut dev: Vec<&str> = wrapper.to_vec();
        dev.extend(["xtool", "dev"]);
        assert_eq!(run_.ran[1].argv(), dev);
        for cmd in &run_.ran {
            assert_eq!(cmd.cwd.as_deref(), Some(app.as_path()));
            assert!(cmd.private_umask);
            assert_eq!(cmd.input, Input::Inherit);
            assert!(cmd.env.is_empty());
        }
        let transcript = ui.transcript();
        assert!(transcript.contains("Please tap 'trust' on your device..."));
        assert!(transcript.contains("The following certificates must be revoked:"));
        assert!(transcript.contains("7 days"));
    }

    #[test]
    fn without_prepare_only_xtool_dev_runs() {
        let dir = tempfile::tempdir().unwrap();
        let repo = make_repo(
            dir.path(),
            &LIMIARA.replace("prepare = [\"./prepare.sh\"]", ""),
        );
        let mut ui = ScriptUi::new([]);
        let mut run_ = FakeRunner::default();
        let mut cx = Ctx::new(
            &mut ui,
            &mut run_,
            Paths::from_home(dir.path().join("home")),
            State::default(),
            Host {
                wsl: true,
                os: None,
                arch: None,
            },
        );
        cx.project = Some(Project::load(&repo).unwrap());
        run(&mut cx).unwrap();
        assert_eq!(run_.ran.len(), 1);
        assert_eq!(run_.ran[0].display(), "xtool dev");
    }

    #[test]
    fn run_without_a_project_says_how_to_get_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut ui = ScriptUi::new([]);
        let mut run_ = FakeRunner::default();
        let mut cx = Ctx::new(
            &mut ui,
            &mut run_,
            Paths::from_home(dir.path().to_path_buf()),
            State::default(),
            Host {
                wsl: true,
                os: None,
                arch: None,
            },
        );
        assert!(run(&mut cx)
            .unwrap_err()
            .to_string()
            .contains("pippwin setup"));
    }
}
