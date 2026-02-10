#[path = "../.thebe/mod.rs"]
#[allow(clippy::all, unused)]
mod thebe;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let app = thebe::router();

  let addr = "0.0.0.0:3000";
  println!("Listening on http://{addr}");
  let listener = tokio::net::TcpListener::bind(addr).await?;
  axum::serve(listener, app).await?;

  Ok(())
}
