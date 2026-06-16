use std::sync::Arc;

use chrono::NaiveDate;
use thiserror::Error;

use crate::application::ports::clock::Clock;
use crate::application::ports::day_detail_store::{DayDetailStore, DayDetailStoreError};
use crate::application::usage::OverviewCost;
use crate::domain::source::SourceKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DayDetailModel {
    pub source: SourceKey,
    pub model: String,
    pub tokens: u64,
    pub cost: OverviewCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DayDetailReadModel {
    pub date: NaiveDate,
    pub total_tokens: u64,
    pub cost: OverviewCost,
    pub models: Vec<DayDetailModel>,
    pub as_of_ms: i64,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum DayDetailQueryError {
    #[error("day detail storage failed")]
    Storage(#[from] DayDetailStoreError),
}

pub(crate) struct DayDetailQuery {
    store: Arc<dyn DayDetailStore>,
    clock: Arc<dyn Clock>,
}

impl DayDetailQuery {
    pub(crate) fn new(store: Arc<dyn DayDetailStore>, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }

    pub(crate) fn get(
        &self,
        date: NaiveDate,
    ) -> Result<Option<DayDetailReadModel>, DayDetailQueryError> {
        let mut model = self.store.read_day_detail(date)?;
        if let Some(ref mut m) = model {
            m.as_of_ms = self.clock.now_epoch_ms();
        }
        Ok(model)
    }
}
