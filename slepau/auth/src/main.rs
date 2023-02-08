use auth::{validate::KP, UserClaims};
use axum::{
	body::Body,
	error_handling::HandleErrorLayer,
	response::IntoResponse,
	routing::{get, post},
	Extension, Router,
};

use common::{
	http::{assets_service, index_service},
	init::backup::backup_service,
	utils::{log_env, SOCKET, URL, WEB_DIST},
	Cache,
};
use env_logger::Env;
use hyper::{header, StatusCode};
use log::{error, info};
use tower::ServiceBuilder;

use std::{
	fs::read_to_string,
	net::SocketAddr,
	path::PathBuf,
	sync::{Arc, RwLock},
	time::Duration,
};
use tokio::{
	join,
	signal::unix::{signal, SignalKind},
	sync::watch,
};
use tower_http::timeout::TimeoutLayer;

mod db;
mod ends;
mod user;

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
	Hi, I'm Auth 🔐!\n\
	Other slepau depend on me to tell them who you are.\n\
	I'll give you cookie `auth` if your credentials are good.\n\
	\n\
	I'm a rusty HTTP slepau\n\
	that aims to be self contained.\n\
	\n\
	"
	);

	{
		// Check that keys exist
		lazy_static::initialize(&KP);
	}

	// Read cache
	let cache = Arc::new(RwLock::new(Cache::init()));
	let db = Arc::new(RwLock::new(common::init::init::<db::DBAuth>().await));

	let (shutdown_tx, mut shutdown_rx) = watch::channel(());

	async fn home_service(
		Extension(claims): Extension<UserClaims>,
		req: axum::http::Request<Body>,
	) -> Result<impl IntoResponse, impl IntoResponse> {
		let host: String = req
			.headers()
			.get("Host")
			.and_then(|v| Some(v.to_str().unwrap().split('.').last().unwrap().into()))
			.unwrap_or(URL.host().unwrap().to_string());
		let home = read_to_string(PathBuf::from(WEB_DIST.as_str()).join("home.html"));
		home
			.as_ref()
			.map(|home| {
				(
					[(header::CONTENT_TYPE, "text/html")],
					home.replace("_HOST_", &host).replace("_USER_", &claims.user),
				)
			})
			.or(Err(StatusCode::INTERNAL_SERVER_ERROR))
	}

	let security_limit = |n, secs| {
		ServiceBuilder::new()
			.layer(HandleErrorLayer::new(|_| async { StatusCode::TOO_MANY_REQUESTS }))
			.buffer((n * secs + 5) as usize)
			.load_shed()
			.rate_limit(n, Duration::from_secs(secs))
	};

	// Build router
	let app = Router::new()
		// Admin Actions V
		.merge(
			Router::new()
				// // Get/Modify Site
				// .route("/sites", )
				// // Get/Modify User
				// .route("/sites/:id/users", )
				// // Get/Modify Admin (only super)
				// .route("/admins", )
		)
		// User Actions V
		.merge(
			Router::new()
				.route("/reset", post(crate::ends::reset))
				.route("/register", post(crate::ends::register))
				.layer(security_limit(1, 10)),
		)
		.merge(
			Router::new()
				.route("/user", get(crate::ends::user))
				.route("/logout", get(crate::ends::logout))
				.layer(security_limit(1, 1)),
		)
		.fallback(home_service)
		.layer(axum::middleware::from_fn(auth::validate::authenticate))
		.nest(
			"/login",
			Router::new()
				.route(
					"/",
					post(crate::ends::login)
						.layer(security_limit(1, 5))
						.fallback_service(index_service(WEB_DIST.as_str(), Some("login.html"))),
				)
				.fallback_service(index_service(WEB_DIST.as_str(), Some("login.html"))),
		)
		.nest_service(
			"/web",
			assets_service(WEB_DIST.as_str()).fallback_service(index_service(WEB_DIST.as_str(), None)),
		)
		.layer(TimeoutLayer::new(Duration::from_secs(30)))
		.layer(
			tower::ServiceBuilder::new()
				.layer(HandleErrorLayer::new(|_| async { StatusCode::SERVICE_UNAVAILABLE }))
				.concurrency_limit(100)
				.layer(Extension(db.clone()))
				.layer(Extension(cache.clone()))
				.layer(Extension(shutdown_rx.clone())), // .layer(Extension(resource_tx.clone())),
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
