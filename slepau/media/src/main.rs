use axum::{
	error_handling::HandleErrorLayer,
	routing::{get, post},
	Extension, Router,
};

use common::{utils::{log_env, SOCKET, URL, WEB_DIST}, http::assets_service};
use env_logger::Env;
use hyper::{StatusCode};
use log::{error, info};

use std::{net::SocketAddr, time::Duration};
use tokio::{
	join,
	signal::unix::{signal, SignalKind},
	sync::watch,
};
use tower_http::timeout::TimeoutLayer;

pub mod ends;

#[tokio::main]
pub async fn main() {
	// Enable env_logger implemenation of log.
	let env = Env::default()
		.filter_or("LOG_LEVEL", "info")
		.write_style_or("LOG_STYLE", "auto");

	env_logger::init_from_env(env);
	log_env();

	print!(
		"\
	Hi, I'm Chunk ⚙🔭!\n\
	I'll help you organize yourself.\n\
	\n\
	I'm a rusty HTTP slepau\n\
	that aims to be self contained.\n\
	\n\
	Cookie `auth` tells me who you are 😉\n\
	\n\
	"
	);

	let (shutdown_tx, mut shutdown_rx) = watch::channel(());

	// Build router
	let app = Router::new()
		.nest(
			"/api",
			Router::new()
				.route("/media/:id", get(ends::media_get))
				.route("/media", post(ends::media_post)),
		)
		.fallback(ends::home_service)
		// .nest_service("/app", get(index_service_user))
		.layer(axum::middleware::from_fn(auth::validate::authenticate))
		.nest_service("/web", assets_service(WEB_DIST.as_str()))
		.layer(TimeoutLayer::new(Duration::from_secs(30)))
		.layer(
			tower::ServiceBuilder::new()
				.layer(HandleErrorLayer::new(|_| async { StatusCode::SERVICE_UNAVAILABLE }))
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

	let server = tokio::spawn(server);

	// Listen to iterrupt or terminate signal to order a shutdown if either is triggered
	let mut s0 = signal(SignalKind::interrupt()).unwrap();
	let mut s1 = signal(SignalKind::terminate()).unwrap();
	tokio::select! {
		_ = s0.recv() => {
			info!("Received Interrupt, exiting.");
		}
		_ = s1.recv() => {
			info!("Received Terminate, exiting.");
		}
	}

	info!("Telling everyone to shutdown.");
	shutdown_tx.send(()).unwrap();

	info!("Waiting for everyone to shutdown.");
	let _server_r = join!(server);
	// .nest_service("/app", get(index_service_user))
	// .layer(axum::middleware::from_fn(auth::validate::authenticate))
	info!("Everyone's shut down, goodbye!");
}
