use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::net::{IpAddr, Ipv4Addr};
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RuntimeDiscoveryReport {
    pub(crate) process_candidates_found: usize,
    pub(crate) endpoints: Vec<RuntimeEndpoint>,
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

    #[cfg(test)]
    pub(crate) fn discover(&self) -> Vec<RuntimeEndpoint> {
        self.discover_report().endpoints
    }

    pub(crate) fn discover_report(&self) -> RuntimeDiscoveryReport {
        let mut endpoints = Vec::new();
        let mut process_candidates_found = 0;
        for process in &self.processes {
            let Some(variant) = classify_variant(process) else {
                continue;
            };
            process_candidates_found += 1;
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
        RuntimeDiscoveryReport {
            process_candidates_found,
            endpoints,
        }
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
        || executable_named(process.executable.as_deref(), "Antigravity IDE.exe")
        || executable_named(
            process.executable.as_deref(),
            "language_server_windows_x64.exe",
        )
    {
        return Some(AntigravityProductVariant::Ide);
    }
    if executable_named(process.executable.as_deref(), "agy")
        || executable_named(process.executable.as_deref(), "agy.exe")
        || command_contains(&process.command, "agy")
        || command_contains(&process.command, "antigravity-cli")
    {
        return Some(AntigravityProductVariant::Cli);
    }
    if command_has_value(&process.command, "--app_data_dir", "antigravity")
        || command_has_value(&process.command, "--override_ide_name", "antigravity")
        || command_contains(&process.command, "/opt/antigravity/Antigravity-x64")
        || executable_named(process.executable.as_deref(), "Antigravity.exe")
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

fn executable_named(executable: Option<&Path>, name: &str) -> bool {
    executable
        .and_then(Path::file_name)
        .is_some_and(|file_name| os_str_eq_ignore_ascii_case(file_name, name))
}

fn os_str_eq_ignore_ascii_case(value: &OsStr, expected: &str) -> bool {
    value.to_string_lossy().eq_ignore_ascii_case(expected)
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

#[cfg(target_os = "linux")]
fn current_processes() -> Vec<ProcessSnapshot> {
    linux::current_processes()
}

#[cfg(target_os = "windows")]
fn current_processes() -> Vec<ProcessSnapshot> {
    windows::current_processes()
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn current_processes() -> Vec<ProcessSnapshot> {
    Vec::new()
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::fs;
    use std::net::Ipv6Addr;

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
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_INSUFFICIENT_BUFFER, HANDLE, INVALID_HANDLE_VALUE, NO_ERROR,
    };
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
        TCP_TABLE_OWNER_PID_LISTENER,
    };
    use windows_sys::Win32::Networking::WinSock::AF_INET;
    use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PEB, PROCESS_BASIC_INFORMATION,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ, RTL_USER_PROCESS_PARAMETERS,
    };

    const PROCESS_BASIC_INFORMATION_CLASS: i32 = 0;

    extern "system" {
        fn NtQueryInformationProcess(
            process_handle: HANDLE,
            process_information_class: i32,
            process_information: *mut std::ffi::c_void,
            process_information_length: u32,
            return_length: *mut u32,
        ) -> i32;
    }

    pub(super) fn current_processes() -> Vec<ProcessSnapshot> {
        let listeners = listeners_by_process();
        let mut processes = Vec::new();
        for entry in process_entries() {
            let process_id = entry.th32ProcessID;
            let handle = match ProcessHandle::open(process_id) {
                Some(handle) => handle,
                None => {
                    let executable = process_entry_executable(&entry);
                    if executable.is_empty() {
                        continue;
                    }
                    processes.push(ProcessSnapshot {
                        process_id,
                        executable: Some(PathBuf::from(executable)),
                        command: Vec::new(),
                        listeners: listeners.get(&process_id).cloned().unwrap_or_default(),
                    });
                    continue;
                }
            };

            let executable = query_process_image_path(handle.0)
                .map(PathBuf::from)
                .or_else(|| {
                    let executable = process_entry_executable(&entry);
                    (!executable.is_empty()).then(|| PathBuf::from(executable))
                });
            let command = query_process_command_line(handle.0)
                .map(parse_windows_command_line)
                .map(redact_prompt_arguments)
                .unwrap_or_default();
            if command.is_empty() && executable.is_none() {
                continue;
            }

            processes.push(ProcessSnapshot {
                process_id,
                executable,
                command,
                listeners: listeners.get(&process_id).cloned().unwrap_or_default(),
            });
        }
        processes
    }

    fn process_entries() -> Vec<PROCESSENTRY32W> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Vec::new();
        }
        let snapshot = SnapshotHandle(snapshot);

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..PROCESSENTRY32W::default()
        };
        let mut entries = Vec::new();
        if unsafe { Process32FirstW(snapshot.0, &mut entry) } == 0 {
            return entries;
        }
        loop {
            entries.push(entry);
            entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..PROCESSENTRY32W::default()
            };
            if unsafe { Process32NextW(snapshot.0, &mut entry) } == 0 {
                break;
            }
        }
        entries
    }

    fn process_entry_executable(entry: &PROCESSENTRY32W) -> String {
        utf16_null_terminated(&entry.szExeFile)
    }

    fn query_process_image_path(handle: HANDLE) -> Option<String> {
        let mut buffer = vec![0_u16; 32_768];
        let mut length = buffer.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length) };
        if ok == 0 || length == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buffer[..length as usize]))
    }

    fn query_process_command_line(handle: HANDLE) -> Option<String> {
        let mut info = PROCESS_BASIC_INFORMATION::default();
        let status = unsafe {
            NtQueryInformationProcess(
                handle,
                PROCESS_BASIC_INFORMATION_CLASS,
                (&mut info as *mut PROCESS_BASIC_INFORMATION).cast(),
                std::mem::size_of::<PROCESS_BASIC_INFORMATION>() as u32,
                std::ptr::null_mut(),
            )
        };
        if status < 0 || info.PebBaseAddress.is_null() {
            return None;
        }

        let peb = read_remote_value::<PEB>(handle, info.PebBaseAddress)?;
        if peb.ProcessParameters.is_null() {
            return None;
        }
        let parameters =
            read_remote_value::<RTL_USER_PROCESS_PARAMETERS>(handle, peb.ProcessParameters)?;
        let command_line = parameters.CommandLine;
        if command_line.Buffer.is_null() || command_line.Length == 0 {
            return None;
        }
        let character_count = usize::from(command_line.Length) / std::mem::size_of::<u16>();
        if character_count == 0 || character_count > 32_768 {
            return None;
        }

        let mut buffer = vec![0_u16; character_count];
        let mut bytes_read = 0_usize;
        let ok = unsafe {
            ReadProcessMemory(
                handle,
                command_line.Buffer.cast(),
                buffer.as_mut_ptr().cast(),
                usize::from(command_line.Length),
                &mut bytes_read,
            )
        };
        if ok == 0 || bytes_read < usize::from(command_line.Length) {
            return None;
        }
        Some(String::from_utf16_lossy(&buffer))
    }

    fn read_remote_value<T: Copy + Default>(handle: HANDLE, address: *const T) -> Option<T> {
        let mut value = T::default();
        let mut bytes_read = 0_usize;
        let ok = unsafe {
            ReadProcessMemory(
                handle,
                address.cast(),
                (&mut value as *mut T).cast(),
                std::mem::size_of::<T>(),
                &mut bytes_read,
            )
        };
        (ok != 0 && bytes_read == std::mem::size_of::<T>()).then_some(value)
    }

    fn parse_windows_command_line(command_line: String) -> Vec<String> {
        let mut args = Vec::new();
        let mut current = String::new();
        let mut chars = command_line.chars().peekable();
        let mut in_quotes = false;
        let mut backslashes = 0_usize;

        while let Some(character) = chars.next() {
            match character {
                '\\' => {
                    backslashes += 1;
                }
                '"' => {
                    current.extend(std::iter::repeat_n('\\', backslashes / 2));
                    if backslashes.is_multiple_of(2) {
                        in_quotes = !in_quotes;
                    } else {
                        current.push('"');
                    }
                    backslashes = 0;
                }
                character if character.is_whitespace() && !in_quotes => {
                    current.extend(std::iter::repeat_n('\\', backslashes));
                    backslashes = 0;
                    if !current.is_empty() {
                        args.push(std::mem::take(&mut current));
                    }
                    while matches!(chars.peek(), Some(next) if next.is_whitespace()) {
                        chars.next();
                    }
                }
                character => {
                    current.extend(std::iter::repeat_n('\\', backslashes));
                    backslashes = 0;
                    current.push(character);
                }
            }
        }
        current.extend(std::iter::repeat_n('\\', backslashes));
        if !current.is_empty() {
            args.push(current);
        }
        args
    }

    fn utf16_null_terminated(buffer: &[u16]) -> String {
        let length = buffer
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..length])
    }

    struct SnapshotHandle(HANDLE);

    impl Drop for SnapshotHandle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    struct ProcessHandle(HANDLE);

    impl ProcessHandle {
        fn open(process_id: u32) -> Option<Self> {
            let handle = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
                    0,
                    process_id,
                )
            };
            (!handle.is_null()).then_some(Self(handle))
        }
    }

    impl Drop for ProcessHandle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    fn listeners_by_process() -> BTreeMap<u32, Vec<LocalListener>> {
        listener_rows_by_process(tcp_listener_rows().iter())
    }

    fn listener_rows_by_process<'a>(
        rows: impl IntoIterator<Item = &'a MIB_TCPROW_OWNER_PID>,
    ) -> BTreeMap<u32, Vec<LocalListener>> {
        let mut listeners: BTreeMap<u32, BTreeSet<(IpAddr, u16)>> = BTreeMap::new();
        for row in rows {
            let host = Ipv4Addr::from(u32::from_be(row.dwLocalAddr).to_be_bytes());
            let port = u16::from_be((row.dwLocalPort & 0xFFFF) as u16);
            listeners
                .entry(row.dwOwningPid)
                .or_default()
                .insert((IpAddr::V4(host), port));
        }
        listeners
            .into_iter()
            .map(|(process_id, entries)| {
                (
                    process_id,
                    entries
                        .into_iter()
                        .map(|(host, port)| LocalListener::new(host, port))
                        .collect(),
                )
            })
            .collect()
    }

    fn tcp_listener_rows() -> Vec<MIB_TCPROW_OWNER_PID> {
        let mut table_size = 0_u32;
        let initial_result = unsafe {
            GetExtendedTcpTable(
                std::ptr::null_mut(),
                &mut table_size,
                0,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if initial_result != ERROR_INSUFFICIENT_BUFFER || table_size == 0 {
            return Vec::new();
        }

        let mut table = vec![0_u8; table_size as usize];
        let result = unsafe {
            GetExtendedTcpTable(
                table.as_mut_ptr().cast(),
                &mut table_size,
                0,
                AF_INET as u32,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if result != NO_ERROR {
            return Vec::new();
        }

        let table = unsafe { &*(table.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>()) };
        let rows = unsafe {
            std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
        };
        rows.to_vec()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn groups_ipv4_listener_rows_by_process() {
            let rows = [
                row(Ipv4Addr::LOCALHOST, 3230, 10),
                row(Ipv4Addr::new(192, 168, 1, 10), 3231, 10),
                row(Ipv4Addr::LOCALHOST, 2065, 11),
            ];

            let listeners = listener_rows_by_process(rows.iter());

            assert_eq!(
                listeners.get(&10),
                Some(&vec![
                    LocalListener::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3230),
                    LocalListener::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)), 3231),
                ])
            );
            assert_eq!(
                listeners.get(&11),
                Some(&vec![LocalListener::new(
                    IpAddr::V4(Ipv4Addr::LOCALHOST),
                    2065
                )])
            );
        }

        fn row(host: Ipv4Addr, port: u16, process_id: u32) -> MIB_TCPROW_OWNER_PID {
            MIB_TCPROW_OWNER_PID {
                dwState: 0,
                dwLocalAddr: u32::from(host).to_be(),
                dwLocalPort: u32::from(port.to_be()),
                dwRemoteAddr: 0,
                dwRemotePort: 0,
                dwOwningPid: process_id,
            }
        }

        #[test]
        fn parses_quoted_windows_command_line() {
            let args = parse_windows_command_line(
                r#""C:\Program Files\Antigravity\language_server.exe" --app_data_dir antigravity --csrf_token token"#
                    .to_owned(),
            );

            assert_eq!(
                args,
                vec![
                    r"C:\Program Files\Antigravity\language_server.exe",
                    "--app_data_dir",
                    "antigravity",
                    "--csrf_token",
                    "token",
                ]
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
    fn discovers_windows_app_endpoint_from_language_server_process() {
        let discovery = RuntimeDiscovery::from_processes(vec![ProcessSnapshot::new(
            20,
            Some(PathBuf::from(
                r"C:\Users\user\AppData\Local\Programs\antigravity\resources\bin\language_server.exe",
            )),
            vec![
                "language_server.exe".to_owned(),
                "--app_data_dir".to_owned(),
                "antigravity".to_owned(),
                "--override_ide_name".to_owned(),
                "antigravity".to_owned(),
                "--csrf_token".to_owned(),
                "token-app".to_owned(),
            ],
            vec![LocalListener::ipv4(3230)],
        )]);

        let endpoints = discovery.discover();

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].variant, AntigravityProductVariant::App);
        assert_eq!(endpoints[0].csrf_token.as_deref(), Some("token-app"));
    }

    #[test]
    fn discovers_windows_ide_endpoint_from_language_server_process() {
        let discovery = RuntimeDiscovery::from_processes(vec![ProcessSnapshot::new(
            21,
            Some(PathBuf::from(
                r"C:\Users\user\AppData\Local\Programs\Antigravity IDE\resources\app\extensions\antigravity\bin\language_server_windows_x64.exe",
            )),
            vec![
                "language_server_windows_x64.exe".to_owned(),
                "--app_data_dir".to_owned(),
                "antigravity-ide".to_owned(),
                "--subclient_type".to_owned(),
                "ide".to_owned(),
                "--csrf_token".to_owned(),
                "token-ide".to_owned(),
            ],
            vec![LocalListener::ipv4(2065)],
        )]);

        let endpoints = discovery.discover();

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].variant, AntigravityProductVariant::Ide);
        assert_eq!(endpoints[0].csrf_token.as_deref(), Some("token-ide"));
    }

    #[test]
    fn discovers_windows_cli_endpoint_from_agy_executable() {
        let discovery = RuntimeDiscovery::from_processes(vec![ProcessSnapshot::new(
            22,
            Some(PathBuf::from(
                r"C:\Users\user\AppData\Local\agy\bin\agy.exe",
            )),
            vec!["agy.exe".to_owned()],
            vec![LocalListener::ipv4(11044)],
        )]);

        let endpoints = discovery.discover();

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].variant, AntigravityProductVariant::Cli);
    }

    #[test]
    fn does_not_classify_unrelated_process_under_antigravity_directory() {
        let discovery = RuntimeDiscovery::from_processes(vec![ProcessSnapshot::new(
            23,
            Some(PathBuf::from(
                r"C:\Users\user\.antigravity-ide\extensions\rust-analyzer.exe",
            )),
            vec!["rust-analyzer.exe".to_owned()],
            vec![LocalListener::ipv4(11044)],
        )]);

        assert!(discovery.discover().is_empty());
    }

    #[test]
    fn redacts_cli_prompt_arguments_from_process_snapshot() {
        let command = redact_prompt_arguments(vec![
            "agy".to_owned(),
            "--prompt".to_owned(),
            "private prompt".to_owned(),
            "--model".to_owned(),
            "gemini".to_owned(),
            "--prompt-interactive".to_owned(),
            "private interactive prompt".to_owned(),
        ]);

        assert_eq!(
            command,
            vec![
                "agy",
                "--prompt",
                "<redacted>",
                "--model",
                "gemini",
                "--prompt-interactive",
                "<redacted>"
            ]
        );
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
