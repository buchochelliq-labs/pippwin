//! Step 11: the iPhone, passed through from Windows to WSL over USB.

use std::time::Duration;

use super::{Ctx, Status, Step};
use crate::cmd::{Cmd, Input};
use crate::error::{Error, Result};
use crate::parse::{self, UsbDevice};
use crate::validate;
use crate::versions;

pub const PHONE: Step = Step {
    id: "phone",
    title: "iPhone connection",
    why: "WSL does not see USB devices by itself. usbipd-win, on Windows, passes the iPhone through to \
WSL: it is bound once (an administrator action, so Windows asks for permission) and attached each time \
you plug it in. Inside WSL, usbmuxd talks to the phone.",
    check: check_phone,
    run: run_phone,
    when_done: None,
};

fn devices(cx: &mut Ctx) -> Vec<String> {
    cx.run
        .capture(&Cmd::new("idevice_id").arg("-l"))
        .map(|o| parse::idevice_ids(&o.stdout))
        .unwrap_or_default()
}

fn check_phone(cx: &mut Ctx) -> Status {
    match devices(cx).first() {
        Some(udid) => Status::Done(format!("iPhone {udid}")),
        None => Status::Missing("no iPhone is visible in WSL".into()),
    }
}

pub fn usbipd_check(cx: &mut Ctx) -> Status {
    if !cx.run.exists("usbipd.exe") {
        return Status::Missing("usbipd-win is not installed on Windows".into());
    }
    match apple_devices(cx) {
        Ok(list) if list.iter().any(UsbDevice::is_attached) => {
            Status::Done("iPhone attached to WSL".into())
        }
        Ok(list) if list.iter().any(UsbDevice::is_shared) => {
            Status::Missing("iPhone shared, not attached".into())
        }
        Ok(list) if !list.is_empty() => Status::Missing("iPhone connected, not shared yet".into()),
        Ok(_) => Status::Missing("no Apple device on USB".into()),
        Err(_) => Status::Problem("usbipd list failed".into()),
    }
}

pub fn usbmuxd_check(cx: &mut Ctx) -> Status {
    if usbmuxd_running(cx) {
        Status::Done("running".into())
    } else {
        Status::Missing("not running".into())
    }
}

fn usbmuxd_running(cx: &mut Ctx) -> bool {
    cx.run
        .capture(&Cmd::new("pgrep").args(["-x", "usbmuxd"]))
        .is_ok_and(|o| o.success)
}

fn apple_devices(cx: &mut Ctx) -> Result<Vec<UsbDevice>> {
    let out = cx.run.capture(&Cmd::new("usbipd.exe").arg("list"))?;
    if !out.success {
        return Err(Error::CommandFailed {
            program: "usbipd.exe".into(),
            status: "list failed".into(),
        });
    }
    Ok(parse::usbipd_list(&out.stdout)
        .into_iter()
        .filter(UsbDevice::is_apple)
        .collect())
}

/// `usbipd bind` needs administrator rights: started through PowerShell's
/// `Start-Process -Verb RunAs`, so Windows shows its own UAC prompt.
pub fn bind_cmd(bus_id: &str) -> Result<Cmd> {
    if !validate::bus_id(bus_id) {
        return Err(Error::InvalidInput(
            "usbipd printed a bus ID this tool does not understand".into(),
        ));
    }
    Ok(Cmd::new("powershell.exe").args([
        "-NoProfile",
        "-Command",
        &format!("Start-Process -FilePath usbipd -Verb RunAs -Wait -ArgumentList 'bind','--busid','{bus_id}'"),
    ]))
}

pub fn attach_cmd(bus_id: &str) -> Result<Cmd> {
    if !validate::bus_id(bus_id) {
        return Err(Error::InvalidInput(
            "usbipd printed a bus ID this tool does not understand".into(),
        ));
    }
    Ok(Cmd::new("usbipd.exe").args(["attach", "--wsl", "--busid", bus_id]))
}

fn mux_error_note(cx: &mut Ctx) {
    cx.ui.say(&format!(
        "If xtool later fails with [bold]AFCClient.Error.muxError[/], xtool's guide points to a workaround \
that relays the connection from iTunes on Windows instead of usbipd: {}",
        versions::MUX_ERROR_WORKAROUND
    ));
}

/// How long to wait for usbmuxd to start on its own after the phone is attached.
const USBMUXD_WAIT: Duration = Duration::from_secs(5);

/// Whether `check` becomes true within `limit`, checking once a second.
fn wait_for(limit: Duration, mut check: impl FnMut() -> bool) -> bool {
    let tries = limit.as_secs().max(1);
    for attempt in 0..tries {
        if check() {
            return true;
        }
        if attempt + 1 < tries {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    false
}

/// Keeps the iPhone attached across replugs: usbipd-win's own `--auto-attach`,
/// in a minimized Windows window, until that window is closed or Windows
/// restarts. No administrator rights are needed once the phone is shared.
pub fn auto_attach_cmd(bus_id: &str) -> Result<Cmd> {
    if !validate::bus_id(bus_id) {
        return Err(Error::InvalidInput(
            "usbipd printed a bus ID this tool does not understand".into(),
        ));
    }
    Ok(Cmd::new("powershell.exe").args([
        "-NoProfile",
        "-Command",
        &format!(
            "Start-Process -FilePath usbipd -WindowStyle Minimized -ArgumentList 'attach','--wsl','--busid','{bus_id}','--auto-attach'"
        ),
    ]))
}

/// The bus IDs usbipd is keeping attached with `--auto-attach`, from the
/// command lines of the running usbipd processes.
fn auto_attached(cx: &mut Ctx) -> Option<Vec<String>> {
    let out = cx
        .run
        .capture(&Cmd::new("powershell.exe").args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_Process -Filter \"Name='usbipd.exe'\" | ForEach-Object { $_.CommandLine }",
        ]))
        .ok()
        .filter(|o| o.success)?;
    Some(parse::auto_attach_bus_ids(&out.stdout))
}

/// `doctor`: whether replugging the iPhone reconnects it by itself.
pub fn auto_attach_check(cx: &mut Ctx) -> Status {
    match auto_attached(cx) {
        Some(ids) if !ids.is_empty() => Status::Done(format!("on, for bus {}", ids.join(", "))),
        Some(_) => Status::Missing(
            "off: after a replug, `pippwin run` offers to connect the iPhone (or run `pippwin phone`)".into(),
        ),
        None => Status::Unknown("could not ask Windows".into()),
    }
}

fn offer_auto_attach(cx: &mut Ctx, bus_id: &str) -> Result<()> {
    if auto_attached(cx).is_some_and(|ids| ids.iter().any(|id| id == bus_id)) {
        return Ok(());
    }
    cx.ui.say(
        "usbipd can reconnect the iPhone to WSL by itself whenever you plug it in. It keeps a minimized \
usbipd window open on Windows; closing that window, or restarting Windows, turns it off.",
    );
    if cx
        .ui
        .confirm("Reconnect automatically whenever you plug it in?", true)?
    {
        cx.run.interactive(&auto_attach_cmd(bus_id)?)?;
    }
    Ok(())
}

/// Before building: the iPhone must be visible in WSL. When it is plugged in
/// but not connected (after a replug, say), offer to connect it; nothing is
/// connected without asking.
pub fn connect_if_needed(cx: &mut Ctx) -> Result<()> {
    if !cx.host.wsl {
        return Ok(());
    }
    loop {
        if !devices(cx).is_empty() {
            return Ok(());
        }
        if !cx.run.exists("usbipd.exe") {
            cx.ui.note("No iPhone is visible in WSL, and usbipd-win is not installed: run [bold]pippwin phone[/].");
            return Ok(());
        }
        let Ok(list) = apple_devices(cx) else {
            cx.ui
                .note("No iPhone is visible in WSL, and usbipd could not list USB devices.");
            return Ok(());
        };
        if list.is_empty() {
            cx.ui
                .say("No iPhone is connected. Plug it in with a USB cable and unlock it.");
            let options = vec![
                "Look again".to_string(),
                "Continue anyway".to_string(),
                "Stop".to_string(),
            ];
            match cx
                .ui
                .choose("The iPhone is needed to install the app.", &options, 0)?
            {
                0 => continue,
                1 => return Ok(()),
                _ => return Err(Error::Cancelled),
            }
        }
        let options = vec![
            "Connect it".to_string(),
            "Continue without it".to_string(),
            "Stop".to_string(),
        ];
        match cx.ui.choose(
            "Your iPhone is plugged in but not connected to WSL. Connect it now?",
            &options,
            0,
        )? {
            0 => return run_phone(cx),
            1 => return Ok(()),
            _ => return Err(Error::Cancelled),
        }
    }
}

fn run_phone(cx: &mut Ctx) -> Result<()> {
    cx.ui
        .say(&format!("Microsoft's guide: {}", versions::USBIPD_GUIDE));
    if !cx.run.exists("usbipd.exe") {
        cx.ui.warn(
            "usbipd-win is missing",
            "Install it on Windows: run pippwin's [bold]scripts/setup-windows.ps1[/] again, or in PowerShell \
[bold]winget install --interactive --exact dorssel.usbipd-win[/]. Then close and reopen this terminal \
and run setup again.",
        );
        mux_error_note(cx);
        return Err(Error::NotReady(
            "usbipd-win is not installed on Windows".into(),
        ));
    }
    let device = loop {
        let list = apple_devices(cx)?;
        match list.len() {
            0 => {
                cx.ui
                    .say("Plug the iPhone in with a USB cable and unlock it.");
                let options = vec!["Look again".to_string(), "Stop".to_string()];
                if cx.ui.choose("No iPhone on USB yet.", &options, 0)? == 1 {
                    return Err(Error::Cancelled);
                }
            }
            1 => break list[0].clone(),
            _ => {
                let options: Vec<String> = list
                    .iter()
                    .map(|d| format!("{} {} ({})", d.bus_id, d.description, d.state))
                    .collect();
                let choice = cx.ui.choose("Which device is the iPhone?", &options, 0)?;
                break list[choice].clone();
            }
        }
    };
    if !device.is_shared() {
        let cmd = bind_cmd(&device.bus_id)?;
        cx.ui.say(
            "Sharing the iPhone with WSL is done once, as administrator. Windows will ask for permission \
(a User Account Control prompt) to run:",
        );
        cx.ui
            .show_command(&Cmd::new("usbipd").args(["bind", "--busid", &device.bus_id]));
        if !cx.ui.confirm("Share it now?", true)? {
            return Err(Error::Cancelled);
        }
        cx.run.interactive(&cmd)?;
    }
    if !device.is_attached() {
        cx.run.interactive(&attach_cmd(&device.bus_id)?)?;
    }
    // Where WSL runs systemd, usbmuxd's udev rule starts it when the phone is
    // attached: give it a moment before asking for sudo to start it by hand.
    if !wait_for(USBMUXD_WAIT, || usbmuxd_running(cx)) {
        let cmd = Cmd::new("sudo").arg("usbmuxd").input(Input::Inherit);
        if !cx.confirm_sudo(
            std::slice::from_ref(&cmd),
            "starting usbmuxd, which talks to the iPhone",
        )? {
            return Err(Error::Cancelled);
        }
        cx.run.interactive(&cmd)?;
    }
    offer_auto_attach(cx, &device.bus_id)?;
    cx.ui.say(
        "Unlock the iPhone. If it asks [bold]Trust This Computer?[/], tap Trust and enter its passcode \
(xtool shows \"Please tap 'trust' on your device...\" while it waits for that).",
    );
    for _ in 0..15 {
        if !devices(cx).is_empty() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    mux_error_note(cx);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_is_elevated_and_bus_ids_are_checked() {
        let cmd = bind_cmd("2-4").unwrap();
        assert_eq!(cmd.program, "powershell.exe");
        assert!(cmd.args[2].contains("-Verb RunAs"));
        assert!(cmd.args[2].contains("'bind','--busid','2-4'"));
        assert!(bind_cmd("2-4'; Remove-Item C:\\ #").is_err());
        assert_eq!(
            attach_cmd("1-1").unwrap().args,
            ["attach", "--wsl", "--busid", "1-1"]
        );
        assert!(attach_cmd("--help").is_err());
        let auto = auto_attach_cmd("2-4").unwrap();
        assert!(auto.args[2].contains("-WindowStyle Minimized"));
        assert!(auto.args[2].contains("'attach','--wsl','--busid','2-4','--auto-attach'"));
        assert!(auto_attach_cmd("2-4'; calc #").is_err());
    }

    use crate::cmd::fake::FakeRunner;
    use crate::paths::Paths;
    use crate::state::State;
    use crate::steps::Host;
    use crate::ui::testing::{Answer, ScriptUi};

    const SHARED: &str = "Connected:\n\
BUSID  VID:PID    DEVICE                                                        STATE\n\
2-4    05ac:12a8  Apple Mobile Device USB Device, Apple Mobile Device Ethernet  Shared\n";
    const NOTHING: &str = "Connected:\n\
BUSID  VID:PID    DEVICE                                                        STATE\n\
2-3    8087:0026  Intel(R) Wireless Bluetooth(R)                                Not shared\n";
    const UDID: &str = "00008110-001A2B3C4D5E801E\n";

    fn with_ctx<T>(
        ui: &mut ScriptUi,
        run: &mut FakeRunner,
        wsl: bool,
        f: impl FnOnce(&mut Ctx) -> T,
    ) -> T {
        let home = tempfile::tempdir().unwrap();
        let mut cx = Ctx::new(
            ui,
            run,
            Paths::from_home(home.path().to_path_buf()),
            State::default(),
            Host {
                wsl,
                os: None,
                arch: None,
            },
        );
        f(&mut cx)
    }

    #[test]
    fn nothing_is_asked_outside_wsl_or_when_the_iphone_is_visible() {
        let (mut ui, mut run) = (ScriptUi::new([]), FakeRunner::default());
        with_ctx(&mut ui, &mut run, false, connect_if_needed).unwrap();
        assert!(run.captured.is_empty());

        let mut run = FakeRunner::default().answer("idevice_id", UDID);
        with_ctx(&mut ui, &mut run, true, connect_if_needed).unwrap();
        assert!(ui.questions.is_empty() && run.ran.is_empty());
    }

    #[test]
    fn a_replugged_iphone_is_connected_when_the_user_says_so() {
        // Not visible at first; visible once attached.
        let mut run = FakeRunner::default()
            .answer_once("idevice_id", "")
            .answer("idevice_id", UDID)
            .answer("usbipd.exe list", SHARED)
            .answer("pgrep", "123\n")
            .answer("powershell.exe -NoProfile -Command", "");
        run.programs = vec!["usbipd.exe".into()];
        // Connect it, then yes to reconnecting automatically.
        let mut ui = ScriptUi::new([Answer::Keys("enter"), Answer::Yes(true)]);
        with_ctx(&mut ui, &mut run, true, connect_if_needed).unwrap();
        assert_eq!(
            ui.questions[0],
            "Your iPhone is plugged in but not connected to WSL. Connect it now?"
        );
        let ran: Vec<String> = run.ran.iter().map(|c| c.display()).collect();
        // Already shared: attach (no bind, so no UAC prompt), then auto-attach; usbmuxd was running.
        assert_eq!(ran[0], "usbipd.exe attach --wsl --busid 2-4", "{ran:?}");
        assert!(ran[1].contains("--auto-attach"), "{ran:?}");
        assert_eq!(
            ran.len(),
            2,
            "no sudo: usbmuxd was already running: {ran:?}"
        );
    }

    #[test]
    fn an_unplugged_iphone_can_be_skipped_or_stop_the_build() {
        let make = || {
            let mut run = FakeRunner::default().answer("usbipd.exe list", NOTHING);
            run.programs = vec!["usbipd.exe".into()];
            run
        };
        let (mut ui, mut run) = (ScriptUi::new([Answer::Keys("down enter")]), make());
        with_ctx(&mut ui, &mut run, true, connect_if_needed).unwrap();
        assert!(ui.transcript().contains("No iPhone is connected"));
        assert!(run.ran.is_empty());

        let (mut ui, mut run) = (ScriptUi::new([Answer::Keys("down down enter")]), make());
        let stopped = with_ctx(&mut ui, &mut run, true, connect_if_needed);
        assert!(matches!(stopped, Err(Error::Cancelled)));
    }

    #[test]
    fn auto_attach_is_reported_and_not_offered_twice() {
        let lines = "\"C:\\Program Files\\usbipd-win\\usbipd.exe\" server\n\
\"C:\\Program Files\\usbipd-win\\usbipd.exe\" attach --wsl --busid 2-4 --auto-attach\n";
        let mut run = FakeRunner::default().answer("powershell.exe -NoProfile -Command", lines);
        let mut ui = ScriptUi::new([]);
        let status = with_ctx(&mut ui, &mut run, true, |cx| {
            offer_auto_attach(cx, "2-4").unwrap();
            auto_attach_check(cx)
        });
        assert!(ui.questions.is_empty(), "already on: not offered again");
        assert_eq!(status.detail(), "on, for bus 2-4");
    }
}
