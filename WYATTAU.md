# WyattAu fork notes: crate dogfooding program

This fork of continuwuity exists to run the WyattAu crate estate inside a
production-grade Matrix homeserver. Upstream is
[continuwuity](https://forgejo.ellis.link/continuwuation/continuwuity)
(GitHub mirror: [continuwuity/continuwuity](https://github.com/continuwuity/continuwuity));
this fork tracks upstream and carries a small number of estate-crate
integrations on top.

## Rules

1. Upstream first: generic improvements go upstream as PRs. This fork
   carries only estate-specific wiring.
2. Estate crates are pinned exactly (`=x.y.z`), matching the
   [estate-integration](https://github.com/WyattAu/estate-integration)
   convention.
3. Every integration lands behind an explicit route or feature so the
   stock behavior remains the default.

## Phase 1: metrics-kit (landed)

`conduwuit_core::metrics` gained a parallel
[metrics-kit](https://github.com/WyattAu/metrics-kit) registry
(pinned `=0.2.0`, `openmetrics` feature, cardinality budget 8192
series):

- `Metrics::kit_gauge` registers a single-series gauge on the kit
  registry. Registration failures (budget refused, duplicate series)
  return `None` and the caller proceeds without the mirror: the
  Prometheus endpoint is unaffected.
- `Services::gauge` mirrors each observable gauge into the kit
  registry through a second OTel callback. The callback re-runs per
  collection; the estimates it reads are RocksDB property reads
  (cheap, documented upstream).
- `Metrics::render_openmetrics` renders the kit registry as
  OpenMetrics 1.0.0 (`application/openmetrics-text; version=1.0.0`).
- New route `GET /_continuwuity/metrics/openmetrics`, gated by the
  same `allow_prometheus_metrics` config and bearer token as the
  Prometheus endpoint.

Known scope limits, recorded rather than hidden: the kit registry
carries single-series gauges only (labelled series such as
`continuwuity.users{origin}` remain on the Prometheus endpoint), and
the gauge callback runs twice per collection.

## Phases

| Phase | Scope | State |
|---|---|---|
| 0 | Stock deployment on the estate NAS | deployment scaffold in the dotfiles repo (`truenas-matrix/`) |
| 1 | Estate crates as additive integrations (metrics-kit done; telemetry-init, config-kit next) | in progress |
| 2 | Behavior swaps behind config flags (outbox-kit for the federation send queue, loop-retry in the sender) | not started |
| 3 | Estate-specific features (webhookkit admin notifications) | not started |
