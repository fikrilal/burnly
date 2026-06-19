use crate::application::budget_notifications::{BudgetNotificationClaim, BudgetNotificationStatus};

pub(crate) trait BudgetNotificationStore: Send + Sync {
    fn claim(
        &self,
        claim: &BudgetNotificationClaim,
    ) -> Result<BudgetNotificationClaimOutcome, BudgetNotificationStoreError>;

    fn set_status(
        &self,
        claim: &BudgetNotificationClaim,
        status: BudgetNotificationStatus,
    ) -> Result<(), BudgetNotificationStoreError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetNotificationClaimOutcome {
    Claimed,
    AlreadyClaimed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetNotificationStoreError {
    Unavailable,
}
