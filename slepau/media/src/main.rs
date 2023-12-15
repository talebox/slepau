#![feature(result_flattening)]

use axum::{
	error_handling::HandleErrorLayer,
	middleware::from_fn,
	routing::{get, post},
	Extension, Router,
};

use common::{
	http::static_routes,
	socket::ResourceMessage,
	utils::{log_env, SOCKET, URL},
};
use env_logger::Env;
use hyper::StatusCode;
use log::{error, info};

use std::{
	net::SocketAddr,
	sync::{Arc, RwLock},
	time::Duration,
};
use tokio::{
	join,
	signal::unix::{signal, SignalKind},
	sync::{broadcast, mpsc, watch},
};
use tower_http::timeout::TimeoutLayer;

pub mod db;
pub mod ends;
mod socket;

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
	Hey, I'm Media!\n\
	I hold all your files in a nice format.\n\
	\n\
	I'm a rusty HTTP slepau\n\
	that aims to be self contained.\n\
	\n\
	Cookie `auth` tells me who you are 😉\n\
	\n\
	"
	);

	let (shutdown_tx, mut shutdown_rx) = watch::channel(());
	// What media has changed, mainly used to inform the UI
	// let (media_tx, media_rx) = broadcast::channel(5);
	// Immediate tasks being requested from the task service
	let (task_tx, task_rx) = mpsc::channel(5);
	let (resource_tx, _resource_rx) = broadcast::channel::<ResourceMessage>(16);

	let db = common::init::init::<db::DB>().await;
	// info!("{db:?}");
	let db = Arc::new(RwLock::new(db));
	let load_existing_handle;
	{
		let db = db.clone();
		load_existing_handle = tokio::task::spawn_blocking(move || db::def::load_existing(db));
	}

	// Build router
	let app = Router::new()
		.route(
			"/:id",
			get(ends::media_get).patch(ends::media_patch).delete(ends::media_delete),
		)
		.route("/", post(ends::media_post))
		.route(
			"/media/:id",
			get(ends::media_get).patch(ends::media_patch).delete(ends::media_delete),
		)
		.route(
			"/stream",
			get(socket::websocket_handler).layer(from_fn(auth::validate::flow::auth_required)),
		)
		.route("/stats", get(ends::stats))
		.route("/media", post(ends::media_post))
		.layer(axum::middleware::from_fn(auth::validate::authenticate))
		.layer(TimeoutLayer::new(Duration::from_secs(30)))
		.layer(
			tower::ServiceBuilder::new()
				.layer(HandleErrorLayer::new(|_| async { StatusCode::SERVICE_UNAVAILABLE }))
				.concurrency_limit(100)
				.layer(Extension(shutdown_rx.clone()))
				.layer(Extension(resource_tx.clone()))
				.layer(Extension(task_tx.clone()))
				.layer(Extension(db.clone())),
		);

	let conversion_service = tokio::spawn(db::task::conversion_service(
		db.clone(),
		shutdown_rx.clone(),
		resource_tx,
		task_rx,
	));

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
	info!("Listening on '{}'.", SOCKET.to_string());
	info!("Public url is on '{}'.", URL.as_str());

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
	let _server_r = join!(server, conversion_service, load_existing_handle);
	info!("Everyone's shut down!");

	common::init::save(&*db.read().unwrap());

	info!("Goodbye!");
}
