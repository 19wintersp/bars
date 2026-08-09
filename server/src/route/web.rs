use axum::Router;
use axum::routing::get;

pub fn router() -> Router {
	Router::new()
		.route("/", get(async || "Hello, world!"))
		.route("/static/{*path}", get(async || "Hello, world!"))
}
