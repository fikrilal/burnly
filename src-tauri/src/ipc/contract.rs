#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the contract registry is consumed by the TypeScript generation harness"
    )
)]

use super::response::CONTRACT_VERSION;

pub(super) const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "__burnly_contract_probe",
        export_name: "invokeContractProbe",
        request_type: "Record<string, never>",
        response_type: "ContractProbeResponse",
    },
    CommandSpec {
        name: "app_get_bootstrap",
        export_name: "invokeAppGetBootstrap",
        request_type: "Record<string, never>",
        response_type: "AppBootstrapResponse",
    },
    CommandSpec {
        name: "app_get_capabilities",
        export_name: "invokeAppGetCapabilities",
        request_type: "Record<string, never>",
        response_type: "AppCapabilitiesResponse",
    },
    CommandSpec {
        name: "app_hide_tray_panel",
        export_name: "invokeAppHideTrayPanel",
        request_type: "Record<string, never>",
        response_type: "HideTrayPanelResponse",
    },
    CommandSpec {
        name: "app_open_external_url",
        export_name: "invokeAppOpenExternalUrl",
        request_type: "OpenExternalUrlCommandRequest",
        response_type: "OpenExternalUrlResponse",
    },
    CommandSpec {
        name: "diagnostics_get_health",
        export_name: "invokeDiagnosticsGetHealth",
        request_type: "Record<string, never>",
        response_type: "DiagnosticsHealthResponse",
    },
    CommandSpec {
        name: "diagnostics_export_report",
        export_name: "invokeDiagnosticsExportReport",
        request_type: "Record<string, never>",
        response_type: "DiagnosticsExportResponse",
    },
    CommandSpec {
        name: "diagnostics_copy_report",
        export_name: "invokeDiagnosticsCopyReport",
        request_type: "Record<string, never>",
        response_type: "DiagnosticsCopyResponse",
    },
    CommandSpec {
        name: "account_get_session",
        export_name: "invokeAccountGetSession",
        request_type: "Record<string, never>",
        response_type: "AccountSessionResponse",
    },
    CommandSpec {
        name: "account_start_login",
        export_name: "invokeAccountStartLogin",
        request_type: "Record<string, never>",
        response_type: "AccountSessionResponse",
    },
    CommandSpec {
        name: "account_cancel_login",
        export_name: "invokeAccountCancelLogin",
        request_type: "Record<string, never>",
        response_type: "AccountSessionResponse",
    },
    CommandSpec {
        name: "account_logout",
        export_name: "invokeAccountLogout",
        request_type: "Record<string, never>",
        response_type: "AccountSessionResponse",
    },
    CommandSpec {
        name: "collect_sync_get_status",
        export_name: "invokeCollectSyncGetStatus",
        request_type: "Record<string, never>",
        response_type: "CollectSyncStatusResponse",
    },
    CommandSpec {
        name: "collect_sync_retry",
        export_name: "invokeCollectSyncRetry",
        request_type: "Record<string, never>",
        response_type: "CollectSyncStatusResponse",
    },
    CommandSpec {
        name: "settings_get",
        export_name: "invokeSettingsGet",
        request_type: "Record<string, never>",
        response_type: "SettingsResponse",
    },
    CommandSpec {
        name: "settings_update",
        export_name: "invokeSettingsUpdate",
        request_type: "UpdateSettingsCommandRequest",
        response_type: "SettingsResponse",
    },
    CommandSpec {
        name: "refresh_get_state",
        export_name: "invokeRefreshGetState",
        request_type: "Record<string, never>",
        response_type: "RefreshStatusResponse",
    },
    CommandSpec {
        name: "refresh_request",
        export_name: "invokeRefreshRequest",
        request_type: "Record<string, never>",
        response_type: "RefreshStatusResponse",
    },
    CommandSpec {
        name: "refresh_cancel",
        export_name: "invokeRefreshCancel",
        request_type: "Record<string, never>",
        response_type: "RefreshStatusResponse",
    },
    CommandSpec {
        name: "update_get_state",
        export_name: "invokeUpdateGetState",
        request_type: "Record<string, never>",
        response_type: "UpdateStatusResponse",
    },
    CommandSpec {
        name: "update_check",
        export_name: "invokeUpdateCheck",
        request_type: "Record<string, never>",
        response_type: "UpdateStatusResponse",
    },
    CommandSpec {
        name: "update_download",
        export_name: "invokeUpdateDownload",
        request_type: "Record<string, never>",
        response_type: "UpdateStatusResponse",
    },
    CommandSpec {
        name: "update_restart",
        export_name: "invokeUpdateRestart",
        request_type: "Record<string, never>",
        response_type: "UpdateStatusResponse",
    },
    CommandSpec {
        name: "usage_get_tray_summary",
        export_name: "invokeUsageGetTraySummary",
        request_type: "TraySummaryCommandRequest",
        response_type: "TraySummaryResponse",
    },
];

pub(super) const EVENTS: &[EventSpec] = &[
    EventSpec {
        name: "burnly://v1/refresh-progress",
        export_name: "refreshProgress",
        payload_type: "RefreshProgressEvent",
    },
    EventSpec {
        name: "burnly://v1/data-invalidated",
        export_name: "dataInvalidated",
        payload_type: "DataInvalidatedEvent",
    },
    EventSpec {
        name: "burnly://v1/settings-changed",
        export_name: "settingsChanged",
        payload_type: "SettingsChangedEvent",
    },
    EventSpec {
        name: "burnly://v1/account-session-changed",
        export_name: "accountSessionChanged",
        payload_type: "AccountSessionChangedEvent",
    },
    EventSpec {
        name: "burnly://v1/collect-sync-changed",
        export_name: "collectSyncChanged",
        payload_type: "CollectSyncChangedEvent",
    },
    EventSpec {
        name: "burnly://v1/platform-state-changed",
        export_name: "platformStateChanged",
        payload_type: "PlatformStateChangedEvent",
    },
    EventSpec {
        name: "burnly://v1/update-progress",
        export_name: "updateProgress",
        payload_type: "UpdateProgressEvent",
    },
];

pub(super) struct CommandSpec {
    pub(super) name: &'static str,
    pub(super) export_name: &'static str,
    pub(super) request_type: &'static str,
    pub(super) response_type: &'static str,
}

pub(super) struct EventSpec {
    pub(super) name: &'static str,
    pub(super) export_name: &'static str,
    pub(super) payload_type: &'static str,
}

pub(super) fn contract_version() -> u16 {
    CONTRACT_VERSION
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn registry_uses_contract_version_two() {
        assert_eq!(contract_version(), 2);
    }

    #[test]
    fn command_names_are_unique() {
        let mut names = HashSet::new();
        let mut exports = HashSet::new();

        for command in COMMANDS {
            assert!(names.insert(command.name), "duplicate command name");
            assert!(
                exports.insert(command.export_name),
                "duplicate command export"
            );
            assert!(!command.request_type.is_empty());
            assert!(!command.response_type.is_empty());
        }
    }

    #[test]
    fn event_names_are_versioned_and_unique() {
        let mut names = HashSet::new();
        let mut exports = HashSet::new();

        for event in EVENTS {
            assert!(event.name.starts_with("burnly://v1/"));
            assert!(names.insert(event.name), "duplicate event name");
            assert!(exports.insert(event.export_name), "duplicate event export");
            assert!(!event.payload_type.is_empty());
        }
    }
}
