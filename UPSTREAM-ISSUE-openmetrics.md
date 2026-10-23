# Ready-to-file: upstream OpenMetrics issue

File at forgejo.ellis.link/continuwuation/continuwuity (issues are
disabled on the GitHub mirror). Title and body below, verbatim.

## Title

Add an OpenMetrics-format variant of the metrics endpoint

## Body

`GET /_continuwuity/metrics` today serves Prometheus text exposition
via `opentelemetry-prometheus`, gated by `allow_prometheus_metrics`
and the metrics token. OpenMetrics (the IETF-adjacent successor
format: `# TYPE`/`# UNIT` headers, explicit `_total` counter suffixes,
a `# EOF` terminator, and the `application/openmetrics-text` media
type) is what an increasing share of scrapers prefer: VictoriaMetrics,
Grafana Agent, and current `prometheus` tooling all ingest it, and
several compliance checkers require the format's unit and type
declarations.

Proposal: a second route `/_continuwuity/metrics/openmetrics` (or
content negotiation on the existing route via the `Accept` header),
reusing the same `allow_prometheus_metrics` gate and token. The
change is additive: the existing Prometheus endpoint stays the
default and nothing breaks.

A working implementation exists in a downstream fork
(`WyattAu/continuwuity`, branch `wyatt/dogfood-phase1`): the route is
`src/api/client/unversioned.rs::
continuwuity_metrics_openmetrics`, backed by a `render_openmetrics()`
renderer in `src/core/metrics/mod.rs`, built for an internal
deployment. Happy to rebase it onto the opentelemetry exporter output
and split it as a focused PR if the maintainers want the format.
