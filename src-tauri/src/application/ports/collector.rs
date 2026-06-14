use crate::application::collection::{
    CollectionRequest, CollectionResult, CollectorDescriptor, CollectorFailure, DetectionRequest,
    DetectionResult,
};

pub(crate) trait CancellationSignal: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

#[allow(
    dead_code,
    reason = "Collector ports are implemented before refresh orchestration wires runtime calls"
)]
pub(crate) trait Collector: Send + Sync {
    fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure>;

    fn detect(
        &self,
        request: DetectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<DetectionResult, CollectorFailure>;

    fn collect(
        &self,
        request: CollectionRequest,
        cancellation: &dyn CancellationSignal,
    ) -> Result<CollectionResult, CollectorFailure>;
}
