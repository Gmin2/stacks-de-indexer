//! Prometheus metrics for observability.
//!
//! Exposed at `GET /metrics` in Prometheus exposition format.

use prometheus::{
    Encoder, Gauge, Histogram, HistogramOpts, IntCounter, IntCounterVec, Opts, Registry,
    TextEncoder,
};

/// Collection of Prometheus metrics for the indexer.
pub struct Metrics {
    registry: Registry,
    /// Last processed block height.
    pub last_block_height: Gauge,
    /// Total blocks processed since startup.
    pub blocks_processed_total: IntCounter,
    /// Total matched events, labeled by target table.
    pub events_matched_total: IntCounterVec,
    /// Histogram of block processing duration.
    pub block_processing_duration: Histogram,
    /// Total chain reorganizations detected.
    pub reorgs_detected_total: IntCounter,
    /// Current SQLite database file size.
    pub storage_size_bytes: Gauge,
}

impl Metrics {
    /// Create a new metrics registry with all counters and gauges.
    pub fn new() -> Self {
        let registry = Registry::new();

        let last_block_height =
            Gauge::new("indexer_last_block_height", "Last processed block height").unwrap();
        registry.register(Box::new(last_block_height.clone())).unwrap();

        let blocks_processed_total =
            IntCounter::new("indexer_blocks_processed_total", "Total blocks processed").unwrap();
        registry.register(Box::new(blocks_processed_total.clone())).unwrap();

        let events_matched_total = IntCounterVec::new(
            Opts::new("indexer_events_matched_total", "Total matched events"),
            &["table"],
        )
        .unwrap();
        registry.register(Box::new(events_matched_total.clone())).unwrap();

        let block_processing_duration = Histogram::with_opts(
            HistogramOpts::new(
                "indexer_block_processing_duration_seconds",
                "Time to process a single block",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
        )
        .unwrap();
        registry.register(Box::new(block_processing_duration.clone())).unwrap();

        let reorgs_detected_total =
            IntCounter::new("indexer_reorgs_detected_total", "Total reorgs detected").unwrap();
        registry.register(Box::new(reorgs_detected_total.clone())).unwrap();

        let storage_size_bytes =
            Gauge::new("indexer_storage_size_bytes", "SQLite file size in bytes").unwrap();
        registry.register(Box::new(storage_size_bytes.clone())).unwrap();

        Self {
            registry,
            last_block_height,
            blocks_processed_total,
            events_matched_total,
            block_processing_duration,
            reorgs_detected_total,
            storage_size_bytes,
        }
    }

    /// Render all metrics in Prometheus text exposition format.
    pub fn render(&self) -> String {
        let encoder = TextEncoder::new();
        let families = self.registry.gather();
        let mut buf = Vec::new();
        encoder.encode(&families, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }
}
