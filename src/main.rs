// Project: shoptet-scraper
// File: main.rs
// Description: Command line entry point.
// Author: Jan Alexandr Kopřiva
// Based on shoptet-scraper by Jan Alexandr Kopřiva
// License: MIT
use clap::Parser;
use shoptet_scraper::{
    app,
    network::Http,
    output::{session_path, Logger},
    Result, FEEDS,
};
use std::path::{Path, PathBuf};
#[derive(Parser)]
#[command(
    about = "Universal Shoptet scraper for XML feeds and products",
    disable_version_flag = true,
    infer_long_args = true,
    args_override_self = true
)]
struct Args {
    #[arg(value_parser = ["eshop"], help = "Discover Shoptet shops")]
    command: String,
    #[arg(long, help = "Shoptet shop URL")]
    store: Option<String>,
    #[arg(long, help = "Output directory (default: ./output)")]
    output: Option<PathBuf>,
    #[arg(short, long, help = "Enable DEBUG logging")]
    verbose: bool,
    #[arg(short = 'a', long, help = "Download every product gallery image")]
    all_images: bool,
}
fn run(args: Args) -> Result<()> {
    let logger = Logger::new(Path::new("shoptet_scraper.log"), args.verbose)?;
    let base = args
        .output
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("output"));
    let session = session_path(&base, &chrono::Local::now().format("%Y-%m-%d").to_string())?;
    logger.info(&format!(" Session: {}", session.display()));
    let http = Http::new()?;
    if let Some(url) = args.store.filter(|s| !s.is_empty()) {
        if let Some(store) = app::discover(&http, &logger, &url) {
            logger.info(&format!(" Scraping shop: {} (Universal feed)", store.name));
            app::run_scraper(&http, &logger, &store, FEEDS[0], &session, args.all_images)?;
        } else {
            logger.error(&format!(" No Shoptet shop at URL: {url}"));
        }
    } else {
        app::interactive(&http, &logger, &session, args.all_images)?;
    }
    Ok(())
}
fn main() {
    let args = Args::parse();
    if let Err(e) = ctrlc::set_handler(|| {
        println!("\n Shutting down...");
        std::process::exit(0);
    }) {
        eprintln!("Could not install interrupt handler: {e}");
    }
    if let Err(e) = run(args) {
        println!(" Unexpected error: {e}");
        std::process::exit(1);
    }
}
