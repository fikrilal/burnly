use std::sync::Arc;

use chrono::NaiveDate;

use crate::application::budget_evaluation::{
    BudgetEvaluationError, BudgetEvaluationService, BudgetMetric, BudgetThresholdDecision,
};
use crate::application::ports::budget_notification_store::{
    BudgetNotificationClaimOutcome, BudgetNotificationStore, BudgetNotificationStoreError,
};
use crate::application::ports::notification::{
    NotificationDeliveryOutcome, NotificationMessage, NotificationPermission, NotificationPort,
};
use crate::application::ports::settings_store::{SettingsStore, SettingsStoreError};
use crate::domain::budget::BudgetId;

pub(crate) struct BudgetNotificationService {
    evaluator: BudgetEvaluationService,
    settings_store: Arc<dyn SettingsStore>,
    notification_store: Arc<dyn BudgetNotificationStore>,
    notifications: Arc<dyn NotificationPort>,
}

impl BudgetNotificationService {
    pub(crate) fn new(
        evaluator: BudgetEvaluationService,
        settings_store: Arc<dyn SettingsStore>,
        notification_store: Arc<dyn BudgetNotificationStore>,
        notifications: Arc<dyn NotificationPort>,
    ) -> Self {
        Self {
            evaluator,
            settings_store,
            notification_store,
            notifications,
        }
    }

    pub(crate) fn evaluate_and_deliver(
        &self,
        aggregation_timezone: &str,
        now_epoch_ms: i64,
    ) -> Result<(), BudgetEvaluationError> {
        let report = self
            .evaluator
            .evaluate(aggregation_timezone, now_epoch_ms)?;
        let settings = self.settings_store.get().map_err(map_settings_error)?;
        let capability = self.notifications.capability();

        for decision in &report.threshold_decisions {
            let eligible = settings.settings().notifications_enabled()
                && capability.supported
                && capability.permission == NotificationPermission::Granted;
            self.process_decision(decision, now_epoch_ms, eligible)?;
        }

        Ok(())
    }

    fn process_decision(
        &self,
        decision: &BudgetThresholdDecision,
        now_epoch_ms: i64,
        eligible: bool,
    ) -> Result<(), BudgetEvaluationError> {
        let initial_status = if eligible {
            BudgetNotificationStatus::Failed
        } else {
            BudgetNotificationStatus::Suppressed
        };
        let claim = BudgetNotificationClaim::from_decision(decision, now_epoch_ms, initial_status);
        if self
            .notification_store
            .claim(&claim)
            .map_err(map_notification_store_error)?
            == BudgetNotificationClaimOutcome::AlreadyClaimed
        {
            return Ok(());
        }
        if !eligible {
            return Ok(());
        }

        let status = match self.notifications.deliver(&message(decision)) {
            NotificationDeliveryOutcome::Delivered => BudgetNotificationStatus::Delivered,
            NotificationDeliveryOutcome::Failed => BudgetNotificationStatus::Failed,
        };
        self.notification_store
            .set_status(&claim, status)
            .map_err(map_notification_store_error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetNotificationStatus {
    Delivered,
    Failed,
    Suppressed,
}

impl BudgetNotificationStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Failed => "failed",
            Self::Suppressed => "suppressed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BudgetNotificationClaim {
    pub budget_id: BudgetId,
    pub period_start_date: NaiveDate,
    pub aggregation_timezone: String,
    pub threshold_basis_points: u32,
    pub observed_value: u64,
    pub notified_at_ms: i64,
    pub status: BudgetNotificationStatus,
}

impl BudgetNotificationClaim {
    fn from_decision(
        decision: &BudgetThresholdDecision,
        notified_at_ms: i64,
        status: BudgetNotificationStatus,
    ) -> Self {
        Self {
            budget_id: decision.budget_id,
            period_start_date: decision.period.start_date(),
            aggregation_timezone: decision.period.aggregation_timezone().to_owned(),
            threshold_basis_points: decision.threshold_basis_points,
            observed_value: decision.observed_value,
            notified_at_ms,
            status,
        }
    }
}

fn message(decision: &BudgetThresholdDecision) -> NotificationMessage {
    let threshold = format_percent(decision.threshold_basis_points);
    let observed = match decision.metric {
        BudgetMetric::Tokens => format!("{} tokens used", decision.observed_value),
        BudgetMetric::Cost => format!("{} cost micros used", decision.observed_value),
    };
    NotificationMessage {
        title: format!("{} reached {threshold}", decision.budget_name),
        body: format!("{observed} in the current budget period."),
    }
}

fn format_percent(basis_points: u32) -> String {
    if basis_points.is_multiple_of(100) {
        format!("{}%", basis_points / 100)
    } else {
        format!("{}.{:02}%", basis_points / 100, basis_points % 100)
    }
}

fn map_settings_error(_error: SettingsStoreError) -> BudgetEvaluationError {
    BudgetEvaluationError::StorageUnavailable
}

fn map_notification_store_error(_error: BudgetNotificationStoreError) -> BudgetEvaluationError {
    BudgetEvaluationError::StorageUnavailable
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::application::budget_evaluation::{
        BudgetCostCompleteness, BudgetUsageAggregate, BudgetUsageCost, BudgetUsageRequest,
    };
    use crate::application::ports::budget_store::{BudgetStore, BudgetStoreError};
    use crate::application::ports::budget_usage_store::{BudgetUsageStore, BudgetUsageStoreError};
    use crate::application::ports::notification::NotificationCapability;
    use crate::application::ports::settings_store::{
        ProjectPathRetentionResult, SettingsStoreError,
    };
    use crate::domain::budget::{
        Budget, BudgetDefinition, BudgetLimit, BudgetPeriod, BudgetScope, BudgetThreshold,
    };
    use crate::domain::settings::{Settings, SettingsDocument};

    use super::*;

    struct FixedBudgetStore {
        budget: Budget,
    }

    impl BudgetStore for FixedBudgetStore {
        fn create(
            &self,
            _definition: &BudgetDefinition,
            _now_epoch_ms: i64,
        ) -> Result<Budget, BudgetStoreError> {
            Err(BudgetStoreError::Unavailable)
        }

        fn get(&self, _id: BudgetId) -> Result<Budget, BudgetStoreError> {
            Ok(self.budget.clone())
        }

        fn list(&self) -> Result<Vec<Budget>, BudgetStoreError> {
            Ok(vec![self.budget.clone()])
        }

        fn replace(
            &self,
            _id: BudgetId,
            _expected_revision: i64,
            _definition: &BudgetDefinition,
            _now_epoch_ms: i64,
        ) -> Result<Budget, BudgetStoreError> {
            Err(BudgetStoreError::Unavailable)
        }

        fn set_enabled(
            &self,
            _id: BudgetId,
            _expected_revision: i64,
            _enabled: bool,
            _now_epoch_ms: i64,
        ) -> Result<Budget, BudgetStoreError> {
            Err(BudgetStoreError::Unavailable)
        }

        fn delete(&self, _id: BudgetId, _expected_revision: i64) -> Result<(), BudgetStoreError> {
            Err(BudgetStoreError::Unavailable)
        }
    }

    struct FixedUsageStore;

    impl BudgetUsageStore for FixedUsageStore {
        fn aggregate_budget_usage(
            &self,
            _request: &BudgetUsageRequest,
        ) -> Result<BudgetUsageAggregate, BudgetUsageStoreError> {
            Ok(BudgetUsageAggregate {
                total_tokens: 850,
                active_days: 1,
                cost: BudgetUsageCost {
                    amount_micros: None,
                    currency: None,
                    completeness: BudgetCostCompleteness::Unavailable,
                    unavailable_days: 1,
                },
            })
        }
    }

    struct FixedSettingsStore {
        enabled: bool,
    }

    impl SettingsStore for FixedSettingsStore {
        fn get(&self) -> Result<SettingsDocument, SettingsStoreError> {
            Ok(SettingsDocument::new(
                Settings::new(
                    "UTC".to_owned(),
                    false,
                    15,
                    false,
                    "quit",
                    self.enabled,
                    false,
                )
                .expect("settings"),
                1,
            )
            .expect("document"))
        }

        fn replace(
            &self,
            _expected_revision: i64,
            _settings: &Settings,
            _updated_at_ms: i64,
        ) -> Result<SettingsDocument, SettingsStoreError> {
            Err(SettingsStoreError::Unavailable)
        }

        fn replace_project_path_retention(
            &self,
            _expected_revision: i64,
            _retain_paths: bool,
            _updated_at_ms: i64,
        ) -> Result<ProjectPathRetentionResult, SettingsStoreError> {
            Err(SettingsStoreError::Unavailable)
        }
    }

    #[derive(Default)]
    struct MemoryNotificationStore {
        claims: Mutex<Vec<BudgetNotificationClaim>>,
    }

    impl BudgetNotificationStore for MemoryNotificationStore {
        fn claim(
            &self,
            claim: &BudgetNotificationClaim,
        ) -> Result<BudgetNotificationClaimOutcome, BudgetNotificationStoreError> {
            let mut claims = self.claims.lock().expect("claims lock");
            if claims.iter().any(|existing| same_identity(existing, claim)) {
                return Ok(BudgetNotificationClaimOutcome::AlreadyClaimed);
            }
            claims.push(claim.clone());
            Ok(BudgetNotificationClaimOutcome::Claimed)
        }

        fn set_status(
            &self,
            claim: &BudgetNotificationClaim,
            status: BudgetNotificationStatus,
        ) -> Result<(), BudgetNotificationStoreError> {
            let mut claims = self.claims.lock().expect("claims lock");
            let stored = claims
                .iter_mut()
                .find(|stored| same_identity(stored, claim))
                .expect("claimed notification");
            stored.status = status;
            Ok(())
        }
    }

    struct RecordingNotificationPort {
        capability: NotificationCapability,
        delivery: NotificationDeliveryOutcome,
        messages: Mutex<Vec<NotificationMessage>>,
    }

    impl NotificationPort for RecordingNotificationPort {
        fn capability(&self) -> NotificationCapability {
            self.capability
        }

        fn request_permission(&self) -> NotificationPermission {
            self.capability.permission
        }

        fn deliver(&self, message: &NotificationMessage) -> NotificationDeliveryOutcome {
            self.messages
                .lock()
                .expect("messages lock")
                .push(message.clone());
            self.delivery
        }
    }

    #[test]
    fn delivers_once_and_records_delivered_status() {
        let notification_store = Arc::new(MemoryNotificationStore::default());
        let notifications = Arc::new(port(
            true,
            NotificationPermission::Granted,
            NotificationDeliveryOutcome::Delivered,
        ));
        let service = service(true, notification_store.clone(), notifications.clone());

        service
            .evaluate_and_deliver("UTC", 1_718_668_800_000)
            .expect("first");
        service
            .evaluate_and_deliver("UTC", 1_718_668_800_000)
            .expect("duplicate");

        assert_eq!(notifications.messages.lock().expect("messages").len(), 1);
        let claims = notification_store.claims.lock().expect("claims");
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].status, BudgetNotificationStatus::Delivered);
    }

    #[test]
    fn disabled_unsupported_and_denied_notifications_are_suppressed() {
        for (enabled, supported, permission) in [
            (false, true, NotificationPermission::Granted),
            (true, false, NotificationPermission::Unknown),
            (true, true, NotificationPermission::Denied),
        ] {
            let notification_store = Arc::new(MemoryNotificationStore::default());
            let notifications = Arc::new(port(
                supported,
                permission,
                NotificationDeliveryOutcome::Delivered,
            ));
            service(enabled, notification_store.clone(), notifications.clone())
                .evaluate_and_deliver("UTC", 1_718_668_800_000)
                .expect("evaluation");

            assert!(notifications.messages.lock().expect("messages").is_empty());
            assert_eq!(
                notification_store.claims.lock().expect("claims")[0].status,
                BudgetNotificationStatus::Suppressed
            );
        }
    }

    #[test]
    fn adapter_failure_is_recorded_without_failing_evaluation() {
        let notification_store = Arc::new(MemoryNotificationStore::default());
        let notifications = Arc::new(port(
            true,
            NotificationPermission::Granted,
            NotificationDeliveryOutcome::Failed,
        ));

        service(true, notification_store.clone(), notifications)
            .evaluate_and_deliver("UTC", 1_718_668_800_000)
            .expect("failure is isolated");

        assert_eq!(
            notification_store.claims.lock().expect("claims")[0].status,
            BudgetNotificationStatus::Failed
        );
    }

    fn service(
        enabled: bool,
        notification_store: Arc<MemoryNotificationStore>,
        notifications: Arc<RecordingNotificationPort>,
    ) -> BudgetNotificationService {
        let budget = Budget::new(
            BudgetId::new(1).expect("id"),
            1,
            BudgetDefinition::new(
                "Daily tokens",
                BudgetLimit::tokens(1_000).expect("limit"),
                BudgetPeriod::Daily,
                BudgetScope::Global,
                true,
                vec![BudgetThreshold::new(8_000, true).expect("threshold")],
            )
            .expect("definition"),
        )
        .expect("budget");
        BudgetNotificationService::new(
            BudgetEvaluationService::new(
                Arc::new(FixedBudgetStore { budget }),
                Arc::new(FixedUsageStore),
            ),
            Arc::new(FixedSettingsStore { enabled }),
            notification_store,
            notifications,
        )
    }

    fn port(
        supported: bool,
        permission: NotificationPermission,
        delivery: NotificationDeliveryOutcome,
    ) -> RecordingNotificationPort {
        RecordingNotificationPort {
            capability: NotificationCapability {
                supported,
                permission,
            },
            delivery,
            messages: Mutex::new(Vec::new()),
        }
    }

    fn same_identity(left: &BudgetNotificationClaim, right: &BudgetNotificationClaim) -> bool {
        left.budget_id == right.budget_id
            && left.period_start_date == right.period_start_date
            && left.aggregation_timezone == right.aggregation_timezone
            && left.threshold_basis_points == right.threshold_basis_points
    }
}
