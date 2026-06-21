use std::{fs, io::Read, path::PathBuf};

use sha2::{Digest, Sha256};

use crate::application::{
    collection::{CollectorDescriptor, CollectorFailure, CollectorFailureCode, CollectorIntegrity},
    ports::collector::CancellationSignal,
};

use super::{
    command::prepare_version_check,
    manifest::{BinaryTarget, SidecarManifest},
    process::{execute, ProcessLimits},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SidecarLocation {
    PackagedResourceDirectory(PathBuf),
    DevelopmentBinary(PathBuf),
}

#[derive(Debug)]
pub(crate) struct VerifiedSidecar {
    pub executable: PathBuf,
    pub descriptor: CollectorDescriptor,
}

pub(crate) fn verify(
    manifest: &SidecarManifest,
    target: BinaryTarget,
    location: SidecarLocation,
    cancellation: &dyn CancellationSignal,
) -> Result<VerifiedSidecar, CollectorFailure> {
    let entry = manifest
        .entry_for(target)
        .ok_or_else(|| failure(CollectorFailureCode::BinaryMissing))?;
    let (executable, expected_development) = match location {
        SidecarLocation::PackagedResourceDirectory(directory) => (
            directory
                .join("sidecars")
                .join("ccusage")
                .join(entry.executable_name()),
            false,
        ),
        SidecarLocation::DevelopmentBinary(executable) => (executable, true),
    };

    let metadata =
        fs::metadata(&executable).map_err(|_| failure(CollectorFailureCode::BinaryMissing))?;
    if !metadata.is_file() || entry.integrity().is_development() != expected_development {
        return Err(failure(CollectorFailureCode::BinaryChecksumMismatch));
    }

    let integrity = match entry.integrity().expected_sha256() {
        Some(expected) if sha256(&executable)? == expected => CollectorIntegrity::Verified,
        Some(_) => return Err(failure(CollectorFailureCode::BinaryChecksumMismatch)),
        None => CollectorIntegrity::UnverifiedDevelopment,
    };
    let version_check = prepare_version_check(&executable)?;
    let output = execute(
        version_check.process(),
        cancellation,
        ProcessLimits::version_check(),
    )?;
    let runtime_version = parse_version(&output.stdout)
        .ok_or_else(|| failure(CollectorFailureCode::VersionMismatch))?;
    if runtime_version != manifest.expected_version() {
        return Err(failure(CollectorFailureCode::VersionMismatch));
    }
    let descriptor = manifest
        .descriptor(target, runtime_version.to_owned(), integrity)
        .map_err(|_| failure(CollectorFailureCode::VersionMismatch))?;

    Ok(VerifiedSidecar {
        executable,
        descriptor,
    })
}

fn sha256(path: &std::path::Path) -> Result<String, CollectorFailure> {
    let mut file =
        fs::File::open(path).map_err(|_| failure(CollectorFailureCode::BinaryMissing))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| failure(CollectorFailureCode::BinaryChecksumMismatch))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn parse_version(stdout: &str) -> Option<&str> {
    let mut parts = stdout.split_whitespace();
    let product = parts.next()?;
    let version = parts.next()?;
    if product != "ccusage" || parts.next().is_some() {
        return None;
    }
    Some(version)
}

fn failure(code: CollectorFailureCode) -> CollectorFailure {
    CollectorFailure::new(code, None, None)
}

#[cfg(all(test, unix))]
mod tests {
    use std::{fs, os::unix::fs::symlink};

    use crate::application::ports::collector::CancellationSignal;

    use super::*;

    struct Active;

    impl CancellationSignal for Active {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    #[test]
    fn verifies_explicit_development_binary_without_claiming_release_integrity() {
        let fixture = Fixture::new("20.0.14");
        let manifest = fixture.manifest(r#"{"kind":"unverified_dev"}"#);

        let verified = verify(
            &manifest,
            fixture.target,
            SidecarLocation::DevelopmentBinary(fixture.executable.clone()),
            &Active,
        )
        .expect("verified development sidecar");

        assert_eq!(verified.executable, fixture.executable);
        assert_eq!(
            verified.descriptor.integrity,
            CollectorIntegrity::UnverifiedDevelopment
        );
        assert_eq!(verified.descriptor.runtime_version, "20.0.14");
    }

    #[test]
    fn verifies_packaged_binary_checksum_before_version() {
        let fixture = Fixture::new("20.0.14");
        let (package_root, packaged) = fixture.package();
        let checksum = sha256(&packaged).expect("checksum");
        let manifest = fixture.manifest(&format!(
            r#"{{"kind":"release_sha256","sha256":"{checksum}"}}"#
        ));

        let verified = verify(
            &manifest,
            fixture.target,
            SidecarLocation::PackagedResourceDirectory(package_root.path().to_path_buf()),
            &Active,
        )
        .expect("verified packaged sidecar");

        assert_eq!(verified.descriptor.integrity, CollectorIntegrity::Verified);
    }

    #[test]
    fn rejects_checksum_version_location_policy_and_missing_binary_failures() {
        let fixture = Fixture::new("20.0.10");
        let invalid_checksum = fixture.manifest(
            r#"{"kind":"release_sha256","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        );
        let (package_root, _) = fixture.package();
        assert_code(
            verify(
                &invalid_checksum,
                fixture.target,
                SidecarLocation::PackagedResourceDirectory(package_root.path().to_path_buf()),
                &Active,
            ),
            CollectorFailureCode::BinaryChecksumMismatch,
        );

        let development = fixture.manifest(r#"{"kind":"unverified_dev"}"#);
        assert_code(
            verify(
                &development,
                fixture.target,
                SidecarLocation::DevelopmentBinary(fixture.executable.clone()),
                &Active,
            ),
            CollectorFailureCode::VersionMismatch,
        );
        assert_code(
            verify(
                &development,
                fixture.target,
                SidecarLocation::PackagedResourceDirectory(package_root.path().to_path_buf()),
                &Active,
            ),
            CollectorFailureCode::BinaryChecksumMismatch,
        );
        assert_code(
            verify(
                &development,
                fixture.target,
                SidecarLocation::DevelopmentBinary(fixture.directory.path().join("missing")),
                &Active,
            ),
            CollectorFailureCode::BinaryMissing,
        );
    }

    fn assert_code(
        result: Result<VerifiedSidecar, CollectorFailure>,
        expected: CollectorFailureCode,
    ) {
        assert_eq!(result.expect_err("verification failure").code, expected);
    }

    struct Fixture {
        directory: tempfile::TempDir,
        executable: PathBuf,
        target: BinaryTarget,
    }

    impl Fixture {
        fn new(version: &str) -> Self {
            let target = BinaryTarget::current().expect("supported test target");
            let directory = tempfile::tempdir().expect("fixture directory");
            let fixture = if version == "20.0.14" {
                "fake-collector.sh"
            } else {
                "fake-collector-old.sh"
            };
            let executable = fixture_path(fixture);
            Self {
                directory,
                executable,
                target,
            }
        }

        fn manifest(&self, integrity: &str) -> SidecarManifest {
            SidecarManifest::parse(&format!(
                r#"{{
                    "collectorKey":"ccusage",
                    "displayName":"ccusage",
                    "expectedVersion":"20.0.14",
                    "sourceRevision":"a7726bb9227ef828a8fa06422a08162254a61563",
                    "adapterVersion":1,
                    "entries":[{{
                        "target":"{}",
                        "rustTargetTriple":"{}",
                        "packageName":"{}",
                        "executableName":"{}",
                        "integrity":{integrity}
                    }}]
                }}"#,
                self.target.as_str(),
                self.target.rust_target_triple(),
                self.target.package_name(),
                self.target.executable_name()
            ))
            .expect("sidecar manifest")
        }

        fn package(&self) -> (tempfile::TempDir, PathBuf) {
            let root = tempfile::tempdir().expect("package root");
            let executable = root
                .path()
                .join("sidecars")
                .join("ccusage")
                .join(self.target.executable_name());
            fs::create_dir_all(executable.parent().expect("package parent"))
                .expect("create package directories");
            symlink(&self.executable, &executable).expect("link packaged executable");
            (root, executable)
        }
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("tests/fixtures/collectors/ccusage/process")
            .join(name)
    }
}
