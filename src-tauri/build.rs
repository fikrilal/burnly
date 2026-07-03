fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "__burnly_contract_probe",
            "app_get_bootstrap",
            "app_get_capabilities",
            "app_hide_tray_panel",
            "app_open_external_url",
            "diagnostics_get_health",
            "diagnostics_export_report",
            "diagnostics_copy_report",
            "settings_get",
            "settings_update",
            "refresh_get_state",
            "refresh_request",
            "refresh_cancel",
            "update_get_state",
            "update_check",
            "update_download",
            "update_restart",
            "usage_get_tray_summary",
        ]),
    ))
    .expect("failed to build Tauri application manifest")
}
