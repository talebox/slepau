#![feature(linked_list_remove)]
#![feature(linked_list_retain)]
use auth::validate::KPR;
use axum::{
	error_handling::HandleErrorLayer,
	routing::{get, post},
	Extension, Router,
};

use common::{
	init::{init},
	socket::ResourceMessage,
	utils::{log_env, SOCKET, URL},
};
use env_logger::Env;
use hyper::StatusCode;
use log::{error, info};

mod db;
mod ends;
mod radio;
mod socket;
mod views;

use std::{
	net::SocketAddr,
	sync::{Arc, RwLock},
	time::Duration,
};

#[cfg(not(target_family = "windows"))]
use tokio::signal::unix::{signal, SignalKind};

use tokio::{
	join,
	sync::{broadcast, mpsc, watch},
};
use tower_http::timeout::TimeoutLayer;

#[tokio::main]
async fn main() {
	// Enable env_logger implemenation of log.
	let env = Env::default()
		.filter_or("LOG_LEVEL", "info")
		.write_style_or("LOG_STYLE", "auto");

	env_logger::init_from_env(env);
	log_env();
	common::sonnerie::init();

	print!(
		"\
	Hi, I'm Samn (Smart Arduino Mesh Network) !\n\
	I'm the spine for your Node limbs.\n\
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
	let db = init::<db::DB>().await;
	// info!("{db:?}");
	let db = Arc::new(RwLock::new(db));

	let (shutdown_tx, shutdown_rx) = watch::channel(());
	let (resource_tx, _resource_rx) = broadcast::channel::<ResourceMessage>(16);
	let (radio_tx, radio_rx) = mpsc::channel(30);

	// Build router
	let app = Router::new()
		.route("/:key", get(ends::log_get))
		.route("/command", post(ends::command))
		.route("/command/wait", post(ends::command_response))
		.route("/stream", get(socket::websocket_handler))
		// .layer(axum::middleware::from_fn(auth::validate::flow::only_supers))
		.layer(axum::middleware::from_fn(auth::validate::authenticate))
		.layer(TimeoutLayer::new(Duration::from_secs(30)))
		.layer(
			tower::ServiceBuilder::new()
				.layer(HandleErrorLayer::new(|_| async { StatusCode::SERVICE_UNAVAILABLE }))
				.concurrency_limit(100)
				.layer(Extension(shutdown_rx.clone()))
				.layer(Extension(resource_tx.clone()))
				.layer(Extension(radio_tx.clone()))
				.layer(Extension(db.clone())),
		);

	info!("Listening on '{}'.", SOCKET.to_string());
	info!("Public url is on '{}'.", URL.as_str());

	// Create server
	let mut _shutdown_rx = shutdown_rx.clone();
	let server = axum::Server::bind(&SOCKET)
		.serve(app.into_make_service_with_connect_info::<SocketAddr>())
		.with_graceful_shutdown(async move {
			if let Err(err) = _shutdown_rx.changed().await {
				error!("Error receiving shutdown {err:?}");
			} else {
				info!("Http server shutting down gracefully");
			}
		});

	let server = tokio::spawn(server);
	let radio = tokio::spawn(radio::radio_service(
		db.clone(),
		shutdown_rx.clone(),
		radio_rx,
		resource_tx.clone(),
	));

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
	let _server_r = join!(server, radio);

	info!("Everyone's shut down!");

	if db.is_poisoned() {
		error!(
			"DB was poisoned, can't clear it because we're in (stable) channel; so saving won't work.\n\
			This probaly happened because of an error.\n\
			Logging service will soon be implemented to notify of these."
		);
		// db.clear_poison();
	}

	match Arc::try_unwrap(db) {
		Ok(db) => {
			let db = db.into_inner().unwrap();
			common::init::save(&db);
		}
		Err(db) => {
			error!("Couldn't unwrap DB, will save anyways, but beware of this");
			common::init::save(&*db.read().unwrap());
		}
	}
}
