fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "__burnly_contract_probe",
            "app_get_bootstrap",
            "app_get_capabilities",
            "app_hide_tray_panel",
            "settings_get",
            "settings_update",
            "settings_update_project_path_retention",
            "refresh_get_state",
            "refresh_request",
            "refresh_cancel",
            "usage_get_tray_summary",
        ]),
    ))
    .expect("failed to build Tauri application manifest")
}
