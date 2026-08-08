use std::{fs, io::Read, path::PathBuf};

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::application::{
    collection::{CollectorDescriptor, CollectorFailure, CollectorFailureCode, CollectorIntegrity},
    ports::collector::CancellationSignal,
};

use super::{
    command::prepare_version_check,
    manifest::{BinaryTarget, SidecarEntry, SidecarManifest},
    process::{execute, ProcessLimits},
};

const PACKAGED_PAYLOAD_HEADER: &[u8] = b"BURNLY-CCUSAGE-PAYLOAD-V1\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SidecarLocation {
    PackagedResourceDirectory(PathBuf),
    DevelopmentBinary(PathBuf),
}

#[derive(Debug)]
pub(crate) struct VerifiedSidecar {
    pub executable: PathBuf,
    pub descriptor: CollectorDescriptor,
    _materialized_directory: Option<TempDir>,
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
    let (executable, expected_development, packaged_directory) = match location {
        SidecarLocation::PackagedResourceDirectory(directory) => {
            let sidecar_directory = directory.join("sidecars").join("ccusage");
            (
                sidecar_directory.join(entry.executable_name()),
                false,
                Some(sidecar_directory),
            )
        }
        SidecarLocation::DevelopmentBinary(executable) => (executable, true, None),
    };

    if entry.integrity().is_development() != expected_development {
        return Err(failure(CollectorFailureCode::BinaryChecksumMismatch));
    }

    let (executable, materialized_directory, integrity) = match entry.integrity().expected_sha256()
    {
        Some(expected) => {
            verified_release_executable(executable, packaged_directory.as_deref(), entry, expected)?
        }
        None => {
            let metadata = fs::metadata(&executable)
                .map_err(|_| failure(CollectorFailureCode::BinaryMissing))?;
            if !metadata.is_file() {
                return Err(failure(CollectorFailureCode::BinaryMissing));
            }
            (executable, None, CollectorIntegrity::UnverifiedDevelopment)
        }
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
        _materialized_directory: materialized_directory,
    })
}

fn verified_release_executable(
    executable: PathBuf,
    packaged_directory: Option<&std::path::Path>,
    entry: &SidecarEntry,
    expected_sha256: &str,
) -> Result<(PathBuf, Option<TempDir>, CollectorIntegrity), CollectorFailure> {
    let direct_binary_is_file = fs::metadata(&executable)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false);
    if direct_binary_is_file {
        if sha256(&executable)? == expected_sha256 {
            return Ok((executable, None, CollectorIntegrity::Verified));
        }
        let payload = packaged_directory
            .map(|directory| directory.join(format!("{}.payload", entry.executable_name())));
        if !payload.as_ref().is_some_and(|path| path.is_file()) {
            return Err(failure(CollectorFailureCode::BinaryChecksumMismatch));
        }
    }

    let packaged_directory =
        packaged_directory.ok_or_else(|| failure(CollectorFailureCode::BinaryChecksumMismatch))?;
    let payload = packaged_directory.join(format!("{}.payload", entry.executable_name()));
    let materialized = materialize_payload(&payload, entry.executable_name(), expected_sha256)?;
    Ok((
        materialized.executable,
        Some(materialized.directory),
        CollectorIntegrity::Verified,
    ))
}

struct MaterializedPayload {
    directory: TempDir,
    executable: PathBuf,
}

fn materialize_payload(
    payload: &std::path::Path,
    executable_name: &str,
    expected_sha256: &str,
) -> Result<MaterializedPayload, CollectorFailure> {
    let bytes = fs::read(payload).map_err(|_| failure(CollectorFailureCode::BinaryMissing))?;
    let executable_bytes = bytes
        .strip_prefix(PACKAGED_PAYLOAD_HEADER)
        .ok_or_else(|| failure(CollectorFailureCode::BinaryChecksumMismatch))?;
    let observed = format!("{:x}", Sha256::digest(executable_bytes));
    if observed != expected_sha256 {
        return Err(failure(CollectorFailureCode::BinaryChecksumMismatch));
    }

    let directory = tempfile::Builder::new()
        .prefix("burnly-ccusage-sidecar-")
        .tempdir()
        .map_err(|_| failure(CollectorFailureCode::Internal))?;
    let executable = directory.path().join(executable_name);
    fs::write(&executable, executable_bytes)
        .map_err(|_| failure(CollectorFailureCode::Internal))?;
    make_executable(&executable).map_err(|_| failure(CollectorFailureCode::Internal))?;
    Ok(MaterializedPayload {
        directory,
        executable,
    })
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
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
        let fixture = Fixture::new("20.0.19");
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
        assert_eq!(verified.descriptor.runtime_version, "20.0.19");
    }

    #[test]
    fn verifies_packaged_binary_checksum_before_version() {
        let fixture = Fixture::new("20.0.19");
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
    fn materializes_verified_packaged_payload_when_direct_binary_was_mutated() {
        let fixture = Fixture::new("20.0.19");
        let (package_root, packaged) = fixture.package();
        let checksum = sha256(&fixture.executable).expect("checksum");
        fs::remove_file(&packaged).expect("remove linked executable");
        fs::write(&packaged, b"mutated by package tooling").expect("write mutated executable");
        let mut payload = PACKAGED_PAYLOAD_HEADER.to_vec();
        payload.extend(fs::read(&fixture.executable).expect("read fixture"));
        fs::write(
            packaged.with_file_name(format!("{}.payload", fixture.target.executable_name())),
            payload,
        )
        .expect("write payload");
        let manifest = fixture.manifest(&format!(
            r#"{{"kind":"release_sha256","sha256":"{checksum}"}}"#
        ));

        let verified = verify(
            &manifest,
            fixture.target,
            SidecarLocation::PackagedResourceDirectory(package_root.path().to_path_buf()),
            &Active,
        )
        .expect("verified packaged payload");

        assert_ne!(verified.executable, packaged);
        assert_eq!(verified.descriptor.integrity, CollectorIntegrity::Verified);
        assert_eq!(
            sha256(&verified.executable).expect("materialized checksum"),
            checksum
        );
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
            let fixture = if version == "20.0.19" {
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
                    "expectedVersion":"20.0.19",
                    "sourceRevision":"caf89e8c0291a2acec09e01ff609e6253f6dd81b",
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
