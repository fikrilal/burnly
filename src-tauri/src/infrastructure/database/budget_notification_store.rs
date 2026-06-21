use std::sync::Mutex;

use rusqlite::{params, ErrorCode};

use crate::application::budget_notifications::{BudgetNotificationClaim, BudgetNotificationStatus};
use crate::application::ports::budget_notification_store::{
    BudgetNotificationClaimOutcome, BudgetNotificationStore, BudgetNotificationStoreError,
};

use super::Database;

pub(crate) struct SqliteBudgetNotificationStore {
    database: Mutex<Database>,
}

impl SqliteBudgetNotificationStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl BudgetNotificationStore for SqliteBudgetNotificationStore {
    fn claim(
        &self,
        claim: &BudgetNotificationClaim,
    ) -> Result<BudgetNotificationClaimOutcome, BudgetNotificationStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| BudgetNotificationStoreError::Unavailable)?;
        let observed_value = i64::try_from(claim.observed_value)
            .map_err(|_| BudgetNotificationStoreError::Unavailable)?;
        let result = database.connection().execute(
            "INSERT INTO budget_notification_state (
                budget_id, period_start_date, aggregation_timezone,
                threshold_bps, observed_value, notified_at_ms, delivery_status
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                claim.budget_id.value(),
                claim.period_start_date.to_string(),
                claim.aggregation_timezone,
                claim.threshold_basis_points,
                observed_value,
                claim.notified_at_ms,
                claim.status.as_str(),
            ],
        );
        match result {
            Ok(_) => Ok(BudgetNotificationClaimOutcome::Claimed),
            Err(error) if is_unique_constraint(&error) => {
                Ok(BudgetNotificationClaimOutcome::AlreadyClaimed)
            }
            Err(_) => Err(BudgetNotificationStoreError::Unavailable),
        }
    }

    fn set_status(
        &self,
        claim: &BudgetNotificationClaim,
        status: BudgetNotificationStatus,
    ) -> Result<(), BudgetNotificationStoreError> {
        let database = self
            .database
            .lock()
            .map_err(|_| BudgetNotificationStoreError::Unavailable)?;
        let changed = database
            .connection()
            .execute(
                "UPDATE budget_notification_state
                 SET delivery_status = ?1
                 WHERE budget_id = ?2
                   AND period_start_date = ?3
                   AND aggregation_timezone = ?4
                   AND threshold_bps = ?5",
                params![
                    status.as_str(),
                    claim.budget_id.value(),
                    claim.period_start_date.to_string(),
                    claim.aggregation_timezone,
                    claim.threshold_basis_points,
                ],
            )
            .map_err(|_| BudgetNotificationStoreError::Unavailable)?;
        if changed == 1 {
            Ok(())
        } else {
            Err(BudgetNotificationStoreError::Unavailable)
        }
    }
}

fn is_unique_constraint(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.code == ErrorCode::ConstraintViolation
                && matches!(inner.extended_code, 1555 | 2067)
    )
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use crate::application::ports::budget_notification_store::BudgetNotificationStore;
    use crate::domain::budget::BudgetId;

    use super::*;

    #[test]
    fn claims_once_and_persists_delivery_status_across_reopen() {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(&path).expect("open database");
        database.migrate_to_latest().expect("migrate");
        database
            .connection()
            .execute(
                "INSERT INTO budgets (
                    name, metric, period, limit_value, currency, source_id,
                    enabled, created_at_ms, updated_at_ms
                 ) VALUES ('Daily tokens', 'tokens', 'daily', 1000, NULL,
                    NULL, 1, 100, 100)",
                [],
            )
            .expect("budget");
        database
            .connection()
            .execute(
                "INSERT INTO budget_thresholds (budget_id, threshold_bps, enabled)
                 VALUES (1, 8000, 1)",
                [],
            )
            .expect("threshold");
        let store = SqliteBudgetNotificationStore::new(database);
        let claim = claim();

        assert_eq!(
            store.claim(&claim).expect("claim"),
            BudgetNotificationClaimOutcome::Claimed
        );
        assert_eq!(
            store.claim(&claim).expect("duplicate"),
            BudgetNotificationClaimOutcome::AlreadyClaimed
        );
        store
            .set_status(&claim, BudgetNotificationStatus::Delivered)
            .expect("status");
        drop(store);

        let reopened = Database::open(path).expect("reopen");
        let status: String = reopened
            .connection()
            .query_row(
                "SELECT delivery_status FROM budget_notification_state",
                [],
                |row| row.get(0),
            )
            .expect("read status");
        assert_eq!(status, "delivered");
    }

    fn claim() -> BudgetNotificationClaim {
        BudgetNotificationClaim {
            budget_id: BudgetId::new(1).expect("id"),
            period_start_date: NaiveDate::from_ymd_opt(2026, 6, 19).expect("date"),
            aggregation_timezone: "Asia/Jakarta".to_owned(),
            threshold_basis_points: 8_000,
            observed_value: 850,
            notified_at_ms: 1_718_668_800_000,
            status: BudgetNotificationStatus::Failed,
        }
    }
}
