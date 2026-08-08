use serde::Deserialize;
use thiserror::Error;

use crate::application::collection::{
    CollectorDescriptor, CollectorIntegrity, CollectorKey, ProfileDescriptor,
};

use super::capability_profiles::profiles;

const DEVELOPMENT_MANIFEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/sidecars/ccusage/development-manifest.json"
));
const EXPECTED_COLLECTOR_KEY: &str = "ccusage";
const EXPECTED_VERSION: &str = "20.0.19";
const EXPECTED_SOURCE_REVISION: &str = "caf89e8c0291a2acec09e01ff609e6253f6dd81b";
const ADAPTER_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SidecarManifest {
    collector_key: String,
    display_name: String,
    expected_version: String,
    source_revision: String,
    adapter_version: u16,
    entries: Vec<SidecarEntry>,
}

impl SidecarManifest {
    pub(crate) fn parse(json: &str) -> Result<Self, ManifestError> {
        let manifest: Self = serde_json::from_str(json).map_err(ManifestError::InvalidJson)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub(crate) fn entry_for(&self, target: BinaryTarget) -> Option<&SidecarEntry> {
        self.entries.iter().find(|entry| entry.target == target)
    }

    pub(crate) fn expected_version(&self) -> &str {
        &self.expected_version
    }

    pub(crate) fn descriptor(
        &self,
        target: BinaryTarget,
        runtime_version: String,
        integrity: CollectorIntegrity,
    ) -> Result<CollectorDescriptor, ManifestError> {
        let entry = self
            .entry_for(target)
            .ok_or(ManifestError::TargetNotDeclared(target))?;
        if runtime_version != self.expected_version {
            return Err(ManifestError::RuntimeVersionMismatch);
        }
        if !entry.integrity.accepts(integrity) {
            return Err(ManifestError::InvalidIntegrityState);
        }

        Ok(CollectorDescriptor {
            collector: CollectorKey::new(self.collector_key.clone())
                .expect("validated manifest collector key"),
            display_name: self.display_name.clone(),
            runtime_version,
            expected_version: self.expected_version.clone(),
            adapter_version: self.adapter_version,
            binary_target: entry.target.as_str().to_owned(),
            integrity,
            profiles: profiles()
                .iter()
                .map(|profile| ProfileDescriptor {
                    source: profile.source,
                    profile_version: profile.profile_version,
                    supported_projections: profile.supported_projections.to_vec(),
                })
                .collect(),
        })
    }

    fn validate(&self) -> Result<(), ManifestError> {
        if self.collector_key != EXPECTED_COLLECTOR_KEY {
            return Err(ManifestError::UnexpectedCollectorKey);
        }
        if self.display_name.trim().is_empty() {
            return Err(ManifestError::EmptyDisplayName);
        }
        if self.expected_version != EXPECTED_VERSION {
            return Err(ManifestError::UnexpectedVersion);
        }
        if self.source_revision != EXPECTED_SOURCE_REVISION {
            return Err(ManifestError::UnexpectedSourceRevision);
        }
        if self.adapter_version != ADAPTER_VERSION {
            return Err(ManifestError::UnexpectedAdapterVersion);
        }
        if self.entries.is_empty() {
            return Err(ManifestError::NoEntries);
        }

        let mut targets = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            entry.validate()?;
            if targets.contains(&entry.target) {
                return Err(ManifestError::DuplicateTarget(entry.target));
            }
            targets.push(entry.target);
        }

        Ok(())
    }
}

pub(crate) fn development_manifest() -> &'static SidecarManifest {
    static MANIFEST: std::sync::OnceLock<SidecarManifest> = std::sync::OnceLock::new();
    MANIFEST.get_or_init(|| {
        SidecarManifest::parse(DEVELOPMENT_MANIFEST)
            .expect("checked-in ccusage development manifest must be valid")
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SidecarEntry {
    target: BinaryTarget,
    rust_target_triple: String,
    package_name: String,
    executable_name: String,
    integrity: IntegrityPolicy,
}

impl SidecarEntry {
    pub(crate) const fn target(&self) -> BinaryTarget {
        self.target
    }

    pub(crate) fn executable_name(&self) -> &str {
        &self.executable_name
    }

    pub(crate) fn rust_target_triple(&self) -> &str {
        &self.rust_target_triple
    }

    pub(crate) fn package_name(&self) -> &str {
        &self.package_name
    }

    pub(crate) fn integrity(&self) -> &IntegrityPolicy {
        &self.integrity
    }

    fn validate(&self) -> Result<(), ManifestError> {
        if self.executable_name != self.target.executable_name() {
            return Err(ManifestError::UnexpectedExecutableName(self.target));
        }
        if self.rust_target_triple != self.target.rust_target_triple() {
            return Err(ManifestError::UnexpectedTargetTriple(self.target));
        }
        if self.package_name != self.target.package_name() {
            return Err(ManifestError::UnexpectedPackageName(self.target));
        }
        self.integrity.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum IntegrityPolicy {
    ReleaseSha256 { sha256: String },
    UnverifiedDev,
}

impl IntegrityPolicy {
    fn validate(&self) -> Result<(), ManifestError> {
        match self {
            Self::ReleaseSha256 { sha256 } if is_sha256(sha256) => Ok(()),
            Self::ReleaseSha256 { .. } => Err(ManifestError::InvalidSha256),
            Self::UnverifiedDev => Ok(()),
        }
    }

    const fn accepts(&self, state: CollectorIntegrity) -> bool {
        match self {
            Self::ReleaseSha256 { .. } => matches!(
                state,
                CollectorIntegrity::Verified | CollectorIntegrity::Mismatch
            ),
            Self::UnverifiedDev => {
                matches!(state, CollectorIntegrity::UnverifiedDevelopment)
            }
        }
    }

    pub(crate) fn expected_sha256(&self) -> Option<&str> {
        match self {
            Self::ReleaseSha256 { sha256 } => Some(sha256),
            Self::UnverifiedDev => None,
        }
    }

    pub(crate) const fn is_development(&self) -> bool {
        matches!(self, Self::UnverifiedDev)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum BinaryTarget {
    DarwinArm64,
    DarwinX64,
    LinuxArm64,
    LinuxX64,
    WindowsArm64,
    WindowsX64,
}

impl BinaryTarget {
    pub(crate) const ALL: [Self; 6] = [
        Self::DarwinArm64,
        Self::DarwinX64,
        Self::LinuxArm64,
        Self::LinuxX64,
        Self::WindowsArm64,
        Self::WindowsX64,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::DarwinArm64 => "darwin-arm64",
            Self::DarwinX64 => "darwin-x64",
            Self::LinuxArm64 => "linux-arm64",
            Self::LinuxX64 => "linux-x64",
            Self::WindowsArm64 => "windows-arm64",
            Self::WindowsX64 => "windows-x64",
        }
    }

    pub(crate) const fn package_name(self) -> &'static str {
        match self {
            Self::DarwinArm64 => "@ccusage/ccusage-darwin-arm64",
            Self::DarwinX64 => "@ccusage/ccusage-darwin-x64",
            Self::LinuxArm64 => "@ccusage/ccusage-linux-arm64",
            Self::LinuxX64 => "@ccusage/ccusage-linux-x64",
            Self::WindowsArm64 => "@ccusage/ccusage-win32-arm64",
            Self::WindowsX64 => "@ccusage/ccusage-win32-x64",
        }
    }

    pub(crate) const fn rust_target_triple(self) -> &'static str {
        match self {
            Self::DarwinArm64 => "aarch64-apple-darwin",
            Self::DarwinX64 => "x86_64-apple-darwin",
            Self::LinuxArm64 => "aarch64-unknown-linux-gnu",
            Self::LinuxX64 => "x86_64-unknown-linux-gnu",
            Self::WindowsArm64 => "aarch64-pc-windows-msvc",
            Self::WindowsX64 => "x86_64-pc-windows-msvc",
        }
    }

    pub(crate) const fn executable_name(self) -> &'static str {
        match self {
            Self::WindowsArm64 | Self::WindowsX64 => "ccusage.exe",
            _ => "ccusage",
        }
    }

    pub(crate) const fn current() -> Option<Self> {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(Self::DarwinArm64)
        } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            Some(Self::DarwinX64)
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            Some(Self::LinuxArm64)
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Self::LinuxX64)
        } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
            Some(Self::WindowsArm64)
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Some(Self::WindowsX64)
        } else {
            None
        }
    }

    pub(crate) fn from_rust_target_triple(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|target| target.rust_target_triple() == value)
    }
}

#[derive(Debug, Error)]
pub(crate) enum ManifestError {
    #[error("ccusage manifest is invalid JSON")]
    InvalidJson(#[source] serde_json::Error),
    #[error("ccusage manifest has an unexpected collector key")]
    UnexpectedCollectorKey,
    #[error("ccusage manifest display name must not be empty")]
    EmptyDisplayName,
    #[error("ccusage manifest has an unexpected version")]
    UnexpectedVersion,
    #[error("ccusage manifest has an unexpected source revision")]
    UnexpectedSourceRevision,
    #[error("ccusage manifest has an unexpected adapter version")]
    UnexpectedAdapterVersion,
    #[error("ccusage manifest must declare at least one entry")]
    NoEntries,
    #[error("ccusage manifest declares target {0:?} more than once")]
    DuplicateTarget(BinaryTarget),
    #[error("ccusage manifest does not declare target {0:?}")]
    TargetNotDeclared(BinaryTarget),
    #[error("ccusage runtime version does not match the pinned manifest version")]
    RuntimeVersionMismatch,
    #[error("ccusage runtime integrity state is incompatible with the manifest policy")]
    InvalidIntegrityState,
    #[error("ccusage manifest has an unexpected executable name for {0:?}")]
    UnexpectedExecutableName(BinaryTarget),
    #[error("ccusage manifest has an unexpected Rust target triple for {0:?}")]
    UnexpectedTargetTriple(BinaryTarget),
    #[error("ccusage manifest has an unexpected package name for {0:?}")]
    UnexpectedPackageName(BinaryTarget),
    #[error("ccusage release checksum must be 64 lowercase hexadecimal characters")]
    InvalidSha256,
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_checked_in_development_manifest_with_explicit_unverified_state() {
        let manifest = development_manifest();
        for target in BinaryTarget::ALL {
            assert_eq!(
                manifest
                    .entry_for(target)
                    .expect("supported development target")
                    .integrity(),
                &IntegrityPolicy::UnverifiedDev
            );
        }
        let entry = manifest
            .entry_for(BinaryTarget::LinuxX64)
            .expect("linux x64 development entry");

        assert_eq!(entry.executable_name(), "ccusage");
        assert_eq!(entry.rust_target_triple(), "x86_64-unknown-linux-gnu");
        assert_eq!(entry.package_name(), "@ccusage/ccusage-linux-x64");
        assert_eq!(entry.integrity(), &IntegrityPolicy::UnverifiedDev);
        assert_eq!(
            manifest
                .descriptor(
                    BinaryTarget::LinuxX64,
                    EXPECTED_VERSION.to_owned(),
                    CollectorIntegrity::UnverifiedDevelopment,
                )
                .expect("collector descriptor")
                .integrity,
            CollectorIntegrity::UnverifiedDevelopment
        );
    }

    #[test]
    fn development_manifest_rejects_verified_integrity() {
        assert!(matches!(
            development_manifest().descriptor(
                BinaryTarget::LinuxX64,
                EXPECTED_VERSION.to_owned(),
                CollectorIntegrity::Verified,
            ),
            Err(ManifestError::InvalidIntegrityState)
        ));
    }

    #[test]
    fn descriptor_requires_the_pinned_runtime_version() {
        assert!(matches!(
            development_manifest().descriptor(
                BinaryTarget::LinuxX64,
                "20.0.10".to_owned(),
                CollectorIntegrity::UnverifiedDevelopment,
            ),
            Err(ManifestError::RuntimeVersionMismatch)
        ));
    }

    #[test]
    fn release_manifest_accepts_observed_verification_results() {
        let release = manifest_json(
            r#"{"kind":"release_sha256","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        );
        let manifest = SidecarManifest::parse(&release).expect("release manifest");

        for integrity in [CollectorIntegrity::Verified, CollectorIntegrity::Mismatch] {
            assert_eq!(
                manifest
                    .descriptor(
                        BinaryTarget::LinuxX64,
                        EXPECTED_VERSION.to_owned(),
                        integrity,
                    )
                    .expect("collector descriptor")
                    .integrity,
                integrity
            );
        }
    }

    #[test]
    fn target_policy_matches_all_reviewed_native_packages() {
        let packages = BinaryTarget::ALL.map(|target| target.package_name());

        assert_eq!(
            packages,
            [
                "@ccusage/ccusage-darwin-arm64",
                "@ccusage/ccusage-darwin-x64",
                "@ccusage/ccusage-linux-arm64",
                "@ccusage/ccusage-linux-x64",
                "@ccusage/ccusage-win32-arm64",
                "@ccusage/ccusage-win32-x64",
            ]
        );
        for target in BinaryTarget::ALL {
            assert_eq!(
                BinaryTarget::from_rust_target_triple(target.rust_target_triple()),
                Some(target)
            );
        }
        assert_eq!(
            BinaryTarget::from_rust_target_triple("wasm32-unknown-unknown"),
            None
        );
    }

    #[test]
    fn checked_in_release_manifest_is_complete_and_verified() {
        let manifest = SidecarManifest::parse(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/sidecars/ccusage/release-manifest.json"
        )))
        .expect("release manifest");

        for target in BinaryTarget::ALL {
            let entry = manifest.entry_for(target).expect("release target entry");
            assert_eq!(entry.rust_target_triple(), target.rust_target_triple());
            assert_eq!(entry.package_name(), target.package_name());
            assert!(matches!(
                entry.integrity(),
                IntegrityPolicy::ReleaseSha256 { .. }
            ));
        }
    }

    #[test]
    fn release_entry_requires_valid_sha256() {
        let invalid = manifest_json(r#"{"kind":"release_sha256","sha256":"not-a-checksum"}"#);

        assert!(matches!(
            SidecarManifest::parse(&invalid),
            Err(ManifestError::InvalidSha256)
        ));
    }

    #[test]
    fn rejects_unknown_target_and_malformed_manifest() {
        let linux_only_manifest =
            SidecarManifest::parse(&manifest_json(r#"{"kind":"unverified_dev"}"#))
                .expect("linux-only manifest");

        assert!(matches!(
            linux_only_manifest.descriptor(
                BinaryTarget::DarwinArm64,
                "20.0.19".to_owned(),
                CollectorIntegrity::UnverifiedDevelopment,
            ),
            Err(ManifestError::TargetNotDeclared(BinaryTarget::DarwinArm64))
        ));
        assert!(matches!(
            SidecarManifest::parse("{}"),
            Err(ManifestError::InvalidJson(_))
        ));
    }

    fn manifest_json(integrity: &str) -> String {
        format!(
            r#"{{
                "collectorKey":"ccusage",
                "displayName":"ccusage",
                "expectedVersion":"20.0.19",
                "sourceRevision":"caf89e8c0291a2acec09e01ff609e6253f6dd81b",
                "adapterVersion":1,
                "entries":[{{
                    "target":"linux-x64",
                    "rustTargetTriple":"x86_64-unknown-linux-gnu",
                    "packageName":"@ccusage/ccusage-linux-x64",
                    "executableName":"ccusage",
                    "integrity":{integrity}
                }}]
            }}"#
        )
    }
}
