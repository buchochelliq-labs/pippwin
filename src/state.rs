//! What pippwin remembers between runs, in `~/.config/pippwin/state.toml`
//! (`0600`): the projects set up and where they are, the Apple ID and
//! login mode, and the finished steps. It holds no secrets: no password,
//! no two-factor code, no key contents, no token. The key file's
//! *location* is kept so `doctor` can check its permissions.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{io_reason, Error, Result};
use crate::perms;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    /// The folder (holding `pippwin.toml`) of the project last set up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<PathBuf>,
    /// The Apple ID used in password mode (an account name, not a secret).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple_id: Option<String>,
    /// `key` or `password`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_mode: Option<String>,
    /// Where the API key file was put.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_file: Option<PathBuf>,
    /// Steps finished, by id.
    #[serde(default)]
    pub completed: Vec<String>,
    /// Every project set up, by folder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<ProjectRecord>,
}

/// A project pippwin set up: its name, where it is and where it came from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectRecord {
    pub name: String,
    /// The folder holding its `pippwin.toml`.
    pub root: PathBuf,
    /// The URL it was cloned from (or its `project.repository`).
    pub repository: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Built and installed at least once.
    #[serde(default)]
    pub installed: bool,
}

impl State {
    pub fn load(path: &Path) -> Result<State> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|_| Error::State {
                path: path.to_path_buf(),
                reason: "it is not valid; delete it to start afresh".into(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
            Err(e) => Err(Error::State {
                path: path.to_path_buf(),
                reason: io_reason(&e),
            }),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string(self).map_err(|_| Error::State {
            path: path.to_path_buf(),
            reason: "it could not be encoded".into(),
        })?;
        if let Some(dir) = path.parent() {
            perms::ensure_private_dir(dir)?;
        }
        perms::write_private_file(path, text.as_bytes())
    }

    pub fn record(&self, root: &Path) -> Option<&ProjectRecord> {
        self.projects.iter().find(|p| p.root == root)
    }

    /// Add or update the record for `record.root` and make it current. An
    /// earlier install still counts only when the same repository and
    /// branch are selected again: switching either means the phone holds
    /// a different build, so the build step must run.
    pub fn remember(&mut self, record: ProjectRecord) {
        self.current = Some(record.root.clone());
        match self.projects.iter_mut().find(|p| p.root == record.root) {
            Some(existing) => {
                let same_selection =
                    existing.repository == record.repository && existing.branch == record.branch;
                let installed = same_selection && existing.installed;
                *existing = ProjectRecord {
                    installed: installed || record.installed,
                    ..record
                };
            }
            None => self.projects.push(record),
        }
    }

    pub fn mark_installed(&mut self, root: &Path) {
        if let Some(record) = self.projects.iter_mut().find(|p| p.root == root) {
            record.installed = true;
        }
    }

    pub fn mark_done(&mut self, step: &str) {
        if !self.completed.iter().any(|s| s == step) {
            self.completed.push(step.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn switching_branch_forgets_the_earlier_install() {
        let record = |branch: &str, installed: bool| ProjectRecord {
            name: "Demo".into(),
            root: PathBuf::from("/home/u/demo"),
            repository: "https://example.com/demo".into(),
            branch: Some(branch.into()),
            installed,
        };
        let mut state = State::default();
        state.remember(record("main", true));
        // The same selection again keeps the install.
        state.remember(record("main", false));
        assert!(state.record(Path::new("/home/u/demo")).unwrap().installed);
        // Another branch must be built and installed again.
        state.remember(record("feature", false));
        let now = state.record(Path::new("/home/u/demo")).unwrap();
        assert_eq!(now.branch.as_deref(), Some("feature"));
        assert!(!now.installed);
        // Coming back to main after that does not revive the old install.
        state.remember(record("main", false));
        assert!(!state.record(Path::new("/home/u/demo")).unwrap().installed);
    }

    #[test]
    fn round_trip_is_0600() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cfg/state.toml");
        let mut state = State {
            apple_id: Some("me@example.com".into()),
            login_mode: Some("password".into()),
            ..State::default()
        };
        state.mark_done("apt");
        state.mark_done("apt");
        let record = ProjectRecord {
            name: "Example".into(),
            root: "/home/ada/example".into(),
            repository: "https://github.com/example/example".into(),
            branch: Some("main".into()),
            installed: false,
        };
        state.remember(record.clone());
        state.mark_installed(Path::new("/home/ada/example"));
        // Remembering it again keeps `installed`.
        state.remember(record);
        assert!(
            state
                .record(Path::new("/home/ada/example"))
                .unwrap()
                .installed
        );
        assert_eq!(state.projects.len(), 1);
        assert_eq!(
            state.current.as_deref(),
            Some(Path::new("/home/ada/example"))
        );
        state.save(&path).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(path.parent().unwrap()).unwrap().mode() & 0o777,
            0o700
        );
        assert_eq!(State::load(&path).unwrap(), state);
        assert_eq!(state.completed, vec!["apt".to_string()]);
    }

    #[test]
    fn holds_no_secret_fields() {
        let state = State {
            current: Some("/r".into()),
            apple_id: Some("a@b.c".into()),
            login_mode: Some("key".into()),
            key_file: Some("/k/AuthKey_X.p8".into()),
            completed: vec!["x".into()],
            projects: vec![ProjectRecord {
                name: "R".into(),
                root: "/r".into(),
                repository: "https://example.org/r/r".into(),
                branch: Some("b".into()),
                installed: true,
            }],
        };
        let value: toml::Value = toml::from_str(&toml::to_string(&state).unwrap()).unwrap();
        let table = value.as_table().unwrap();
        let mut keys: Vec<&String> = table.keys().collect();
        let project = table["projects"].as_array().unwrap()[0].as_table().unwrap();
        keys.extend(project.keys());
        let allowed = [
            "current",
            "apple_id",
            "login_mode",
            "key_file",
            "completed",
            "projects",
            "name",
            "root",
            "repository",
            "branch",
            "installed",
        ];
        for key in &keys {
            assert!(allowed.contains(&key.as_str()), "unexpected field {key}");
            for secret in ["password", "code", "pem", "token", "secret", "private"] {
                assert!(!key.contains(secret), "{key}");
            }
        }
        // And a state file with a smuggled-in field is refused, not kept.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.toml");
        std::fs::write(&path, "password = \"hunter2\"\n").unwrap();
        let err = State::load(&path).unwrap_err();
        assert!(!err.to_string().contains("hunter2"));
    }

    #[test]
    fn missing_file_is_a_fresh_state() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            State::load(&dir.path().join("none.toml")).unwrap(),
            State::default()
        );
    }
}
