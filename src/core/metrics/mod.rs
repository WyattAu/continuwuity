mod instruments;
mod process;
mod tests;

use std::sync::atomic::AtomicU32;

use metrics_kit::{Encoder as KitEncoder, Registry};
pub use metrics_kit::{Format, Gauge as KitGauge};
use opentelemetry::metrics::Meter;
pub use opentelemetry::{KeyValue, metrics::AsyncInstrument};
pub use prometheus::TEXT_FORMAT as PROMETHEUS_CONTENT_TYPE;
use prometheus::{Encoder, TextEncoder};
use tokio::runtime;
use tokio_metrics::TaskMonitor;
#[cfg(tokio_unstable)]
use tokio_metrics::{RuntimeIntervals, RuntimeMonitor};

pub use self::instruments::{Instruments, duration_buckets};
use crate::{Err, Result};

pub struct Metrics {
	_runtime: Option<runtime::Handle>,

	runtime_metrics: Option<runtime::RuntimeMetrics>,

	task_monitor: Option<TaskMonitor>,

	#[cfg(tokio_unstable)]
	_runtime_monitor: Option<RuntimeMonitor>,

	#[cfg(tokio_unstable)]
	runtime_intervals: std::sync::Mutex<Option<RuntimeIntervals>>,

	// TODO: move stats
	pub requests_handle_active: AtomicU32,
	pub requests_handle_finished: AtomicU32,
	pub requests_panic: AtomicU32,

	pub meter: Meter,
	pub instruments: Instruments,
	registry: Option<prometheus::Registry>,
	kit_registry: Registry,
}

impl Metrics {
	#[must_use]
	pub fn new(
		runtime: Option<runtime::Handle>,
		meter: Meter,
		registry: Option<prometheus::Registry>,
	) -> Self {
		#[cfg(tokio_unstable)]
		let runtime_monitor = runtime.as_ref().map(RuntimeMonitor::new);

		#[cfg(tokio_unstable)]
		let runtime_intervals = runtime_monitor.as_ref().map(RuntimeMonitor::intervals);

		let runtime_metrics = runtime.as_ref().map(runtime::Handle::metrics);

		Self {
			_runtime: runtime.clone(),

			instruments: Instruments::new(&meter, runtime_metrics.clone()),
			meter,
			registry,
			kit_registry: Registry::with_max_series(8192),

			runtime_metrics,

			task_monitor: runtime.map(|_| TaskMonitor::new()),

			#[cfg(tokio_unstable)]
			_runtime_monitor: runtime_monitor,

			#[cfg(tokio_unstable)]
			runtime_intervals: std::sync::Mutex::new(runtime_intervals),

			requests_handle_active: AtomicU32::new(0),
			requests_handle_finished: AtomicU32::new(0),
			requests_panic: AtomicU32::new(0),
		}
	}

	#[cfg(tokio_unstable)]
	pub fn runtime_interval(&self) -> Option<tokio_metrics::RuntimeMetrics> {
		self.runtime_intervals
			.lock()
			.expect("locked")
			.as_mut()
			.map(Iterator::next)
			.expect("next interval")
	}

	pub fn render(&self) -> Result<String> {
		let Some(registry) = &self.registry else {
			return Err!("Prometheus metrics are not enabled");
		};

		let mut buf = Vec::new();
		TextEncoder::new()
			.encode(&registry.gather(), &mut buf)
			.map_err(|e| crate::err!("{e}"))?;

		Ok(String::from_utf8(buf)?)
	}

	/// Register a single-series gauge on the metrics-kit registry for the
	/// OpenMetrics exposition path. Returns `None` when the cardinality
	/// budget or the duplicate-series contract refuses the registration;
	/// the caller then proceeds without the kit mirror.
	pub fn kit_gauge(&self, name: &'static str, help: &'static str) -> Option<KitGauge> {
		self.kit_registry.gauge(name, help, &[]).ok()
	}

	/// Register a gauge with explicit labels on the metrics-kit registry.
	/// Same refusal semantics as [`Self::kit_gauge`].
	pub fn kit_gauge_labeled(
		&self,
		name: &'static str,
		help: &'static str,
		labels: &[(&str, &str)],
	) -> Option<KitGauge> {
		self.kit_registry.gauge(name, help, labels).ok()
	}

	/// OpenMetrics text exposition from the metrics-kit registry. Carries
	/// the single-series gauges only; labelled series remain on the
	/// Prometheus endpoint.
	pub fn render_openmetrics(&self) -> Result<String> {
		Ok(KitEncoder::new(Format::OpenMetrics).encode_to_string(&self.kit_registry))
	}

	#[inline]
	pub fn task_root(&self) -> Option<&TaskMonitor> {
		self.task_monitor.as_ref()
	}

	#[inline]
	pub fn num_workers(&self) -> usize {
		self.runtime_metrics()
			.map_or(0, runtime::RuntimeMetrics::num_workers)
	}

	#[inline]
	pub fn runtime_metrics(&self) -> Option<&runtime::RuntimeMetrics> {
		self.runtime_metrics.as_ref()
	}
}
