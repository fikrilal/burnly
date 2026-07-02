use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};

use super::product_variant::AntigravityProductVariant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeEndpoint {
    pub(crate) variant: AntigravityProductVariant,
    pub(crate) process_id: u32,
    pub(crate) host: IpAddr,
    pub(crate) port: u16,
    pub(crate) csrf_token: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeDiscovery {
    processes: Vec<ProcessSnapshot>,
}

impl RuntimeDiscovery {
    pub(crate) fn current() -> Self {
        Self {
            processes: current_processes(),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_processes(processes: Vec<ProcessSnapshot>) -> Self {
        Self { processes }
    }

    pub(crate) fn discover(&self) -> Vec<RuntimeEndpoint> {
        let mut endpoints = Vec::new();
        for process in &self.processes {
            let Some(variant) = classify_variant(process) else {
                continue;
            };
            let csrf_token = csrf_token(process);
            for listener in &process.listeners {
                if !listener.host.is_loopback() {
                    continue;
                }
                endpoints.push(RuntimeEndpoint {
                    variant,
                    process_id: process.process_id,
                    host: listener.host,
                    port: listener.port,
                    csrf_token: csrf_token.clone(),
                });
            }
        }
        endpoints.sort_by_key(|endpoint| {
            (
                endpoint.variant.as_str(),
                endpoint.process_id,
                endpoint.host,
                endpoint.port,
            )
        });
        endpoints.dedup_by(|left, right| {
            left.variant == right.variant
                && left.process_id == right.process_id
                && left.host == right.host
                && left.port == right.port
        });
        endpoints
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessSnapshot {
    process_id: u32,
    executable: Option<PathBuf>,
    command: Vec<String>,
    listeners: Vec<LocalListener>,
}

impl ProcessSnapshot {
    #[cfg(test)]
    pub(crate) fn new(
        process_id: u32,
        executable: Option<PathBuf>,
        command: Vec<String>,
        listeners: Vec<LocalListener>,
    ) -> Self {
        Self {
            process_id,
            executable,
            command,
            listeners,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LocalListener {
    host: IpAddr,
    port: u16,
}

impl LocalListener {
    #[cfg(test)]
    pub(crate) const fn ipv4(port: u16) -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
        }
    }

    const fn new(host: IpAddr, port: u16) -> Self {
        Self { host, port }
    }
}

fn classify_variant(process: &ProcessSnapshot) -> Option<AntigravityProductVariant> {
    if command_has_value(&process.command, "--app_data_dir", "antigravity-ide")
        || command_contains(&process.command, "/opt/antigravity-ide/Antigravity-IDE")
    {
        return Some(AntigravityProductVariant::Ide);
    }
    if executable_ends_with(process.executable.as_deref(), "agy")
        || command_contains(&process.command, "agy")
        || command_contains(&process.command, "antigravity-cli")
    {
        return Some(AntigravityProductVariant::Cli);
    }
    if command_has_value(&process.command, "--app_data_dir", "antigravity")
        || command_has_value(&process.command, "--override_ide_name", "antigravity")
        || command_contains(&process.command, "/opt/antigravity/Antigravity-x64")
    {
        return Some(AntigravityProductVariant::App);
    }
    None
}

fn csrf_token(process: &ProcessSnapshot) -> Option<String> {
    flag_value(&process.command, "--csrf_token").filter(|value| !value.trim().is_empty())
}

fn command_has_value(command: &[String], flag: &str, expected: &str) -> bool {
    flag_value(command, flag).as_deref() == Some(expected)
}

fn flag_value(command: &[String], flag: &str) -> Option<String> {
    command
        .windows(2)
        .find(|items| items[0] == flag)
        .map(|items| items[1].clone())
}

fn command_contains(command: &[String], needle: &str) -> bool {
    command.iter().any(|item| item.contains(needle))
}

fn executable_ends_with(executable: Option<&Path>, name: &str) -> bool {
    executable
        .and_then(Path::file_name)
        .and_then(|file_name| file_name.to_str())
        == Some(name)
}

#[cfg(target_os = "linux")]
fn current_processes() -> Vec<ProcessSnapshot> {
    linux::current_processes()
}

#[cfg(not(target_os = "linux"))]
fn current_processes() -> Vec<ProcessSnapshot> {
    Vec::new()
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    pub(super) fn current_processes() -> Vec<ProcessSnapshot> {
        let sockets = listening_sockets();
        let Ok(entries) = fs::read_dir("/proc") else {
            return Vec::new();
        };

        let mut processes = Vec::new();
        for entry in entries.flatten() {
            let Some(process_id) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u32>().ok())
            else {
                continue;
            };
            let process_root = entry.path();
            let command = read_cmdline(&process_root);
            if command.is_empty() {
                continue;
            }
            let executable = fs::read_link(process_root.join("exe")).ok();
            let listeners = process_listeners(&process_root, &sockets);
            processes.push(ProcessSnapshot {
                process_id,
                executable,
                command,
                listeners,
            });
        }
        processes
    }

    fn read_cmdline(process_root: &Path) -> Vec<String> {
        fs::read(process_root.join("cmdline"))
            .ok()
            .map(|bytes| {
                let command = bytes
                    .split(|byte| *byte == 0)
                    .filter_map(|item| {
                        if item.is_empty() {
                            None
                        } else {
                            Some(String::from_utf8_lossy(item).into_owned())
                        }
                    })
                    .collect::<Vec<_>>();
                redact_prompt_arguments(command)
            })
            .unwrap_or_default()
    }

    fn redact_prompt_arguments(command: Vec<String>) -> Vec<String> {
        let mut redacted = Vec::with_capacity(command.len());
        let mut redact_next = false;
        for item in command {
            if redact_next {
                redacted.push("<redacted>".to_owned());
                redact_next = false;
                continue;
            }
            redact_next = matches!(item.as_str(), "--prompt" | "--prompt-interactive");
            redacted.push(item);
        }
        redacted
    }

    fn process_listeners(
        process_root: &Path,
        sockets: &BTreeMap<u64, LocalListener>,
    ) -> Vec<LocalListener> {
        let mut listeners = BTreeSet::new();
        let Ok(entries) = fs::read_dir(process_root.join("fd")) else {
            return Vec::new();
        };
        for entry in entries.flatten() {
            let Ok(target) = fs::read_link(entry.path()) else {
                continue;
            };
            let Some(inode) = socket_inode(&target) else {
                continue;
            };
            if let Some(listener) = sockets.get(&inode) {
                listeners.insert((listener.host, listener.port));
            }
        }
        listeners
            .into_iter()
            .map(|(host, port)| LocalListener::new(host, port))
            .collect()
    }

    fn socket_inode(target: &Path) -> Option<u64> {
        let target = target.to_str()?;
        let inode = target
            .strip_prefix("socket:[")
            .and_then(|value| value.strip_suffix(']'))?;
        inode.parse().ok()
    }

    fn listening_sockets() -> BTreeMap<u64, LocalListener> {
        let mut sockets = BTreeMap::new();
        read_tcp_table("/proc/net/tcp", &mut sockets);
        read_tcp_table("/proc/net/tcp6", &mut sockets);
        sockets
    }

    fn read_tcp_table(path: &str, sockets: &mut BTreeMap<u64, LocalListener>) {
        let Ok(content) = fs::read_to_string(path) else {
            return;
        };
        for line in content.lines().skip(1) {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.len() <= 9 || fields[3] != "0A" {
                continue;
            }
            let Some((host, port)) = parse_local_address(fields[1]) else {
                continue;
            };
            let Ok(inode) = fields[9].parse::<u64>() else {
                continue;
            };
            sockets.insert(inode, LocalListener::new(host, port));
        }
    }

    fn parse_local_address(value: &str) -> Option<(IpAddr, u16)> {
        let (host_hex, port_hex) = value.split_once(':')?;
        let port = u16::from_str_radix(port_hex, 16).ok()?;
        match host_hex.len() {
            8 => {
                let raw = u32::from_str_radix(host_hex, 16).ok()?;
                let octets = raw.to_le_bytes();
                Some((IpAddr::V4(Ipv4Addr::from(octets)), port))
            }
            32 => {
                let mut octets = [0_u8; 16];
                for (index, chunk) in host_hex.as_bytes().chunks(8).enumerate() {
                    let chunk = std::str::from_utf8(chunk).ok()?;
                    let raw = u32::from_str_radix(chunk, 16).ok()?;
                    octets[index * 4..index * 4 + 4].copy_from_slice(&raw.to_le_bytes());
                }
                Some((IpAddr::V6(Ipv6Addr::from(octets)), port))
            }
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_ipv4_proc_net_address() {
            let (host, port) = parse_local_address("0100007F:8667").expect("address");

            assert_eq!(host, IpAddr::V4(Ipv4Addr::LOCALHOST));
            assert_eq!(port, 34407);
        }

        #[test]
        fn redacts_cli_prompt_arguments_from_process_snapshot() {
            let command = redact_prompt_arguments(vec![
                "agy".to_owned(),
                "--prompt".to_owned(),
                "private prompt".to_owned(),
                "--model".to_owned(),
                "gemini".to_owned(),
            ]);

            assert_eq!(
                command,
                vec!["agy", "--prompt", "<redacted>", "--model", "gemini"]
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_app_endpoint_from_antigravity_process() {
        let discovery = RuntimeDiscovery::from_processes(vec![ProcessSnapshot::new(
            10,
            Some(PathBuf::from(
                "/opt/antigravity/Antigravity-x64/language_server",
            )),
            vec![
                "language_server".to_owned(),
                "--override_ide_name".to_owned(),
                "antigravity".to_owned(),
                "--csrf_token".to_owned(),
                "token-app".to_owned(),
            ],
            vec![LocalListener::ipv4(33751)],
        )]);

        let endpoints = discovery.discover();

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].variant, AntigravityProductVariant::App);
        assert_eq!(endpoints[0].port, 33751);
        assert_eq!(endpoints[0].csrf_token.as_deref(), Some("token-app"));
    }

    #[test]
    fn discovers_ide_endpoint_from_language_server_process() {
        let discovery = RuntimeDiscovery::from_processes(vec![ProcessSnapshot::new(
            11,
            Some(PathBuf::from(
                "/opt/antigravity-ide/Antigravity-IDE/resources/app/extensions/antigravity/bin/language_server_linux_x64",
            )),
            vec![
                "language_server_linux_x64".to_owned(),
                "--app_data_dir".to_owned(),
                "antigravity-ide".to_owned(),
                "--csrf_token".to_owned(),
                "token-ide".to_owned(),
            ],
            vec![LocalListener::ipv4(41641)],
        )]);

        let endpoints = discovery.discover();

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].variant, AntigravityProductVariant::Ide);
        assert_eq!(endpoints[0].csrf_token.as_deref(), Some("token-ide"));
    }

    #[test]
    fn discovers_cli_endpoint_without_requiring_csrf_token() {
        let discovery = RuntimeDiscovery::from_processes(vec![ProcessSnapshot::new(
            12,
            Some(PathBuf::from("/home/user/.local/bin/agy")),
            vec!["agy".to_owned()],
            vec![LocalListener::ipv4(34415)],
        )]);

        let endpoints = discovery.discover();

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].variant, AntigravityProductVariant::Cli);
        assert_eq!(endpoints[0].csrf_token, None);
    }

    #[test]
    fn ignores_non_loopback_listeners_and_unrelated_processes() {
        let discovery = RuntimeDiscovery::from_processes(vec![
            ProcessSnapshot::new(
                12,
                Some(PathBuf::from("/home/user/.local/bin/agy")),
                vec!["agy".to_owned()],
                vec![LocalListener::new(
                    IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)),
                    34415,
                )],
            ),
            ProcessSnapshot::new(
                13,
                Some(PathBuf::from("/usr/bin/other")),
                vec!["other".to_owned()],
                vec![LocalListener::ipv4(1)],
            ),
        ]);

        assert!(discovery.discover().is_empty());
    }
}
