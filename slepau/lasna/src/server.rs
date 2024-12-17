use std::{net::SocketAddr, time::Duration};

use auth::validate::KPR;
use axum::{
	error_handling::HandleErrorLayer, response::IntoResponse, BoxError,
	Extension, Json, Router,
};
use common::utils::{SOCKET, URL};
use hyper::StatusCode;
use log::{error, info};
use tower::ServiceBuilder;
use tower_governor::{
	errors::display_error, governor::GovernorConfigBuilder,
	key_extractor::SmartIpKeyExtractor, GovernorLayer,
};
use tower_http::timeout::TimeoutLayer;

use crate::DEVICE_CONNECTIONS;

async fn get_devices() -> impl IntoResponse {
	Json(
		DEVICE_CONNECTIONS
			.read()
			.await
			.keys()
			.copied()
			.collect::<Vec<_>>(),
	)
}
// async fn get_device(Path(device_id): Path<DeviceId>) -> impl IntoResponse {
// 	Json(DEVICE_CONNECTIONS.read().await.get(&device_id).is_some())
// }

pub async fn spawn_server(mut shutdown_rx: tokio::sync::watch::Receiver<()>) {
	{
		// Check that keys exist
		lazy_static::initialize(&KPR);
	}

	let governor_conf = Box::new(
		GovernorConfigBuilder::default()
			.key_extractor(SmartIpKeyExtractor)
			.per_second(2)
			.burst_size(5)
			.finish()
			.unwrap(),
	);


	// Build router
	let app = Router::new()
		.route("/", axum::routing::get(get_devices))
		.layer(axum::middleware::from_fn(auth::validate::flow::only_supers))
		// .route("/:device_id", axum::routing::get(get_device))
		.layer(axum::middleware::from_fn(auth::validate::authenticate))
		// The request limiter :)
		.layer(
			ServiceBuilder::new()
				.layer(HandleErrorLayer::new(|e: BoxError| async move {
					display_error(e)
				}))
				.layer(GovernorLayer {
					config: Box::leak(governor_conf),
				}),
		)
		// .fallback_service(static_routes())
		.layer(TimeoutLayer::new(Duration::from_secs(30)))
		.layer(
			tower::ServiceBuilder::new()
				.layer(HandleErrorLayer::new(|_| async {
					StatusCode::SERVICE_UNAVAILABLE
				}))
				.concurrency_limit(100)
				.layer(Extension(shutdown_rx.clone())),
		);

	info!("Listening on '{}'.", SOCKET.to_string());
	info!("Public url is on '{}'.", URL.as_str());

	// Create server
	let server = axum::Server::bind(&SOCKET)
		.serve(app.into_make_service_with_connect_info::<SocketAddr>())
		.with_graceful_shutdown(async move {
			if let Err(err) = shutdown_rx.changed().await {
				error!("Error receiving shutdown {err:?}");
			} else {
				info!("Http server shutting down gracefully");
			}
		});

	server.await.ok();
	
}
