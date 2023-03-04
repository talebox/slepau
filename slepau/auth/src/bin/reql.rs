use futures_util::stream::TryStreamExt;
use reql::{r, Command, Session};
use serde_json::Value;

#[tokio::main]
async fn main() -> reql::Result<()> {
	println!("HAWO");
	// Create a RethinkDB connection out of the stream
	// See the API docs for more options you can configure
	let conn = r.connect(()).await?;

	// Create the query you want to run
	// The query returns a `Stream` of responses from RethinkDB
	let query = r.db("movies").table("meta").limit(10);

	run(query.clone(), &conn).await?;

	// r.expr(r.args(([1,2], func!(|v| v.sum()))));

	Ok(())
}

// We are just going to print the JSON response for this example
fn print_json(json: Value) {
	println!("{}", serde_json::to_string_pretty(&json).unwrap());
}

async fn run(c: Command, conn: &Session) -> reql::Result<()> {
	let mut stream = c.run(conn);
	while let Some(v) = stream.try_next().await? as Option<Value> {
		print_json(v)
	}

	Ok(())
}
