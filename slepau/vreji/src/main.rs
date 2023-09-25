use auth::validate::KPR;
use axum::{
	error_handling::HandleErrorLayer,
	routing::{get},
	Extension, Router,
};

use common::{
	utils::{log_env, SOCKET, URL},
};
use env_logger::Env;
use hyper::StatusCode;
use log::{error, info};
use tower::ServiceBuilder;
use tower_governor::{errors::display_error, GovernorLayer};

use std::{
	net::SocketAddr,
	sync::{Arc, RwLock},
	time::Duration,
};

#[cfg(not(target_family = "windows"))]
use tokio::signal::unix::{signal, SignalKind};

use tokio::{join, sync::watch};
use tower_http::timeout::TimeoutLayer;

mod ends;

#[tokio::main]
async fn main() {
	// Enable env_logger implemenation of log.
	let env = Env::default()
		.filter_or("LOG_LEVEL", "info")
		.write_style_or("LOG_STYLE", "auto");

	env_logger::init_from_env(env);
	log_env();

	print!(
		"\
	Hi, I'm Vreji (record in Lojban) !\n\
	I log every activity.\n\
	\n\
	I'm a rusty HTTP slepau\n\
	that aims to be self contained.\n\
	\n\
	"
	);

	{
		// Check that keys exist
		// lazy_static::initialize(&KP);
		lazy_static::initialize(&KPR);
	}

	// DB Init

	let (shutdown_tx, mut shutdown_rx) = watch::channel(());

	// Build router
	let app = Router::new()
		// .route("/:key", method_router)
		.route("/:key", get(ends::log_get))
		.route("/anything", get(ends::by_anything))
		.route("/ips", get(ends::by_ip))
		.route("/by_user", get(ends::by_user))
		.route("/stats", get(ends::stats))
		.layer(axum::middleware::from_fn(auth::validate::flow::only_supers))
		.layer(axum::middleware::from_fn(auth::validate::authenticate))
		.layer(TimeoutLayer::new(Duration::from_secs(30)))
		.layer(
			tower::ServiceBuilder::new()
				.layer(HandleErrorLayer::new(|_| async { StatusCode::SERVICE_UNAVAILABLE }))
				.concurrency_limit(100)
				.layer(Extension(shutdown_rx.clone())), // .layer(Extension(resource_tx.clone())),
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

	#[cfg(target_family = "windows")]
	async fn wait_terminate() {
		tokio::signal::ctrl_c().await.ok();
	}
	#[cfg(not(target_family = "windows"))]
	async fn wait_terminate() {
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
	}

	wait_terminate().await;

	info!("Telling everyone to shutdown.");
	shutdown_tx.send(()).unwrap();

	info!("Waiting for everyone to shutdown.");
	let _server_r = join!(server);

	info!("Everyone's shut down!");
}
