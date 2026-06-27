//! Native system tray menu integration.

use chrono::DateTime;
use tauri::menu::{MenuBuilder, MenuEvent, MenuItem, MenuItemBuilder};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Runtime};

const TRAY_ID: &str = "burnly-tray";
const OPEN_PANEL_ID: &str = "burnly.tray.open_panel";
const REFRESH_ID: &str = "burnly.tray.refresh";
const STATUS_ID: &str = "burnly.tray.status";
const QUIT_ID: &str = "burnly.tray.quit";
const TODAY_ID: &str = "burnly.tray.today";
const WEEK_ID: &str = "burnly.tray.week";
const MONTH_ID: &str = "burnly.tray.month";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayAction {
    OpenPanel,
    Refresh,
    Quit,
}

impl TrayAction {
    pub(crate) fn from_menu_event(event: &MenuEvent) -> Option<Self> {
        if event.id() == OPEN_PANEL_ID {
            Some(Self::OpenPanel)
        } else if event.id() == REFRESH_ID {
            Some(Self::Refresh)
        } else if event.id() == QUIT_ID {
            Some(Self::Quit)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayRefreshStatus {
    Idle,
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Partial,
    Failed,
}

impl TrayRefreshStatus {
    const fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::Cancelling)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraySnapshot {
    pub(crate) status: TrayRefreshStatus,
    pub(crate) last_successful_refresh_at_ms: Option<i64>,
    pub(crate) budget_summary: Option<String>,
    pub(crate) today_tokens: Option<u64>,
    pub(crate) week_tokens: Option<u64>,
    pub(crate) month_tokens: Option<u64>,
}

pub(crate) struct TrayController<R: Runtime> {
    _tray_icon: TrayIcon<R>,
    status_item: MenuItem<R>,
    refresh_item: MenuItem<R>,
    today_item: MenuItem<R>,
    week_item: MenuItem<R>,
    month_item: MenuItem<R>,
}

impl<R: Runtime> Clone for TrayController<R> {
    fn clone(&self) -> Self {
        Self {
            _tray_icon: self._tray_icon.clone(),
            status_item: self.status_item.clone(),
            refresh_item: self.refresh_item.clone(),
            today_item: self.today_item.clone(),
            week_item: self.week_item.clone(),
            month_item: self.month_item.clone(),
        }
    }
}

impl<R: Runtime> TrayController<R> {
    pub(crate) fn install(manager: &AppHandle<R>, snapshot: &TraySnapshot) -> tauri::Result<Self> {
        let today_item = MenuItemBuilder::with_id(TODAY_ID, today_label(snapshot))
            .enabled(false)
            .build(manager)?;
        let week_item = MenuItemBuilder::with_id(WEEK_ID, week_label(snapshot))
            .enabled(false)
            .build(manager)?;
        let month_item = MenuItemBuilder::with_id(MONTH_ID, month_label(snapshot))
            .enabled(false)
            .build(manager)?;
        let status_item = MenuItemBuilder::with_id(STATUS_ID, status_label(snapshot))
            .enabled(false)
            .build(manager)?;
        let refresh_item = MenuItemBuilder::with_id(REFRESH_ID, refresh_label(snapshot.status))
            .enabled(!snapshot.status.is_active())
            .build(manager)?;
        let open_panel_item =
            MenuItemBuilder::with_id(OPEN_PANEL_ID, "Open Summary").build(manager)?;
        let quit_item = MenuItemBuilder::with_id(QUIT_ID, "Quit Burnly").build(manager)?;

        let menu = MenuBuilder::new(manager)
            .items(&[&today_item, &week_item, &month_item])
            .separator()
            .items(&[&open_panel_item, &status_item])
            .separator()
            .item(&refresh_item)
            .separator()
            .item(&quit_item)
            .build()?;

        let icon = manager.default_window_icon().cloned();
        let mut builder = TrayIconBuilder::with_id(TRAY_ID)
            .menu(&menu)
            .show_menu_on_left_click(true)
            .tooltip(tooltip_label(snapshot));
        if let Some(icon) = icon {
            builder = builder.icon(icon);
        }
        let tray_icon = builder.build(manager)?;

        Ok(Self {
            _tray_icon: tray_icon,
            status_item,
            refresh_item,
            today_item,
            week_item,
            month_item,
        })
    }

    pub(crate) fn update(&self, snapshot: &TraySnapshot) {
        let _ = self.today_item.set_text(today_label(snapshot));
        let _ = self.week_item.set_text(week_label(snapshot));
        let _ = self.month_item.set_text(month_label(snapshot));
        let _ = self.status_item.set_text(status_label(snapshot));
        let _ = self.refresh_item.set_text(refresh_label(snapshot.status));
        let _ = self.refresh_item.set_enabled(!snapshot.status.is_active());
        let _ = self._tray_icon.set_tooltip(Some(tooltip_label(snapshot)));
    }
}

#[cfg(test)]
fn action_from_id(id: &str) -> Option<TrayAction> {
    match id {
        OPEN_PANEL_ID => Some(TrayAction::OpenPanel),
        REFRESH_ID => Some(TrayAction::Refresh),
        QUIT_ID => Some(TrayAction::Quit),
        _ => None,
    }
}

fn refresh_label(status: TrayRefreshStatus) -> &'static str {
    if status.is_active() {
        "Refresh running"
    } else {
        "Refresh now"
    }
}

fn status_label(snapshot: &TraySnapshot) -> String {
    let status = match snapshot.status {
        TrayRefreshStatus::Idle => "Idle",
        TrayRefreshStatus::Queued => "Refresh queued",
        TrayRefreshStatus::Running => "Refreshing",
        TrayRefreshStatus::Cancelling => "Cancelling refresh",
        TrayRefreshStatus::Succeeded => "Refresh succeeded",
        TrayRefreshStatus::Partial => "Refresh partially completed",
        TrayRefreshStatus::Failed => "Refresh failed",
    };
    let refresh_status = match snapshot.last_successful_refresh_at_ms {
        Some(timestamp) => format!("{status} - last success {}", format_time(timestamp)),
        None => status.to_owned(),
    };
    match snapshot.budget_summary.as_deref() {
        Some(summary) => format!("{refresh_status} - {summary}"),
        None => refresh_status,
    }
}

fn today_label(snapshot: &TraySnapshot) -> String {
    format!(
        "Today: {} tokens",
        snapshot
            .today_tokens
            .map(format_number)
            .unwrap_or_else(|| "---".to_owned())
    )
}

fn week_label(snapshot: &TraySnapshot) -> String {
    format!(
        "This week: {} tokens",
        snapshot
            .week_tokens
            .map(format_number)
            .unwrap_or_else(|| "---".to_owned())
    )
}

fn month_label(snapshot: &TraySnapshot) -> String {
    format!(
        "This month: {} tokens",
        snapshot
            .month_tokens
            .map(format_number)
            .unwrap_or_else(|| "---".to_owned())
    )
}

fn tooltip_label(snapshot: &TraySnapshot) -> String {
    format!("Burnly - {}", status_label(snapshot))
}

fn format_time(epoch_ms: i64) -> String {
    DateTime::from_timestamp_millis(epoch_ms)
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn format_number(val: u64) -> String {
    let s = val.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for (i, c) in chars.into_iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_ids_map_to_tray_actions() {
        assert_eq!(action_from_id(OPEN_PANEL_ID), Some(TrayAction::OpenPanel));

        assert_eq!(action_from_id(REFRESH_ID), Some(TrayAction::Refresh));
        assert_eq!(action_from_id(QUIT_ID), Some(TrayAction::Quit));
        assert_eq!(action_from_id(STATUS_ID), None);
        assert_eq!(action_from_id("other"), None);
    }

    #[test]
    fn active_refresh_disables_refresh_action_label() {
        assert_eq!(refresh_label(TrayRefreshStatus::Running), "Refresh running");
        assert_eq!(refresh_label(TrayRefreshStatus::Queued), "Refresh running");
        assert_eq!(refresh_label(TrayRefreshStatus::Idle), "Refresh now");
    }

    #[test]
    fn status_label_includes_last_success_when_available() {
        let snapshot = TraySnapshot {
            status: TrayRefreshStatus::Succeeded,
            last_successful_refresh_at_ms: Some(1_765_000_000_000),
            budget_summary: None,
            today_tokens: None,
            week_tokens: None,
            month_tokens: None,
        };

        assert_eq!(
            status_label(&snapshot),
            "Refresh succeeded - last success 2025-12-06 05:46 UTC"
        );
    }

    #[test]
    fn tooltip_includes_preformatted_budget_summary_when_available() {
        let snapshot = TraySnapshot {
            status: TrayRefreshStatus::Idle,
            last_successful_refresh_at_ms: None,
            budget_summary: Some("Budget: Monthly 82%".to_owned()),
            today_tokens: None,
            week_tokens: None,
            month_tokens: None,
        };

        assert_eq!(status_label(&snapshot), "Idle - Budget: Monthly 82%");
        assert_eq!(
            tooltip_label(&snapshot),
            "Burnly - Idle - Budget: Monthly 82%"
        );
    }

    #[test]
    fn test_number_formatting() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(123), "123");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234567), "1,234,567");
    }
}
