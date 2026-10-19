use axum::{Json, extract::State, response::IntoResponse};
use conduwuit::{
	Err, Result,
	matrix::versions::{unstable_features, versions},
	metrics::{Format, PROMETHEUS_CONTENT_TYPE},
	utils::string::constant_time_eq,
};
use futures::StreamExt;
use http::{HeaderMap, header};
use ruma::{api::client::discovery::get_supported_versions, assign};

use crate::Ruma;

/// # `GET /_matrix/client/versions`
///
/// Get the versions of the specification and unstable features supported by
/// this server.
///
/// - Versions take the form MAJOR.MINOR.PATCH
/// - Only the latest PATCH release will be reported for each MAJOR.MINOR value
/// - Unstable features are namespaced and may include version information in
///   their name
///
/// Note: Unstable features are used while developing new features. Clients
/// should avoid using unstable features in their stable releases
pub(crate) async fn get_supported_versions_route(
	State(services): State<crate::State>,
	_body: Ruma<get_supported_versions::Request>,
) -> Result<get_supported_versions::Response> {
	let mut unstable_features = unstable_features();

	if services.config.allow_sticky_events {
		// sticky events (https://github.com/matrix-org/matrix-spec-proposals/pull/4354)
		unstable_features.insert("org.matrix.msc4354".to_owned(), true);
	}

	Ok(assign!(get_supported_versions::Response::new(versions()), {
		unstable_features
	}))
}

/// # `GET /_continuwuity/server_version`
///
/// Conduwuit-specific API to get the server version, results akin to
/// `/_matrix/federation/v1/version`
pub(crate) async fn continuwuity_server_version() -> Result<impl IntoResponse> {
	Ok(Json(serde_json::json!({
		"name": conduwuit::BRANDING,
		"version": conduwuit::version(),
	})))
}

/// # `GET /_continuwuity/local_user_count`
///
/// conduwuit-specific API to return the amount of users registered on this
/// homeserver. Endpoint is disabled if federation is disabled for privacy. This
///
/// only includes active users (not deactivated, etc)
pub(crate) async fn continuwuity_local_user_count(
	State(services): State<crate::State>,
) -> Result<impl IntoResponse> {
	let user_count = services
		.users
		.stream_local_users()
		.filter_map(async |user_id| {
			if !services.users.status(&user_id).await.is_active()
				|| services.appservice.is_exclusive_user_id(&user_id)
			{
				None
			} else {
				Some(user_id)
			}
		})
		.count()
		.await;

	Ok(Json(serde_json::json!({
		"count": user_count
	})))
}

/// # `GET /_continuwuity/metrics`
///
/// Prometheus metrics in the text exposition format. Only routed when
/// `allow_prometheus_metrics` is enabled.
pub(crate) async fn continuwuity_metrics(
	State(services): State<crate::State>,
	headers: HeaderMap,
) -> Result<impl IntoResponse> {
	if let Some(token) = &services.config.prometheus_metrics_token {
		let authorized = headers
			.get(header::AUTHORIZATION)
			.and_then(|value| value.to_str().ok())
			.and_then(|value| value.split_once(' '))
			.is_some_and(|(scheme, provided)| {
				scheme.eq_ignore_ascii_case("bearer") && constant_time_eq(provided, token)
			});

		if !authorized {
			return Err!(Request(Unauthorized("Invalid metrics token")));
		}
	}

	let body = services.server.metrics.render()?;

	Ok(([(header::CONTENT_TYPE, PROMETHEUS_CONTENT_TYPE)], body))
}

/// # `GET /_continuwuity/metrics/openmetrics`
///
/// OpenMetrics text exposition from the metrics-kit registry. Carries the
/// single-series gauges; labelled series remain on the Prometheus endpoint.
/// Same enablement (`allow_prometheus_metrics`) and bearer-token gate as
/// the Prometheus endpoint.
pub(crate) async fn continuwuity_metrics_openmetrics(
	State(services): State<crate::State>,
	headers: HeaderMap,
) -> Result<impl IntoResponse> {
	if let Some(token) = &services.config.prometheus_metrics_token {
		let authorized = headers
			.get(header::AUTHORIZATION)
			.and_then(|value| value.to_str().ok())
			.and_then(|value| value.split_once(' '))
			.is_some_and(|(scheme, provided)| {
				scheme.eq_ignore_ascii_case("bearer") && constant_time_eq(provided, token)
			});

		if !authorized {
			return Err!(Request(Unauthorized("Invalid metrics token")));
		}
	}

	let content_type = Format::OpenMetrics.content_type();
	let body = services.server.metrics.render_openmetrics()?;

	Ok(([(header::CONTENT_TYPE, content_type)], body))
}

/// Fire-and-forget signed admin webhook (`admin_webhook_url` +
/// `admin_webhook_secret`, cycle 8 iteration 4). The payload is signed in
/// the Stripe `t=`/`v1=` header convention via
/// [webhookkit](https://github.com/WyattAu/webhookkit), so receivers
/// verify with `webhookkit::verify_stripe_webhook` and no new receiver
/// code. Delivery runs on a spawned task: failures log and are dropped,
/// because registration must not depend on the notification path.
pub(crate) fn spawn_admin_webhook(services: &crate::State, event: &'static str, user_id: &str) {
	let Some(url) = services.config.admin_webhook_url.clone() else {
		return;
	};
	let Some(secret) = services.config.admin_webhook_secret.clone() else {
		return;
	};

	let payload = serde_json::json!({
		"event": event,
		"user_id": user_id,
		"server": services.config.server_name.to_string(),
	});
	let client = services.client.dns.clone();

	tokio::spawn(async move {
		let body = payload.to_string();
		let timestamp = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map_or(0, |d| d.as_secs())
			.to_string();
		let signature = webhookkit::sign_stripe_timestamped(&timestamp, &body, &secret);

		let result = client
			.post(&url)
			.header("Content-Type", "application/json")
			.header("Signature", signature)
			.header("Webhook-Timestamp", timestamp)
			.body(body)
			.send()
			.await;

		match result {
			| Ok(resp) if !resp.status().is_success() => {
				tracing::warn!(%event, status = %resp.status(), "admin webhook delivery refused");
			},
			| Err(e) => {
				tracing::warn!(%event, error = %e, "admin webhook delivery failed");
			},
			| Ok(_) => {},
		}
	});
}
