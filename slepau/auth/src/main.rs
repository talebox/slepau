use auth::validate::KPR;
use axum::{
	error_handling::HandleErrorLayer,
	routing::{get, patch, post, put},
	BoxError, Extension, Router,
};

use common::{
	http::static_routes,
	init::backup::backup_service,
	utils::{log_env, wait_terminate, SOCKET, URL},
	Cache,
};
use env_logger::Env;
use hyper::StatusCode;
use log::{error, info};
use tower::ServiceBuilder;
use tower_governor::{
	errors::display_error, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};

use std::{
	net::SocketAddr,
	sync::{Arc, RwLock},
	time::Duration,
};


use tokio::{join, sync::watch};
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
	common::sonnerie::init();

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
		// lazy_static::initialize(&KP);
		lazy_static::initialize(&KPR);
	}

	// Read cache
	let cache = Arc::new(RwLock::new(Cache::init()));
	let db = Arc::new(RwLock::new(common::init::init::<db::DBAuth>().await));

	let (shutdown_tx, mut shutdown_rx) = watch::channel(());

	// let security_limit = |n, secs| {
	// 	ServiceBuilder::new()
	// 		.layer(HandleErrorLayer::new(|_| async { StatusCode::TOO_MANY_REQUESTS }))
	// 		.buffer((n * secs + 5) as usize)
	// 		.load_shed()
	// 		.rate_limit(n, Duration::from_secs(secs))
	// };

	let governor_conf = Box::new(
		GovernorConfigBuilder::default()
			.key_extractor(SmartIpKeyExtractor)
			.per_second(2)
			.burst_size(5)
			.finish()
			.unwrap(),
	);
	let governor_conf_relaxed = Box::new(
		GovernorConfigBuilder::default()
			.key_extractor(SmartIpKeyExtractor)
			.per_second(5)
			.burst_size(30)
			.finish()
			.unwrap(),
	);

	// Build router
	let app = Router::new()
		// Admin Actions V
		.merge(
			Router::new()
				// Get/Modify Admin
				.route("/admins", get(ends::admin::get_admins).post(ends::admin::post_admin))
				.route(
					"/admins/:id",
					put(ends::admin::put_admin).delete(ends::admin::del_admin),
				)
				// Only Super ^as
				.layer(axum::middleware::from_fn(auth::validate::flow::only_supers))
				// Get/Modify Site
				.route("/sites", get(ends::admin::get_sites).post(ends::admin::post_site))
				.route(
					"/sites/:site_id",
					put(ends::admin::put_site).delete(ends::admin::del_site),
				)
				// Get/Modify User
				.route(
					"/sites/:site_id/users",
					get(ends::admin::get_users).post(ends::admin::post_user),
				)
				.route(
					"/sites/:site_id/users/:id",
					patch(ends::admin::put_user).delete(ends::admin::del_user),
				)
				// Only Admins ^
				.layer(axum::middleware::from_fn(auth::validate::flow::only_admins)),
		)
		// User Actions V
		.merge(
			Router::new()
				.route("/reset", post(crate::ends::reset))
				.route("/register", post(crate::ends::register)), // .layer(security_limit(1, 10)),
		)
		.merge(
			Router::new()
				.route("/user", get(crate::ends::user).patch(crate::ends::user_patch))
				.route("/logout", get(crate::ends::logout)), // .layer(security_limit(1, 1)),
		)
		.route("/login", post(crate::ends::login))
		
		// The request limiter :)
		.layer(
			ServiceBuilder::new()
				.layer(HandleErrorLayer::new(|e: BoxError| async move { display_error(e) }))
				.layer(GovernorLayer {
					config: Box::leak(governor_conf),
				}),
		)
		.merge(
			Router::new()
				.route("/user/:user/photo", get(crate::ends::user_photo))
				.layer(
					ServiceBuilder::new()
						.layer(HandleErrorLayer::new(|e: BoxError| async move { display_error(e) }))
						.layer(GovernorLayer {
							config: Box::leak(governor_conf_relaxed),
						}),
				),
		)
		.layer(axum::middleware::from_fn(auth::validate::authenticate))
		// Serves static assets
		.fallback_service(static_routes())
		.layer(TimeoutLayer::new(Duration::from_secs(30)))
		.layer(
			tower::ServiceBuilder::new()
				.layer(HandleErrorLayer::new(|_| async { StatusCode::SERVICE_UNAVAILABLE }))
				.concurrency_limit(100)
				.layer(Extension(db.clone()))
				.layer(Extension(cache.clone()))
				.layer(Extension(shutdown_rx.clone())), // .layer(Extension(resource_tx.clone())),
		);
	// If we're local, then allow cors
	// if URL.domain().and_then(|v| Some(v == "localhost")) == Some(true) {
	// }
	// app = app.layer(CorsLayer::permissive());

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

	wait_terminate().await;

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
