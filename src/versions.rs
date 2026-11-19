//! Every pinned version, checksum and download location, in one place.
//!
//! Updating a pin is a reviewed change to this file; docs/maintaining.md
//! ("Updating the pins") lists how each value was obtained.

/// pippwin's own repository, which the Windows and WSL bootstrap
/// scripts clone and build.
pub const PIPPWIN_REPO_URL: &str = "https://github.com/buchochelliq-labs/pippwin";

/// The name of the project file at the root of a project's repository.
pub const PROJECT_FILE: &str = "pippwin.toml";
/// Its reference, on pippwin's documentation site.
pub const PROJECT_FILE_DOCS: &str = "https://buchochelliq-labs.github.io/pippwin/pippwin-toml/";

/// rustup-init, when rustup is missing. `.sha256` files are published next
/// to each binary under static.rust-lang.org/rustup/archive/<version>/.
/// The project's own `rust-toolchain.toml` then picks the Rust toolchain.
pub const RUSTUP_VERSION: &str = "1.29.3";
pub const RUSTUP_SHA256_X86_64: &str =
    "b1c8cb14e30889e7e2999e6b69e62d9e9b9ffc0f4df8180564d306d9680e6c55";
pub const RUSTUP_SHA256_AARCH64: &str =
    "9f608c142e0e38a78957d0661ba1dfaf0b6fb37184cfaa4b9a0aa57af6184282";

/// A Node.js release pippwin installs for a project's `needs.node` major,
/// from the official tarball, checked against the pinned SHA-256 and the
/// release's own SHASUMS256.txt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeRelease {
    pub major: u32,
    pub version: &'static str,
    pub sha256_x64: &'static str,
    pub sha256_arm64: &'static str,
}

/// Every Node.js major pippwin supports, each mapped to one pinned release.
pub const NODE_RELEASES: &[NodeRelease] = &[NodeRelease {
    major: 22,
    version: "22.23.5",
    sha256_x64: "cdeb8670a0c81f51b4dc4ac61919f9b1ad917b304bf8221eb939570663d240f8",
    sha256_arm64: "2babb9eb5264860d540849f4b064a6fbdebcdc323657d9d382ce23d313fd5e09",
}];

/// The pinned release for Node.js `major`.
pub fn node_release(major: u32) -> Option<&'static NodeRelease> {
    NODE_RELEASES.iter().find(|r| r.major == major)
}

/// The supported majors, for messages: `22` or `20, 22`.
pub fn node_majors() -> String {
    NODE_RELEASES
        .iter()
        .map(|r| r.major.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The Swift toolchain xtool's Linux guide names, installed with swiftly.
pub const SWIFT_VERSION: &str = "6.4";
/// swiftly, swift.org's installer. The tarball is checked against its
/// detached signature with swift.org's keys before it runs.
pub const SWIFTLY_BASE_URL: &str = "https://download.swift.org/swiftly/linux";
pub const SWIFT_KEYS_URL: &str = "https://www.swift.org/keys/all-keys.asc";
/// Primary-key fingerprints that may sign swiftly: the "Swift 6.x Release
/// Signing Key" and the "Swift Automatic Signing Key #4", as published in
/// all-keys.asc. A signature by any other key is refused.
pub const SWIFT_SIGNING_FINGERPRINTS: &[&str] = &[
    "52BB7E3DE28A71BE22EC05FFEF80A866B47A981F",
    "E813C892820A6FA13755B268F167DF1ACF9CE069",
];

/// xtool's release. xtool publishes no checksum file, so the AppImage is
/// checked against the SHA-256 GitHub records for the release asset (the
/// `digest` of the asset in the releases API), and against the pin below
/// when one is recorded.
pub const XTOOL_VERSION: &str = "1.24.0";
/// SHA-256 of `xtool-<arch>.AppImage` for [`XTOOL_VERSION`]. `None` until
/// someone with access to the release records it (docs/maintaining.md,
/// "Updating the pins"); until then the GitHub asset digest is the
/// reference, and xtool is not installed when that cannot be read.
pub const XTOOL_SHA256_X86_64: Option<&str> = None;
pub const XTOOL_SHA256_AARCH64: Option<&str> = None;
/// The Darwin SDK version string `xtool sdk status` prints for an SDK
/// that xtool [`XTOOL_VERSION`] considers up to date
/// (`SDKBuilder.currentSDKVersion`).
pub const XTOOL_SDK_VERSION: &str = "epoch=4,darwinTools=1.1.0,oam=1.4.1";
/// The Xcode release xtool's Linux guide names.
pub const XCODE_VERSION: &str = "27.1";
pub const XCODE_DOWNLOAD_URL: &str = "https://developer.apple.com/download/all/?q=Xcode";

/// Links from xtool's Linux guide.
pub const XTOOL_LINUX_GUIDE: &str =
    "https://github.com/xtool-org/xtool/blob/main/Documentation/xtool.docc/Installation-Linux.md";
pub const MUX_ERROR_WORKAROUND: &str =
    "https://github.com/xtool-org/xtool/issues/19#issuecomment-2898986718";
pub const USBIPD_GUIDE: &str = "https://learn.microsoft.com/en-us/windows/wsl/connect-usb";
pub const ASC_KEYS_GUIDE: &str = "https://developer.apple.com/documentation/appstoreconnectapi/creating-api-keys-for-app-store-connect-api";

/// The machine architecture, as the downloads name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
}

impl Arch {
    pub fn current() -> Option<Arch> {
        match std::env::consts::ARCH {
            "x86_64" => Some(Arch::X86_64),
            "aarch64" => Some(Arch::Aarch64),
            _ => None,
        }
    }

    /// `uname -m`.
    pub fn uname(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        }
    }

    pub fn node(self) -> &'static str {
        match self {
            Arch::X86_64 => "x64",
            Arch::Aarch64 => "arm64",
        }
    }

    pub fn node_sha256(self, release: &NodeRelease) -> &'static str {
        match self {
            Arch::X86_64 => release.sha256_x64,
            Arch::Aarch64 => release.sha256_arm64,
        }
    }

    pub fn rustup_sha256(self) -> &'static str {
        match self {
            Arch::X86_64 => RUSTUP_SHA256_X86_64,
            Arch::Aarch64 => RUSTUP_SHA256_AARCH64,
        }
    }

    pub fn xtool_sha256(self) -> Option<&'static str> {
        match self {
            Arch::X86_64 => XTOOL_SHA256_X86_64,
            Arch::Aarch64 => XTOOL_SHA256_AARCH64,
        }
    }
}

pub fn node_tarball(release: &NodeRelease, arch: Arch) -> String {
    format!("node-v{}-linux-{}.tar.xz", release.version, arch.node())
}

pub fn node_url(release: &NodeRelease, file: &str) -> String {
    format!("https://nodejs.org/dist/v{}/{file}", release.version)
}

pub fn rustup_url(arch: Arch) -> String {
    format!(
        "https://static.rust-lang.org/rustup/archive/{RUSTUP_VERSION}/{}-unknown-linux-gnu/rustup-init",
        arch.uname()
    )
}

pub fn xtool_asset(arch: Arch) -> String {
    format!("xtool-{}.AppImage", arch.uname())
}

pub fn xtool_url(arch: Arch) -> String {
    format!(
        "https://github.com/xtool-org/xtool/releases/download/{XTOOL_VERSION}/{}",
        xtool_asset(arch)
    )
}

pub fn xtool_release_api() -> String {
    format!("https://api.github.com/repos/xtool-org/xtool/releases/tags/{XTOOL_VERSION}")
}

pub fn swiftly_url(arch: Arch) -> String {
    format!("{SWIFTLY_BASE_URL}/swiftly-{}.tar.gz", arch.uname())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_majors_map_to_one_pinned_release_each() {
        let release = node_release(22).unwrap();
        assert_eq!(release.version, "22.23.5");
        assert!(release.version.starts_with("22."));
        assert!(node_release(21).is_none());
        assert_eq!(node_majors(), "22");
        let mut majors: Vec<u32> = NODE_RELEASES.iter().map(|r| r.major).collect();
        majors.dedup();
        assert_eq!(majors.len(), NODE_RELEASES.len());
    }

    #[test]
    fn bootstrap_scripts_clone_this_repository_and_read_these_pins() {
        for script in ["scripts/bootstrap.sh", "scripts/setup-windows.ps1"] {
            let path = format!("{}/{script}", env!("CARGO_MANIFEST_DIR"));
            let text = std::fs::read_to_string(path).unwrap();
            assert!(text.contains(PIPPWIN_REPO_URL), "{script}");
            // Only the sentence that says it is never used.
            assert_eq!(
                text.matches("Invoke-Expression").count(),
                text.matches("no Invoke-Expression").count(),
                "{script}"
            );
            assert!(!text.contains("iex "), "{script}");
        }
        let bootstrap =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/bootstrap.sh"))
                .unwrap();
        for name in [
            "RUSTUP_VERSION",
            "RUSTUP_SHA256_X86_64",
            "RUSTUP_SHA256_AARCH64",
        ] {
            assert!(bootstrap.contains(name), "{name}");
        }
        // The forms bootstrap.sh's sed and grep read.
        let source =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/versions.rs"))
                .unwrap();
        assert!(source.contains(&format!(
            "pub const RUSTUP_VERSION: &str = \"{RUSTUP_VERSION}\";"
        )));
        assert!(source.contains(&format!(
            "pub const RUSTUP_SHA256_X86_64: &str =\n    \"{RUSTUP_SHA256_X86_64}\""
        )));
    }

    #[test]
    fn own_toolchain_is_pinned() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/rust-toolchain.toml");
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("channel = \"1.95.0\""));
    }

    #[test]
    fn checksums_are_hex_sha256() {
        let mut sums = vec![RUSTUP_SHA256_X86_64, RUSTUP_SHA256_AARCH64];
        for release in NODE_RELEASES {
            sums.extend([release.sha256_x64, release.sha256_arm64]);
        }
        for sum in sums {
            assert_eq!(sum.len(), 64);
            assert!(sum.bytes().all(|b| b.is_ascii_hexdigit()));
        }
        for fpr in SWIFT_SIGNING_FINGERPRINTS {
            assert_eq!(fpr.len(), 40);
        }
    }

    #[test]
    fn urls_are_https_and_pinned() {
        for arch in [Arch::X86_64, Arch::Aarch64] {
            let node = &NODE_RELEASES[0];
            for url in [
                node_url(node, &node_tarball(node, arch)),
                rustup_url(arch),
                xtool_url(arch),
                swiftly_url(arch),
            ] {
                assert!(url.starts_with("https://"), "{url}");
            }
            assert!(xtool_url(arch).contains(XTOOL_VERSION));
            assert!(node_url(node, &node_tarball(node, arch)).contains(node.version));
            assert!(rustup_url(arch).contains(RUSTUP_VERSION));
        }
    }
}
