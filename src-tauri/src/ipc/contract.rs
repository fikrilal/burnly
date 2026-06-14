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
];

pub(super) const EVENTS: &[EventSpec] = &[
    EventSpec {
        name: "burnly://v1/refresh-progress",
        export_name: "refreshProgress",
        payload_type: "UnknownEventPayload",
    },
    EventSpec {
        name: "burnly://v1/data-invalidated",
        export_name: "dataInvalidated",
        payload_type: "UnknownEventPayload",
    },
    EventSpec {
        name: "burnly://v1/settings-changed",
        export_name: "settingsChanged",
        payload_type: "UnknownEventPayload",
    },
    EventSpec {
        name: "burnly://v1/platform-state-changed",
        export_name: "platformStateChanged",
        payload_type: "UnknownEventPayload",
    },
    EventSpec {
        name: "burnly://v1/update-progress",
        export_name: "updateProgress",
        payload_type: "UnknownEventPayload",
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
    fn registry_uses_contract_version_one() {
        assert_eq!(contract_version(), 1);
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
