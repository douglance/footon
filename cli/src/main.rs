#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    footon::cli::app().serve().await
}
