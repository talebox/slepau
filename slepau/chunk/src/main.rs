use auth::validate::KPR;
use axum::{error_handling::HandleErrorLayer, middleware::from_fn, routing::{get, put}, BoxError, Extension, Router};

use common::{
	http::{static_routes},
	init::backup::backup_service,
	socket::ResourceMessage,
	utils::{log_env, SOCKET, URL},
	Cache,
};
use env_logger::Env;
use hyper::StatusCode;
use log::{error, info};
use tower::ServiceBuilder;
use tower_governor::{errors::display_error, governor::GovernorConfigBuilder, GovernorLayer, key_extractor::SmartIpKeyExtractor};

use std::{
	net::SocketAddr,
	sync::{Arc, RwLock},
	time::Duration,
};
use tokio::{
	join,
	signal::unix::{signal, SignalKind},
	sync::{broadcast, watch},
};
use tower_http::timeout::TimeoutLayer;

use chunk::{
	db,
	ends::{self},
	socket::{self},
};

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

	{
		// Check that keys exist
		lazy_static::initialize(&KPR);
	}

	// Read cache
	let cache = Arc::new(RwLock::new(Cache::init()));
	let db = Arc::new(RwLock::new(common::init::init::<db::DB>().await));

	let (shutdown_tx, mut shutdown_rx) = watch::channel(());
	let (resource_tx, _resource_rx) = broadcast::channel::<ResourceMessage>(16);

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
		.route(
			"/chunks",
			put(ends::chunks_put).delete(ends::chunks_del),
		)
		// ONLY if NOT public ^
		.route_layer(from_fn(auth::validate::flow::auth_required))
		.route("/chunks/:id", get(ends::chunks_get_id))
		.route("/stream", get(socket::websocket_handler))
		// ONLY GET if public ^
		.route_layer(from_fn(auth::validate::flow::public_only_get))
		.route("/page/:id", get(ends::page_get_id))
		// .nest_service("/preview", index_service(WEB_DIST.as_str(), Some("preview.html")))
		.layer(axum::middleware::from_fn(auth::validate::authenticate))
		// The request limiter :)
		.layer(
			ServiceBuilder::new()
				.layer(HandleErrorLayer::new(|e: BoxError| async move { display_error(e) }))
				.layer(GovernorLayer {
					config: Box::leak(governor_conf),
				}),
		)
		.fallback_service(static_routes())
		.layer(TimeoutLayer::new(Duration::from_secs(30)))
		.layer(
			tower::ServiceBuilder::new()
				.layer(HandleErrorLayer::new(|_| async { StatusCode::SERVICE_UNAVAILABLE }))
				.concurrency_limit(100)
				.layer(Extension(db.clone()))
				.layer(Extension(cache.clone()))
				.layer(Extension(shutdown_rx.clone()))
				.layer(Extension(resource_tx.clone())),
		);

	// Backup service
	let backup = tokio::spawn(backup_service(cache.clone(), db.clone(), shutdown_rx.clone()));

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
	let (_server_r, _backup_r) = join!(server, backup);

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

	cache.read().unwrap().save();
}
