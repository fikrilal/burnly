use std::sync::Arc;

use chrono::NaiveDate;
use thiserror::Error;

use crate::application::ports::day_detail_store::{DayDetailStore, DayDetailStoreError};
use crate::application::usage::{OverviewCost, OverviewSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DayDetailReadModel {
    pub date: NaiveDate,
    pub total_tokens: u64,
    pub cost: OverviewCost,
    pub sources: Vec<OverviewSource>, // Reusing OverviewSource for simplicity
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum DayDetailQueryError {
    #[error("day detail storage failed")]
    Storage(#[from] DayDetailStoreError),
}

pub(crate) struct DayDetailQuery {
    store: Arc<dyn DayDetailStore>,
}

impl DayDetailQuery {
    pub(crate) fn new(store: Arc<dyn DayDetailStore>) -> Self {
        Self { store }
    }

    pub(crate) fn get(
        &self,
        date: NaiveDate,
    ) -> Result<Option<DayDetailReadModel>, DayDetailQueryError> {
        Ok(self.store.read_day_detail(date)?)
    }
}
