//! Observability / metrics API contracts.

pub mod post_metrics;

pub use post_metrics::{
    DATA_V1_METRICS_PATH, LtDataPostMetricsElement, LtDataPostMetricsReq,
    METRICS_PRIORITY_HEADER_VALUE,
};
