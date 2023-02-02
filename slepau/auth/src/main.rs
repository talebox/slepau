use auth::{
	validate::{public_key, KP},
	UserClaims,
};
use axum::{
	error_handling::{HandleError, HandleErrorLayer},
	response::IntoResponse,
	routing::{any, get, get_service, post, post_service},
	Extension, Router,
};
// use futures::future::join;
use common::{
	init::{backup::backup_service, magic_bean::mirror_bean},
	utils::{log_env, HOST, HOSTNAME, WEB_DIST},
	Cache,
};
use env_logger::Env;
use hyper::{header, StatusCode};
use lazy_static::lazy_static;
use log::{error, info};
use tower::{
	buffer::{Buffer, BufferLayer},
	limit::{ConcurrencyLimitLayer, GlobalConcurrencyLimitLayer, RateLimit, RateLimitLayer},
	service_fn, Layer, ServiceBuilder,
};

use std::{
	fs::read_to_string,
	future::ready,
	net::SocketAddr,
	path::PathBuf,
	str::FromStr,
	sync::{Arc, RwLock},
	time::Duration,
};
use tokio::{
	join,
	signal::unix::{signal, SignalKind},
	sync::watch,
};
use tower_http::{
	services::{ServeDir, ServeFile},
	timeout::TimeoutLayer,
	trace::TraceLayer,
};

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
	Hi I'm Auth 🔐!\n\
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

	let index_service = |dir: &str, index: Option<&str>| {
		let assets_dir = PathBuf::from(dir);
		let index_file = assets_dir.join(index.unwrap_or("index.html"));
		get_service(ServeFile::new(index_file)).handle_error(|_| ready(StatusCode::INTERNAL_SERVER_ERROR))
	};
	let assets_service = |dir: &str| {
		let assets_dir = PathBuf::from(dir);
		get_service(ServeDir::new(&assets_dir).precompressed_br().precompressed_gzip())
			.handle_error(|_| ready(StatusCode::INTERNAL_SERVER_ERROR))
	};
	async fn home_service(Extension(claims): Extension<UserClaims>) -> Result<impl IntoResponse, impl IntoResponse> {
		// lazy_static! {
		// 	static ref HOME: std::io::Result<String> = ;
		// };
		let HOME = read_to_string(PathBuf::from(WEB_DIST.as_str()).join("home.html"));
		HOME
			.as_ref()
			.and_then(|home| {
				Ok((
					[(header::CONTENT_TYPE, "text/html")],
					home.replace("_HOST_", &HOSTNAME).replace("_USER_", &claims.user),
				))
			})
			.or(Err(StatusCode::INTERNAL_SERVER_ERROR))
	}

	// let buffer = BufferLayer::new(1100);
	// RateLimitLayer::new(5, Duration::from_secs(1));
	// let rate = RateLimit::new(Rate::);
	let security_limit = |n, secs| {
		tower::ServiceBuilder::new()
			.layer(HandleErrorLayer::new(|_| async { StatusCode::TOO_MANY_REQUESTS }))
			.buffer((n * secs + 5) as usize)
			.load_shed()
			.rate_limit(n, Duration::from_secs(secs))
	};

	// Build router
	let app = Router::new()
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

	// Create Socket to listen on
	let addr = SocketAddr::from_str(&HOST).unwrap();

	if cfg!(debug_assertions) {
		info!("Listening on 'http://{}'.", addr)
	} else {
		info!("Listening on 'https://{}'.", addr)
	};

	// Create server
	let server = axum::Server::bind(&addr)
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
			common::init::save(&db).await;
		}
		Err(db) => {
			error!("Couldn't unwrap DB, will save anyways, but beware of this");
			common::init::save(&*db.read().unwrap()).await;
		}
	}

	cache.read().unwrap().save();
}
