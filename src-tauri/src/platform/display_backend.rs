//! Linux display-backend selection for deterministic tray panel placement.
//!
//! Wayland compositors ignore absolute window placement, so the tray panel
//! cannot be positioned when Burnly runs as a native Wayland client. When a
//! Wayland session exposes a reachable XWayland display, Burnly selects the GTK
//! X11 backend before GTK initializes, which keeps the existing corner
//! placement working. A pre-set `GDK_BACKEND` always wins.

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisplayBackend {
    X11,
    Wayland,
    Unspecified,
}

impl DisplayBackend {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::X11 => "x11",
            Self::Wayland => "wayland",
            Self::Unspecified => "unspecified",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisplayBackendReason {
    NotLinux,
    ManualEnvValue,
    X11Session,
    WaylandX11Selected,
    WaylandNoDisplay,
    WaylandProbeFailed,
}

impl DisplayBackendReason {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NotLinux => "not_linux",
            Self::ManualEnvValue => "manual_env_value",
            Self::X11Session => "x11_session",
            Self::WaylandX11Selected => "wayland_x11_selected",
            Self::WaylandNoDisplay => "wayland_no_display",
            Self::WaylandProbeFailed => "wayland_probe_failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DisplayBackendDecision {
    pub(crate) backend: DisplayBackend,
    pub(crate) reason: DisplayBackendReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DisplayEnvironment {
    gdk_backend: Option<String>,
    wayland_display: bool,
    xdg_session_type_wayland: bool,
    display: Option<String>,
}

impl DisplayEnvironment {
    fn from_process_environment() -> Self {
        Self {
            gdk_backend: non_empty_env("GDK_BACKEND"),
            wayland_display: std::env::var_os("WAYLAND_DISPLAY").is_some(),
            xdg_session_type_wayland: std::env::var("XDG_SESSION_TYPE")
                .map(|value| value.eq_ignore_ascii_case("wayland"))
                .unwrap_or(false),
            display: non_empty_env("DISPLAY"),
        }
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

static DECISION: OnceLock<DisplayBackendDecision> = OnceLock::new();

/// Selects the GTK backend and stores the decision for diagnostics.
///
/// Must run before any Tauri or GTK initialization, because GTK reads
/// `GDK_BACKEND` when it opens the display.
pub(crate) fn select_display_backend() -> DisplayBackendDecision {
    let decision = if cfg!(target_os = "linux") {
        let environment = DisplayEnvironment::from_process_environment();
        decide(&environment, x_display_reachable)
    } else {
        DisplayBackendDecision {
            backend: DisplayBackend::Unspecified,
            reason: DisplayBackendReason::NotLinux,
        }
    };

    if decision.reason == DisplayBackendReason::WaylandX11Selected {
        std::env::set_var("GDK_BACKEND", "x11");
    }

    let _ = DECISION.set(decision);

    decision
}

/// The backend decision made at startup, if selection has run.
pub(crate) fn decision() -> Option<DisplayBackendDecision> {
    DECISION.get().copied()
}

fn decide(
    environment: &DisplayEnvironment,
    probe_x_display: impl FnOnce(&str) -> bool,
) -> DisplayBackendDecision {
    if let Some(value) = environment.gdk_backend.as_deref() {
        return DisplayBackendDecision {
            backend: backend_from_gdk_backend(value),
            reason: DisplayBackendReason::ManualEnvValue,
        };
    }

    let wayland_session = environment.wayland_display || environment.xdg_session_type_wayland;
    if !wayland_session {
        return DisplayBackendDecision {
            backend: DisplayBackend::X11,
            reason: DisplayBackendReason::X11Session,
        };
    }

    let Some(display) = environment.display.as_deref() else {
        return DisplayBackendDecision {
            backend: DisplayBackend::Wayland,
            reason: DisplayBackendReason::WaylandNoDisplay,
        };
    };

    if probe_x_display(display) {
        DisplayBackendDecision {
            backend: DisplayBackend::X11,
            reason: DisplayBackendReason::WaylandX11Selected,
        }
    } else {
        DisplayBackendDecision {
            backend: DisplayBackend::Wayland,
            reason: DisplayBackendReason::WaylandProbeFailed,
        }
    }
}

fn backend_from_gdk_backend(value: &str) -> DisplayBackend {
    match value.trim() {
        "x11" => DisplayBackend::X11,
        "wayland" => DisplayBackend::Wayland,
        _ => DisplayBackend::Unspecified,
    }
}

#[cfg(target_os = "linux")]
fn x_display_reachable(display: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::linux::net::SocketAddrExt;
        use std::os::unix::net::{SocketAddr, UnixStream};

        let Some(display_number) = local_display_number(display) else {
            return false;
        };
        let socket_name = format!("/tmp/.X11-unix/X{display_number}");

        if let Ok(address) = SocketAddr::from_abstract_name(socket_name.as_bytes()) {
            if UnixStream::connect_addr(&address).is_ok() {
                return true;
            }
        }

        UnixStream::connect(socket_name).is_ok()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = display;
        false
    }
}

#[cfg(target_os = "linux")]
fn local_display_number(display: &str) -> Option<u32> {
    let value = display.trim();
    let value = value
        .strip_prefix("unix:")
        .or_else(|| value.strip_prefix(':'))?;
    let number = value.split('.').next().unwrap_or_default();
    number.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wayland_session() -> DisplayEnvironment {
        DisplayEnvironment {
            gdk_backend: None,
            wayland_display: true,
            xdg_session_type_wayland: false,
            display: Some(":0".to_owned()),
        }
    }

    #[test]
    fn manual_gdk_backend_always_wins() {
        let mut environment = wayland_session();
        environment.gdk_backend = Some("wayland".to_owned());
        assert_eq!(
            decide(&environment, |_| true),
            DisplayBackendDecision {
                backend: DisplayBackend::Wayland,
                reason: DisplayBackendReason::ManualEnvValue,
            }
        );

        let mut environment = wayland_session();
        environment.gdk_backend = Some("x11".to_owned());
        assert_eq!(
            decide(&environment, |_| false),
            DisplayBackendDecision {
                backend: DisplayBackend::X11,
                reason: DisplayBackendReason::ManualEnvValue,
            }
        );
    }

    #[test]
    fn x11_session_keeps_gtk_defaults() {
        let environment = DisplayEnvironment {
            gdk_backend: None,
            wayland_display: false,
            xdg_session_type_wayland: false,
            display: Some(":0".to_owned()),
        };

        assert_eq!(
            decide(&environment, |_| true),
            DisplayBackendDecision {
                backend: DisplayBackend::X11,
                reason: DisplayBackendReason::X11Session,
            }
        );
    }

    #[test]
    fn wayland_session_selects_x11_when_display_is_reachable() {
        assert_eq!(
            decide(&wayland_session(), |_| true),
            DisplayBackendDecision {
                backend: DisplayBackend::X11,
                reason: DisplayBackendReason::WaylandX11Selected,
            }
        );
    }

    #[test]
    fn wayland_session_stays_wayland_without_display_or_reachable_probe() {
        let mut environment = wayland_session();
        environment.display = None;
        assert_eq!(
            decide(&environment, |_| true).reason,
            DisplayBackendReason::WaylandNoDisplay
        );

        assert_eq!(
            decide(&wayland_session(), |_| false).reason,
            DisplayBackendReason::WaylandProbeFailed
        );
    }

    #[test]
    fn xdg_session_type_alone_marks_a_wayland_session() {
        let environment = DisplayEnvironment {
            gdk_backend: None,
            wayland_display: false,
            xdg_session_type_wayland: true,
            display: Some(":1".to_owned()),
        };

        assert_eq!(
            decide(&environment, |_| true).reason,
            DisplayBackendReason::WaylandX11Selected
        );
    }

    #[test]
    fn probe_receives_the_configured_display() {
        let mut probed_display = None;
        let _ = decide(&wayland_session(), |display| {
            probed_display = Some(display.to_owned());
            true
        });

        assert_eq!(probed_display.as_deref(), Some(":0"));
    }

    #[test]
    fn unknown_manual_values_are_reported_as_unspecified() {
        let mut environment = wayland_session();
        environment.gdk_backend = Some("wayland,x11".to_owned());

        assert_eq!(
            decide(&environment, |_| true).backend,
            DisplayBackend::Unspecified
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn local_display_numbers_parse_and_remote_displays_do_not() {
        assert_eq!(local_display_number(":0"), Some(0));
        assert_eq!(local_display_number(":12.1"), Some(12));
        assert_eq!(local_display_number("unix:3"), Some(3));
        assert_eq!(local_display_number("localhost:0"), None);
        assert_eq!(local_display_number("wayland-0"), None);
        assert_eq!(local_display_number(""), None);
    }
}
