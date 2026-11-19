//! Checks of what the user types and of the key file. None of these
//! messages quote a key's contents.

/// An App Store Connect key ID: ten uppercase letters or digits.
pub fn key_id(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() == 10
        && value
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        Ok(value.to_string())
    } else {
        Err(
            "A Key ID is 10 characters, uppercase letters and digits (for example 2X9R4HXF34)."
                .into(),
        )
    }
}

/// An App Store Connect issuer ID: a UUID (8-4-4-4-12 hexadecimal digits).
pub fn issuer_id(value: &str) -> Result<String, String> {
    let value = value.trim();
    let groups: Vec<&str> = value.split('-').collect();
    let lengths = [8, 4, 4, 4, 12];
    let ok = groups.len() == lengths.len()
        && groups
            .iter()
            .zip(lengths)
            .all(|(group, len)| group.len() == len && group.bytes().all(|b| b.is_ascii_hexdigit()));
    if ok {
        Ok(value.to_ascii_lowercase())
    } else {
        Err("An Issuer ID is a UUID, like 57246542-96fe-1a63-e053-0824d011072a.".into())
    }
}

/// An Apple ID as xtool takes it: an email address, or a phone number in
/// international form. Only a sanity check; Apple decides.
pub fn apple_id(value: &str) -> Result<String, String> {
    let value = value.trim();
    let email = value.split_once('@').is_some_and(|(user, domain)| {
        !user.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    });
    let phone = value.starts_with('+')
        && value.len() > 6
        && value[1..].bytes().all(|b| b.is_ascii_digit() || b == b' ');
    if (email && !value.chars().any(char::is_whitespace)) || phone {
        Ok(value.to_string())
    } else {
        Err("Enter the email address (or +phone number) you sign in to Apple with.".into())
    }
}

/// Why a file is not a PEM private key. Each says what is wrong, never what
/// the file holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PemProblem {
    TooLarge,
    NotText,
    NoPrivateKeyHeader,
    NoFooter,
    BadBody,
    Empty,
}

impl PemProblem {
    pub fn describe(&self) -> &'static str {
        match self {
            PemProblem::TooLarge => "the file is too large to be an API key",
            PemProblem::NotText => "the file is not text",
            PemProblem::NoPrivateKeyHeader => {
                "the file does not start with a PEM private key header (-----BEGIN PRIVATE KEY-----)"
            }
            PemProblem::NoFooter => "the PEM block is not closed",
            PemProblem::BadBody => "the PEM block holds characters that are not base64",
            PemProblem::Empty => "the PEM block is empty",
        }
    }
}

const MAX_KEY_BYTES: usize = 16 * 1024;

/// Whether `bytes` look like an App Store Connect `.p8` key: one PEM
/// `PRIVATE KEY` (or `EC PRIVATE KEY`) block with a base64 body. xtool
/// itself then checks it is a P-256 key.
pub fn sniff_private_key_pem(bytes: &[u8]) -> Result<(), PemProblem> {
    if bytes.len() > MAX_KEY_BYTES {
        return Err(PemProblem::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| PemProblem::NotText)?;
    let text = text.trim_start_matches('\u{feff}').trim();
    let kinds = ["PRIVATE KEY", "EC PRIVATE KEY"];
    let Some(kind) = kinds
        .iter()
        .find(|kind| text.starts_with(&format!("-----BEGIN {kind}-----")))
    else {
        return Err(PemProblem::NoPrivateKeyHeader);
    };
    let header = format!("-----BEGIN {kind}-----");
    let footer = format!("-----END {kind}-----");
    if !text.ends_with(&footer) {
        return Err(PemProblem::NoFooter);
    }
    let body = &text[header.len()..text.len() - footer.len()];
    let mut count = 0usize;
    for b in body.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'=' => count += 1,
            b'\r' | b'\n' | b' ' | b'\t' => {}
            _ => return Err(PemProblem::BadBody),
        }
    }
    if count < 16 {
        return Err(PemProblem::Empty);
    }
    Ok(())
}

/// A USB bus ID as usbipd names it (`1-4`, `2-1.3`): checked before it is
/// put in a command line.
pub fn bus_id(value: &str) -> bool {
    let Some((bus, port)) = value.split_once('-') else {
        return false;
    };
    !bus.is_empty()
        && bus.bytes().all(|b| b.is_ascii_digit())
        && !port.is_empty()
        && port
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// A git branch name safe to pass to git (no option-like or odd names).
pub fn branch(value: &str) -> Result<String, String> {
    let value = value.trim();
    let ok = !value.is_empty()
        && !value.starts_with('-')
        && !value.contains("..")
        && !value.ends_with('/')
        && !value.ends_with(".lock")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'));
    if ok {
        Ok(value.to_string())
    } else {
        Err("Enter a branch name such as main.".into())
    }
}

/// A repository URL pippwin may clone: `https://host/path`, with no
/// credentials, query or fragment, and only plain characters.
pub fn https_git_url(value: &str) -> Result<String, String> {
    let value = value.trim();
    let problem = || {
        Err::<String, String>(
            "Enter an https git URL such as https://github.com/owner/repo (no user name or token in it)."
                .into(),
        )
    };
    let Some(rest) = value.strip_prefix("https://") else {
        return problem();
    };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host_ok = !host.is_empty()
        && !host.starts_with(['.', '-', ':'])
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':'));
    let path = path.trim_end_matches('/');
    let path_ok = !path.is_empty()
        && path.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~'))
        });
    if host_ok && path_ok {
        Ok(format!("https://{host}/{path}"))
    } else {
        problem()
    }
}

/// A git remote as an https URL, for comparing origins: an https URL
/// as it is, `git@host:owner/repo` and `ssh://git@host/owner/repo` turned
/// into `https://host/owner/repo`.
pub fn remote_as_https(remote: &str) -> Option<String> {
    let remote = remote.trim();
    if remote.starts_with("https://") {
        return https_git_url(remote).ok();
    }
    let rest = remote.strip_prefix("ssh://").unwrap_or(remote);
    let (user_host, path) = if remote.starts_with("ssh://") {
        rest.split_once('/')?
    } else {
        rest.split_once(':')?
    };
    let host = user_host.rsplit('@').next()?;
    https_git_url(&format!("https://{host}/{path}")).ok()
}

/// Two repository URLs naming the same repository: host case and a
/// trailing `.git` or `/` do not matter.
pub fn same_repository(a: &str, b: &str) -> bool {
    let key = |url: &str| {
        let url = remote_as_https(url)?;
        let rest = url.strip_prefix("https://")?;
        let (host, path) = rest.split_once('/')?;
        Some(format!(
            "{}/{}",
            host.to_ascii_lowercase(),
            path.trim_end_matches(".git")
        ))
    };
    matches!((key(a), key(b)), (Some(a), Some(b)) if a == b)
}

/// The last path segment of a repository URL, without `.git`: the default
/// folder name for its clone.
pub fn repository_name(url: &str) -> Option<String> {
    let url = https_git_url(url).ok()?;
    let last = url.rsplit('/').next()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    (!name.is_empty() && !name.starts_with('.')).then(|| name.to_string())
}

/// An Ubuntu package name (Debian policy: lowercase letters, digits, `+`,
/// `-` and `.`, at least two characters, starting with a letter or digit).
pub fn apt_package(value: &str) -> bool {
    value.len() >= 2
        && value.len() <= 128
        && value
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && value.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.')
        })
}

/// A Rust target triple such as `aarch64-apple-ios`.
pub fn rust_target(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('-')
        && value.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.')
        })
}

/// A plain release number such as `9.12.0`.
pub fn release_version(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 6 && p.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fake key: a PEM block of random base64. Never a real key.
    pub(crate) fn fake_pem() -> String {
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut body = String::new();
        for i in 0..184 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            body.push(alphabet[(state % 64) as usize] as char);
            if i % 64 == 63 {
                body.push('\n');
            }
        }
        format!("-----BEGIN PRIVATE KEY-----\n{body}\n-----END PRIVATE KEY-----\n")
    }

    #[test]
    fn key_ids() {
        assert_eq!(key_id(" 2X9R4HXF34 ").unwrap(), "2X9R4HXF34");
        for bad in ["2x9r4hxf34", "2X9R4HXF3", "2X9R4HXF345", "2X9R4-XF34", ""] {
            assert!(key_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn issuer_ids() {
        assert_eq!(
            issuer_id("57246542-96FE-1A63-E053-0824D011072A").unwrap(),
            "57246542-96fe-1a63-e053-0824d011072a"
        );
        for bad in [
            "57246542-96fe-1a63-e053-0824d011072",
            "57246542-96fe-1a63-e053",
            "5724654296fe1a63e0530824d011072a",
            "zz246542-96fe-1a63-e053-0824d011072a",
        ] {
            assert!(issuer_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn apple_ids() {
        assert!(apple_id("me@example.com").is_ok());
        assert!(apple_id("+44 7700 900123").is_ok());
        assert!(apple_id("me").is_err());
        assert!(apple_id("me@localhost").is_err());
        assert!(apple_id("m e@example.com").is_err());
    }

    #[test]
    fn pem_sniffing() {
        let pem = fake_pem();
        assert_eq!(sniff_private_key_pem(pem.as_bytes()), Ok(()));
        let ec = pem.replace("PRIVATE KEY", "EC PRIVATE KEY");
        assert_eq!(sniff_private_key_pem(ec.as_bytes()), Ok(()));
        let crlf = pem.replace('\n', "\r\n");
        assert_eq!(sniff_private_key_pem(crlf.as_bytes()), Ok(()));
        assert_eq!(
            sniff_private_key_pem(pem.replace("PRIVATE KEY", "PUBLIC KEY").as_bytes()),
            Err(PemProblem::NoPrivateKeyHeader)
        );
        assert_eq!(
            sniff_private_key_pem(b"-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----"),
            Err(PemProblem::NoPrivateKeyHeader)
        );
        let open = pem.replace("-----END PRIVATE KEY-----", "");
        assert_eq!(
            sniff_private_key_pem(open.as_bytes()),
            Err(PemProblem::NoFooter)
        );
        let mut lines: Vec<String> = pem.lines().map(str::to_string).collect();
        lines[1].replace_range(0..1, "*");
        let bad = lines.join("\n");
        assert_eq!(
            sniff_private_key_pem(bad.as_bytes()),
            Err(PemProblem::BadBody)
        );
        assert_eq!(
            sniff_private_key_pem(b"-----BEGIN PRIVATE KEY-----\n-----END PRIVATE KEY-----"),
            Err(PemProblem::Empty)
        );
        assert_eq!(
            sniff_private_key_pem(&[0xff, 0xfe, 0x00]),
            Err(PemProblem::NotText)
        );
        assert_eq!(
            sniff_private_key_pem(&vec![b'A'; MAX_KEY_BYTES + 1]),
            Err(PemProblem::TooLarge)
        );
    }

    #[test]
    fn pem_problems_never_quote_the_file() {
        let pem = fake_pem();
        let body_line = pem.lines().nth(1).unwrap();
        for problem in [
            PemProblem::TooLarge,
            PemProblem::NotText,
            PemProblem::NoPrivateKeyHeader,
            PemProblem::NoFooter,
            PemProblem::BadBody,
            PemProblem::Empty,
        ] {
            assert!(!problem.describe().contains(body_line));
        }
    }

    #[test]
    fn bus_ids() {
        assert!(bus_id("1-4"));
        assert!(bus_id("2-1.3"));
        for bad in ["", "1", "1-", "-1", "1-4;calc", "a-1", "1-1..2"] {
            assert!(!bus_id(bad), "{bad}");
        }
    }

    #[test]
    fn https_git_urls() {
        assert_eq!(
            https_git_url("https://github.com/buchochelliq-labs/pippwin/").unwrap(),
            "https://github.com/buchochelliq-labs/pippwin"
        );
        assert!(https_git_url("https://git.example.org:8443/a/b.git").is_ok());
        for bad in [
            "http://github.com/a/b",
            "git@github.com:a/b.git",
            "https://user:token@github.com/a/b",
            "https://github.com",
            "https://github.com/a/../b",
            "https://github.com/a/b?x=1",
            "https://github.com/a b",
            "https://-x/a",
            "--upload-pack=x",
        ] {
            assert!(https_git_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn remotes_and_repository_names() {
        let https = "https://github.com/example-org/my-app";
        for same in [
            "https://github.com/example-org/my-app.git",
            "https://GitHub.com/example-org/my-app/",
            "git@github.com:example-org/my-app.git",
            "ssh://git@github.com/example-org/my-app",
        ] {
            assert!(same_repository(https, same), "{same}");
        }
        assert!(!same_repository(https, "https://github.com/someone/my-app"));
        assert!(!same_repository(https, "not a url"));
        assert_eq!(repository_name(https).as_deref(), Some("my-app"));
        assert_eq!(
            repository_name("https://example.org/x/app.git").as_deref(),
            Some("app")
        );
    }

    #[test]
    fn package_targets_and_versions() {
        for good in [
            "libssl-dev",
            "g++",
            "libstdc++-13-dev",
            "python3.12",
            "7zip",
        ] {
            assert!(apt_package(good), "{good}");
        }
        for bad in [
            "", "x", "-y", "Foo", "a b", "a;b", "pkg=1.0", "a/b", "$(id)",
        ] {
            assert!(!apt_package(bad), "{bad}");
        }
        assert!(rust_target("aarch64-apple-ios"));
        assert!(!rust_target("--help") && !rust_target("x y"));
        assert!(release_version("9.12.0"));
        for bad in ["9", "9.12", "latest", "9.12.0-beta", "v9.12.0"] {
            assert!(!release_version(bad), "{bad}");
        }
    }

    #[test]
    fn branches() {
        assert!(branch("feature/ios-build").is_ok());
        assert!(branch("main").is_ok());
        for bad in ["-x", "a..b", "a b", "x.lock", "", "a;b"] {
            assert!(branch(bad).is_err(), "{bad}");
        }
    }
}
