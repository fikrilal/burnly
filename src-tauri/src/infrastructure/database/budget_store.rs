use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::application::ports::budget_store::{BudgetStore, BudgetStoreError};
use crate::domain::budget::{
    Budget, BudgetDefinition, BudgetId, BudgetLimit, BudgetPeriod, BudgetScope, BudgetThreshold,
};
use crate::domain::usage::CurrencyCode;

use super::Database;

pub(crate) struct SqliteBudgetStore {
    database: Mutex<Database>,
}

impl SqliteBudgetStore {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }
}

impl BudgetStore for SqliteBudgetStore {
    fn create(
        &self,
        definition: &BudgetDefinition,
        now_epoch_ms: i64,
    ) -> Result<Budget, BudgetStoreError> {
        let mut database = self.lock_database()?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| BudgetStoreError::Unavailable)?;
        validate_source(&transaction, definition.scope())?;
        let stored = StoredDefinition::from_domain(definition)?;
        transaction
            .execute(
                "INSERT INTO budgets (
                    name, metric, period, limit_value, currency, source_id,
                    enabled, created_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    definition.name(),
                    stored.metric,
                    definition.period().as_str(),
                    stored.limit_value,
                    stored.currency,
                    source_id(definition.scope()),
                    definition.enabled(),
                    now_epoch_ms,
                ],
            )
            .map_err(|_| BudgetStoreError::Unavailable)?;
        let id = BudgetId::new(transaction.last_insert_rowid())
            .map_err(|_| BudgetStoreError::InvalidStoredValue)?;
        sync_thresholds(&transaction, id, definition.thresholds())?;
        transaction
            .commit()
            .map_err(|_| BudgetStoreError::Unavailable)?;
        read_budget(database.connection(), id)
    }

    fn get(&self, id: BudgetId) -> Result<Budget, BudgetStoreError> {
        let database = self.lock_database()?;
        read_budget(database.connection(), id)
    }

    fn list(&self) -> Result<Vec<Budget>, BudgetStoreError> {
        let database = self.lock_database()?;
        let mut statement = database
            .connection()
            .prepare("SELECT id FROM budgets ORDER BY id")
            .map_err(|_| BudgetStoreError::Unavailable)?;
        let ids = statement
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(|_| BudgetStoreError::Unavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| BudgetStoreError::Unavailable)?;
        ids.into_iter()
            .map(|id| {
                BudgetId::new(id)
                    .map_err(|_| BudgetStoreError::InvalidStoredValue)
                    .and_then(|id| read_budget(database.connection(), id))
            })
            .collect()
    }

    fn replace(
        &self,
        id: BudgetId,
        expected_revision: i64,
        definition: &BudgetDefinition,
        now_epoch_ms: i64,
    ) -> Result<Budget, BudgetStoreError> {
        let mut database = self.lock_database()?;
        let transaction = database
            .connection_mut()
            .transaction()
            .map_err(|_| BudgetStoreError::Unavailable)?;
        require_revision(&transaction, id, expected_revision)?;
        validate_source(&transaction, definition.scope())?;
        let stored = StoredDefinition::from_domain(definition)?;
        let changed = transaction
            .execute(
                "UPDATE budgets SET
                    name = ?1,
                    metric = ?2,
                    period = ?3,
                    limit_value = ?4,
                    currency = ?5,
                    source_id = ?6,
                    enabled = ?7,
                    updated_at_ms = ?8,
                    revision = revision + 1
                 WHERE id = ?9 AND revision = ?10",
                params![
                    definition.name(),
                    stored.metric,
                    definition.period().as_str(),
                    stored.limit_value,
                    stored.currency,
                    source_id(definition.scope()),
                    definition.enabled(),
                    now_epoch_ms,
                    id.value(),
                    expected_revision,
                ],
            )
            .map_err(|_| BudgetStoreError::Unavailable)?;
        if changed != 1 {
            return Err(BudgetStoreError::Conflict);
        }
        sync_thresholds(&transaction, id, definition.thresholds())?;
        transaction
            .commit()
            .map_err(|_| BudgetStoreError::Unavailable)?;
        read_budget(database.connection(), id)
    }

    fn set_enabled(
        &self,
        id: BudgetId,
        expected_revision: i64,
        enabled: bool,
        now_epoch_ms: i64,
    ) -> Result<Budget, BudgetStoreError> {
        let database = self.lock_database()?;
        require_revision(database.connection(), id, expected_revision)?;
        let changed = database
            .connection()
            .execute(
                "UPDATE budgets
                 SET enabled = ?1, updated_at_ms = ?2, revision = revision + 1
                 WHERE id = ?3 AND revision = ?4",
                params![enabled, now_epoch_ms, id.value(), expected_revision],
            )
            .map_err(|_| BudgetStoreError::Unavailable)?;
        if changed != 1 {
            return Err(BudgetStoreError::Conflict);
        }
        read_budget(database.connection(), id)
    }

    fn delete(&self, id: BudgetId, expected_revision: i64) -> Result<(), BudgetStoreError> {
        let database = self.lock_database()?;
        require_revision(database.connection(), id, expected_revision)?;
        let changed = database
            .connection()
            .execute(
                "DELETE FROM budgets WHERE id = ?1 AND revision = ?2",
                params![id.value(), expected_revision],
            )
            .map_err(|_| BudgetStoreError::Unavailable)?;
        if changed != 1 {
            return Err(BudgetStoreError::Conflict);
        }
        Ok(())
    }
}

impl SqliteBudgetStore {
    fn lock_database(&self) -> Result<std::sync::MutexGuard<'_, Database>, BudgetStoreError> {
        self.database
            .lock()
            .map_err(|_| BudgetStoreError::Unavailable)
    }
}

struct StoredDefinition<'a> {
    metric: &'static str,
    limit_value: i64,
    currency: Option<&'a str>,
}

impl<'a> StoredDefinition<'a> {
    fn from_domain(definition: &'a BudgetDefinition) -> Result<Self, BudgetStoreError> {
        match definition.limit() {
            BudgetLimit::Tokens(value) => Ok(Self {
                metric: "tokens",
                limit_value: i64::try_from(*value)
                    .map_err(|_| BudgetStoreError::InvalidStoredValue)?,
                currency: None,
            }),
            BudgetLimit::CostMicros {
                amount_micros,
                currency,
            } => Ok(Self {
                metric: "cost",
                limit_value: i64::try_from(*amount_micros)
                    .map_err(|_| BudgetStoreError::InvalidStoredValue)?,
                currency: Some(currency.as_str()),
            }),
        }
    }
}

fn read_budget(connection: &Connection, id: BudgetId) -> Result<Budget, BudgetStoreError> {
    let row = connection
        .query_row(
            "SELECT name, metric, period, limit_value, currency, source_id,
                    enabled, revision
             FROM budgets
             WHERE id = ?1",
            [id.value()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, bool>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            },
        )
        .optional()
        .map_err(|_| BudgetStoreError::Unavailable)?
        .ok_or(BudgetStoreError::NotFound)?;
    let limit_value = u64::try_from(row.3).map_err(|_| BudgetStoreError::InvalidStoredValue)?;
    let limit = match (row.1.as_str(), row.4) {
        ("tokens", None) => BudgetLimit::tokens(limit_value),
        ("cost", Some(currency)) => BudgetLimit::cost_micros(
            limit_value,
            CurrencyCode::new(currency).map_err(|_| BudgetStoreError::InvalidStoredValue)?,
        ),
        _ => return Err(BudgetStoreError::InvalidStoredValue),
    }
    .map_err(|_| BudgetStoreError::InvalidStoredValue)?;
    let scope = match row.5 {
        Some(source_id) => {
            BudgetScope::source(source_id).map_err(|_| BudgetStoreError::InvalidStoredValue)?
        }
        None => BudgetScope::Global,
    };
    let definition = BudgetDefinition::new(
        row.0,
        limit,
        BudgetPeriod::parse(&row.2).map_err(|_| BudgetStoreError::InvalidStoredValue)?,
        scope,
        row.6,
        read_thresholds(connection, id)?,
    )
    .map_err(|_| BudgetStoreError::InvalidStoredValue)?;
    Budget::new(id, row.7, definition).map_err(|_| BudgetStoreError::InvalidStoredValue)
}

fn read_thresholds(
    connection: &Connection,
    id: BudgetId,
) -> Result<Vec<BudgetThreshold>, BudgetStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT threshold_bps, enabled
             FROM budget_thresholds
             WHERE budget_id = ?1
             ORDER BY threshold_bps",
        )
        .map_err(|_| BudgetStoreError::Unavailable)?;
    let thresholds = statement
        .query_map([id.value()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, bool>(1)?))
        })
        .map_err(|_| BudgetStoreError::Unavailable)?
        .map(|row| {
            let (basis_points, enabled) = row.map_err(|_| BudgetStoreError::Unavailable)?;
            let basis_points =
                u32::try_from(basis_points).map_err(|_| BudgetStoreError::InvalidStoredValue)?;
            BudgetThreshold::new(basis_points, enabled)
                .map_err(|_| BudgetStoreError::InvalidStoredValue)
        })
        .collect();
    thresholds
}

fn sync_thresholds(
    transaction: &Transaction<'_>,
    id: BudgetId,
    thresholds: &[BudgetThreshold],
) -> Result<(), BudgetStoreError> {
    let retained = thresholds
        .iter()
        .map(|threshold| i64::from(threshold.basis_points()))
        .collect::<Vec<_>>();
    if retained.is_empty() {
        transaction
            .execute(
                "DELETE FROM budget_thresholds WHERE budget_id = ?1",
                [id.value()],
            )
            .map_err(|_| BudgetStoreError::Unavailable)?;
    } else {
        let placeholders = (0..retained.len())
            .map(|index| format!("?{}", index + 2))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "DELETE FROM budget_thresholds
             WHERE budget_id = ?1 AND threshold_bps NOT IN ({placeholders})"
        );
        let mut values = Vec::with_capacity(retained.len() + 1);
        values.push(id.value());
        values.extend(retained);
        transaction
            .execute(&sql, rusqlite::params_from_iter(values))
            .map_err(|_| BudgetStoreError::Unavailable)?;
    }

    for threshold in thresholds {
        transaction
            .execute(
                "INSERT INTO budget_thresholds (budget_id, threshold_bps, enabled)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(budget_id, threshold_bps)
                 DO UPDATE SET enabled = excluded.enabled",
                params![id.value(), threshold.basis_points(), threshold.enabled()],
            )
            .map_err(|_| BudgetStoreError::Unavailable)?;
    }
    Ok(())
}

fn require_revision(
    connection: &Connection,
    id: BudgetId,
    expected_revision: i64,
) -> Result<(), BudgetStoreError> {
    let revision = connection
        .query_row(
            "SELECT revision FROM budgets WHERE id = ?1",
            [id.value()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|_| BudgetStoreError::Unavailable)?
        .ok_or(BudgetStoreError::NotFound)?;
    if revision != expected_revision {
        return Err(BudgetStoreError::Conflict);
    }
    Ok(())
}

fn validate_source(connection: &Connection, scope: BudgetScope) -> Result<(), BudgetStoreError> {
    let BudgetScope::Source(source_id) = scope else {
        return Ok(());
    };
    let exists = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sources WHERE id = ?1)",
            [source_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|_| BudgetStoreError::Unavailable)?;
    if !exists {
        return Err(BudgetStoreError::UnknownSource);
    }
    Ok(())
}

const fn source_id(scope: BudgetScope) -> Option<i64> {
    match scope {
        BudgetScope::Global => None,
        BudgetScope::Source(id) => Some(id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> SqliteBudgetStore {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let path = directory.keep().join("burnly.sqlite3");
        let mut database = Database::open(path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        SqliteBudgetStore::new(database)
    }

    fn definition(scope: BudgetScope, thresholds: &[u32]) -> BudgetDefinition {
        BudgetDefinition::new(
            "Monthly cost",
            BudgetLimit::cost_micros(25_000_000, CurrencyCode::new("USD").expect("currency"))
                .expect("limit"),
            BudgetPeriod::Monthly,
            scope,
            true,
            thresholds
                .iter()
                .map(|value| BudgetThreshold::new(*value, true).expect("threshold"))
                .collect(),
        )
        .expect("definition")
    }

    #[test]
    fn creates_reads_lists_and_survives_restart() {
        let directory = tempfile::TempDir::new().expect("temporary directory");
        let path = directory.path().join("burnly.sqlite3");
        let mut database = Database::open(&path).expect("open database");
        database.migrate_to_latest().expect("migrate database");
        let store = SqliteBudgetStore::new(database);
        let created = store
            .create(&definition(BudgetScope::Global, &[10_000, 8_000]), 100)
            .expect("create budget");
        assert_eq!(created.revision(), 1);
        assert_eq!(
            created
                .definition()
                .thresholds()
                .iter()
                .map(|threshold| threshold.basis_points())
                .collect::<Vec<_>>(),
            vec![8_000, 10_000]
        );
        drop(store);

        let reopened = SqliteBudgetStore::new(Database::open(path).expect("reopen database"));
        assert_eq!(reopened.get(created.id()).expect("get"), created);
        assert_eq!(reopened.list().expect("list"), vec![created]);
    }

    #[test]
    fn rejects_unknown_source_and_rolls_back_create() {
        let store = store();
        let result = store.create(
            &definition(BudgetScope::source(99).expect("scope"), &[8_000]),
            100,
        );

        assert_eq!(result, Err(BudgetStoreError::UnknownSource));
        assert!(store.list().expect("list").is_empty());
    }

    #[test]
    fn source_scope_requires_and_preserves_an_existing_source() {
        let store = store();
        insert_source(&store, 7);
        let scope = BudgetScope::source(7).expect("scope");

        let created = store
            .create(&definition(scope, &[8_000]), 100)
            .expect("create source budget");

        assert_eq!(created.definition().scope(), scope);
        assert_eq!(
            store.replace(
                created.id(),
                1,
                &definition(BudgetScope::source(99).expect("scope"), &[9_000]),
                200,
            ),
            Err(BudgetStoreError::UnknownSource)
        );
        assert_eq!(store.get(created.id()).expect("get"), created);
    }

    #[test]
    fn replacement_checks_revision_and_reconciles_thresholds() {
        let store = store();
        let created = store
            .create(&definition(BudgetScope::Global, &[8_000, 10_000]), 100)
            .expect("create");
        insert_notification(&store, created.id(), 8_000);

        let updated = store
            .replace(
                created.id(),
                1,
                &definition(BudgetScope::Global, &[8_000, 9_000]),
                200,
            )
            .expect("replace");

        assert_eq!(updated.revision(), 2);
        assert_eq!(threshold_values(&updated), vec![8_000, 9_000]);
        assert_eq!(notification_thresholds(&store), vec![8_000]);
        assert_eq!(
            store.replace(
                created.id(),
                1,
                &definition(BudgetScope::Global, &[10_000]),
                300,
            ),
            Err(BudgetStoreError::Conflict)
        );
        assert_eq!(
            threshold_values(&store.get(created.id()).expect("get")),
            vec![8_000, 9_000]
        );
    }

    #[test]
    fn enable_disable_and_delete_require_current_revision() {
        let store = store();
        let created = store
            .create(&definition(BudgetScope::Global, &[8_000]), 100)
            .expect("create");
        let disabled = store
            .set_enabled(created.id(), 1, false, 200)
            .expect("disable");
        assert!(!disabled.definition().enabled());
        assert_eq!(disabled.revision(), 2);
        assert_eq!(
            store.delete(created.id(), 1),
            Err(BudgetStoreError::Conflict)
        );
        store.delete(created.id(), 2).expect("delete");
        assert_eq!(store.get(created.id()), Err(BudgetStoreError::NotFound));
    }

    #[test]
    fn deleting_budget_cascades_thresholds_and_notification_state() {
        let store = store();
        let created = store
            .create(&definition(BudgetScope::Global, &[8_000]), 100)
            .expect("create");
        insert_notification(&store, created.id(), 8_000);

        store.delete(created.id(), 1).expect("delete");

        let database = store.database.lock().expect("database lock");
        for table in ["budget_thresholds", "budget_notification_state"] {
            let count: i64 = database
                .connection()
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("count rows");
            assert_eq!(count, 0);
        }
    }

    fn insert_notification(store: &SqliteBudgetStore, id: BudgetId, threshold_bps: u32) {
        let database = store.database.lock().expect("database lock");
        database
            .connection()
            .execute(
                "INSERT INTO budget_notification_state (
                    budget_id, period_start_date, aggregation_timezone,
                    threshold_bps, observed_value, notified_at_ms, delivery_status
                 ) VALUES (?1, '2026-06-01', 'UTC', ?2, 100, 100, 'delivered')",
                params![id.value(), threshold_bps],
            )
            .expect("insert notification");
    }

    fn insert_source(store: &SqliteBudgetStore, id: i64) {
        let database = store.database.lock().expect("database lock");
        database
            .connection()
            .execute(
                "INSERT INTO sources (
                    id, source_key, display_name, enabled, detection_state,
                    created_at_ms, updated_at_ms
                 ) VALUES (?1, 'test-source', 'Test Source', 1, 'available', 0, 0)",
                [id],
            )
            .expect("insert source");
    }

    fn threshold_values(budget: &Budget) -> Vec<u32> {
        budget
            .definition()
            .thresholds()
            .iter()
            .map(|threshold| threshold.basis_points())
            .collect()
    }

    fn notification_thresholds(store: &SqliteBudgetStore) -> Vec<u32> {
        let database = store.database.lock().expect("database lock");
        let mut statement = database
            .connection()
            .prepare(
                "SELECT threshold_bps
                 FROM budget_notification_state
                 ORDER BY threshold_bps",
            )
            .expect("prepare query");
        statement
            .query_map([], |row| row.get::<_, u32>(0))
            .expect("query rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect rows")
    }
}
