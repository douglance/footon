#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("fetch") {
        let url = args.next().ok_or("usage: footon fetch <share-url>")?;
        if args.next().is_some() {
            return Err("usage: footon fetch <share-url>".into());
        }
        let markdown = footon::fetch::fetch_markdown(&url).await?;
        print!("{markdown}");
        return Ok(());
    }
    footon::cli::app().serve().await
}
