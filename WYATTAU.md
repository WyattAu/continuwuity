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

Labelled series (cycle 8 fork v2): gauges with FIXED labelsets
(`continuwuity.users{origin}` local/remote) pre-register their series
and mirror inside the closure; DYNAMIC-label gauges (per-server
destinations) stay on the Prometheus endpoint, since unbounded label
registration is what the cardinality budget exists to refuse. The
gauge callback runs twice per collection for mirrored gauges.

## Phases

| Phase | Scope | State |
|---|---|---|
| 0 | Stock deployment on the estate NAS | deployment scaffold in the dotfiles repo (`truenas-matrix/`) |
| 1 | Estate crates as additive integrations (metrics-kit done; telemetry-init + config-kit assessed: estate crates stay out, upstream observability/config already covers them; verdicts in this file's history) | complete |
| 2 | Behavior swaps behind config flags (outbox-kit for the federation send queue, loop-retry in the sender): assessed, upstream sender already carries durable queues, breaker-aware backoff, and netburst recovery; researched no | complete (documented no) |
| 3 | Estate-specific features: webhookkit-signed admin notifications on local registrations | complete |

## Phase 3 notes: the admin webhook

`admin_webhook_url` + `admin_webhook_secret` (core config) enable signed
notifications on local registrations. Signing uses
[webhookkit](https://github.com/WyattAu/webhookkit) 2.2.0's canonical
`sign_stripe_timestamped` (Stripe `t=`/`v1=` header convention), so
receivers verify with that crate's own `verify_stripe_webhook` and no
new receiver code. Delivery is spawned: registration never depends on
the notification path, failures log and drop.

Consolidation note: webhookkit briefly carried two signing APIs (this
fork's phase-1 work landed `signing.rs` while the parallel estate work
landed `sign_payload` + `sign_stripe_timestamped` in `stripe.rs`); the
duplicate was removed upstream and the canonical API is
`sign_payload` + `sign_stripe_timestamped`.

## Verdict receipts (phases 1 and 2)

- **telemetry-init, config-kit: estate crates stay out.** continuwuity's
  observability (`src/core/log/` console/capture/reload layers,
  `src/main/metrics.rs` OTel provider, `src/core/metrics/` exposition)
  and configuration (layered TOML, `CONDUWUIT_*` env overrides, hot
  reload) already provide what those estate crates bootstrap for
  greener services. Adopting them here replaces a comprehensive,
  upstream-shared subsystem with a thinner wrapper and breaks upstream
  diff affinity for no capability gain.
- **outbox-kit, loop-retry: researched no.** The federation send queue
  (`src/service/sending/`) is durable (DB-backed per-destination
  queues), breaker-aware, and carries netburst recovery plus retry
  limits; outbox-kit's Postgres-outbox model duplicates it, and
  loop-retry wraps backoff the sender implements inline. Revisit only
  if the sender grows a requirement its queue cannot express
  (multi-channel fanout, cross-instance claiming).
