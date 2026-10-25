// Project: shoptet-scraper
// File: app.rs
// Description: Shop discovery, interactive selection and scraping pipeline.
// Author: Jan Alexandr Kopřiva
// License: AGPL-3.0-or-later
use crate::{
    extract_image_data, find_items, first_text, gallery,
    network::{Http, Session},
    output::{image_filename, jpeg, save_workbook, Logger},
    parse_xml, product_row,
    security::sanitize_filename,
    Feed, ImageData, Result, FEEDS,
};
use reqwest::Method;
use std::{
    fs,
    io::{self, Write},
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};
#[derive(Clone, Debug)]
pub struct Store {
    pub name: String,
    pub base_url: String,
    pub feed_filename: String,
    pub key: String,
}
pub fn store_config(url: &str) -> Option<Store> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return None;
    }
    let authority = parsed
        .as_str()
        .split_once("://")?
        .1
        .split(['/', '?', '#'])
        .next()?;
    let raw = authority
        .replace(".shoptet.cz", "")
        .replace(".cz", "")
        .replace("www.", "");
    let mut start = true;
    let name: String = raw
        .chars()
        .flat_map(|c| {
            let result: Vec<char> = if start {
                c.to_uppercase().collect()
            } else {
                c.to_lowercase().collect()
            };
            start = !c.is_alphabetic();
            result
        })
        .collect();
    let key = raw.replace(['-', '.'], "_");
    Some(Store {
        name,
        base_url: format!("{}://{authority}", parsed.scheme()),
        feed_filename: format!("feed_{}", sanitize_filename(&key)),
        key,
    })
}
pub fn discover(http: &Http, logger: &Logger, url: &str) -> Option<Store> {
    let store = store_config(url)?;
    let base = &store.base_url;
    if !http.safe(base) {
        logger.warn(&format!(" Refused unsafe shop URL: {base}"));
        return None;
    }
    for path in ["/universal.xml", "/google.xml", "/heureka.xml"] {
        let probe = format!("{base}{path}");
        match http.request(Method::GET, &probe, 15, Session::Discovery) {
            Ok(r) if r.status().as_u16() == 200 => {
                if r.headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_lowercase()
                    .contains("xml")
                {
                    logger.info(&format!(" Shoptet shop found: {base} (feed: {probe})"));
                    return Some(store);
                }
                if let Ok(text) = http.preview(r) {
                    if ["<?xml", "<SHOP", "<rss"].iter().any(|s| text.contains(s)) {
                        logger.info(&format!(" Shoptet shop found: {base} (XML body detected)"));
                        return Some(store);
                    }
                }
            }
            Err(e) => logger.debug(&format!(" Probe {probe} failed: {e}")),
            _ => (),
        }
    }
    match http.request(Method::GET, base, 10, Session::Discovery) {
        Ok(r) if r.status().as_u16() == 200 => {
            if r.text()
                .unwrap_or_default()
                .to_lowercase()
                .contains("shoptet")
            {
                logger.info(&format!(
                    " Shoptet shop found: {base} (named on the home page)"
                ));
                return Some(store);
            }
        }
        Err(e) => logger.debug(&format!(" Home page check failed: {e}")),
        _ => (),
    }
    None
}
pub fn available(http: &Http, logger: &Logger, store: &Store, feed: Feed) -> bool {
    let url = format!("{}{}", store.base_url, feed.path);
    match http
        .fresh()
        .and_then(|session| session.request(Method::HEAD, &url, 10, Session::Feed))
    {
        Ok(r) if r.status().as_u16() == 200 => {
            logger.info(&format!(" {} available: {url}", feed.name));
            true
        }
        Ok(r) => {
            logger.warn(&format!(
                " {} unavailable (status {}): {url}",
                feed.name,
                r.status().as_u16()
            ));
            false
        }
        Err(e) => {
            logger.warn(&format!(" {} unavailable ({e}): {url}", feed.name));
            false
        }
    }
}
pub fn run_scraper(
    http: &Http,
    logger: &Logger,
    store: &Store,
    feed: Feed,
    session: &Path,
    all: bool,
) -> Result<()> {
    let http = http.fresh()?;
    let http = &http;
    logger.info(&format!(
        " Scraping shop: {}, feed: {}, mode: {}",
        store.name,
        feed.name,
        if all {
            "all gallery images"
        } else {
            "main images only"
        }
    ));
    let folder = session.join(sanitize_filename(&store.name));
    let images = folder.join("images");
    fs::create_dir_all(&images)?;
    let url = format!("{}{}", store.base_url.trim_end_matches('/'), feed.path);
    if !http.safe(&url) {
        logger.error(&format!(" Invalid URL: {url}"));
        return Err(format!("Invalid URL: {url}").into());
    }
    logger.info(&format!(" Downloading {} from: {url}", feed.name));
    let xml_path = folder.join(format!(
        "{}_{}_{}.xml",
        store.feed_filename,
        feed.key,
        chrono::Local::now().format("%d-%m-%Y")
    ));
    let downloaded = (|| -> Result<()> {
        let mut response = http
            .request(Method::GET, &url, 30, Session::Feed)?
            .error_for_status()?;
        let mut file = fs::File::create(&xml_path)?;
        io::copy(&mut response, &mut file)?;
        Ok(())
    })();
    if let Err(e) = downloaded {
        logger.error(&format!(" Could not download {}: {e}", feed.name));
        logger.error(" Could not download the feed");
        return Err(format!("Could not download {}: {e}", feed.name).into());
    }
    logger.info(&format!(
        " {} downloaded: {}",
        feed.name,
        xml_path.display()
    ));
    let text = match fs::read(&xml_path).and_then(|b| {
        crate::decode_xml(&b).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }) {
        Ok(text) => text,
        Err(e) => {
            logger.error(&format!(" Could not parse {}: {e}", feed.name));
            logger.error(" Could not parse the feed");
            return Err(format!("Could not parse {}: {e}", feed.name).into());
        }
    };
    let doc = match parse_xml(&text) {
        Ok(doc) => doc,
        Err(e) => {
            logger.error(&format!(" Could not parse {}: {e}", feed.name));
            logger.error(" Could not parse the feed");
            return Err(format!("Could not parse {}: {e}", feed.name).into());
        }
    };
    logger.info(&format!(" {} parsed: {}", feed.name, xml_path.display()));
    let data = if all {
        gallery_data(http, logger, &doc)
    } else {
        let data = extract_image_data(&doc);
        logger.info(&format!(" Found {} images in {}", data.len(), feed.name));
        data
    };
    if data.is_empty() {
        logger.warn(" No images to download");
        return Ok(());
    }
    process_products(http, logger, store, feed, &doc, &data, &images, &folder)?;
    match fs::remove_file(&xml_path) {
        Ok(()) => logger.info(&format!(" Feed file deleted: {}", xml_path.display())),
        Err(e) => logger.error(&format!(" Could not delete the feed file: {e}")),
    }
    logger.info(" Scraping finished");
    Ok(())
}
pub fn gallery_data(http: &Http, logger: &Logger, doc: &roxmltree::Document<'_>) -> Vec<ImageData> {
    let items = find_items(doc);
    logger.info(&format!(
        " Reading galleries for {} products...",
        items.len()
    ));
    let mut result = vec![];
    for (i, item) in items.iter().enumerate() {
        let url = first_text(*item, &["URL", "link"], "");
        if url.is_empty() {
            continue;
        }
        if !http.safe(&url) {
            logger.error(&format!(" Refused unsafe product URL: {url}"));
            continue;
        }
        match http.bytes(&url, 15, Session::Discovery) {
            Ok(bytes) => {
                let images = gallery(&crate::decode_html(&bytes));
                logger.info(&format!(
                    " Found {} images for product: {url}",
                    images.len()
                ));
                let page = reqwest::Url::parse(&url).ok();
                for (j, (image, _)) in images.into_iter().enumerate() {
                    // Galleries may link images relative to the product page.
                    let image = page
                        .as_ref()
                        .and_then(|page| page.join(&image).ok())
                        .map_or(image, String::from);
                    let default = format!("product_{}", i + 1);
                    result.push(ImageData {
                        index: i + 1,
                        suffix: format!("{}_{}", i + 1, j + 1),
                        url: image,
                        number: first_text(*item, &["id", "PRODUCTNO"], &default),
                        name: first_text(*item, &["title", "PRODUCT"], &default),
                    });
                }
            }
            Err(e) => logger.error(&format!(" Could not read images from {url}: {e}")),
        }
    }
    logger.info(&format!(" Found {} gallery images in total", result.len()));
    result
}
#[allow(clippy::too_many_arguments)]
pub fn process_products(
    http: &Http,
    logger: &Logger,
    store: &Store,
    feed: Feed,
    doc: &roxmltree::Document<'_>,
    data: &[ImageData],
    images: &Path,
    folder: &Path,
) -> Result<()> {
    logger.info(&format!(
        " Processing {} images from {}",
        data.len(),
        feed.name
    ));
    let items = find_items(doc);
    let mut rows = vec![];
    let mut total = 0;
    let mut part = 1;
    let mut saved = vec![None; data.len()];
    let (tx, rx) = mpsc::channel();
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| -> Result<()> {
        for _ in 0..50.min(data.len()) {
            let tx = tx.clone();
            let next = &next;
            scope.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(image) = data.get(i) else {
                    break;
                };
                let filename = image_filename(image);
                let saved = match http.image(&image.url) {
                    Ok(Some(bytes)) => match jpeg(&bytes, &images.join(&filename)) {
                        Ok(()) => filename,
                        Err(e) => {
                            logger.error(&format!(" Could not download image {}: {e}", image.url));
                            String::new()
                        }
                    },
                    Ok(None) => {
                        logger.error(&format!(" Refused unsafe image URL: {}", image.url));
                        String::new()
                    }
                    Err(e) => {
                        logger.error(&format!(" Could not download image {}: {e}", image.url));
                        String::new()
                    }
                };
                if tx.send((i, saved)).is_err() {
                    break;
                }
            });
        }
        drop(tx);
        for (i, filename) in rx {
            saved[i] = Some(filename);
        }
        Ok(())
    })?;
    // Rows follow the feed, not the order the downloads happened to finish in.
    for (image, filename) in data.iter().zip(saved) {
        let filename = filename.unwrap_or_default();
        if let Some(item) = image.index.checked_sub(1).and_then(|i| items.get(i)) {
            rows.push(product_row(*item, &image.number, &filename));
            total += 1;
            if rows.len() == 1000 {
                save_part(&rows, folder, store, feed, part)?;
                rows.clear();
                part += 1;
            }
        }
    }
    if !rows.is_empty() {
        save_part(&rows, folder, store, feed, part)?;
    }
    logger.info(&format!(" {} done ({total} products)", feed.name));
    Ok(())
}
fn save_part(
    rows: &[Vec<String>],
    folder: &Path,
    store: &Store,
    feed: Feed,
    part: usize,
) -> Result<()> {
    let path = folder.join(format!(
        "{}_{}_{}_part_{part}.xlsx",
        store.feed_filename,
        feed.key,
        chrono::Local::now().format("%d-%m-%Y")
    ));
    save_workbook(rows, &path, part)
}
pub fn prompt(message: &str) -> Result<Option<String>> {
    print!("{message}");
    io::stdout().flush()?;
    let mut line = String::new();
    if io::stdin().read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim().to_owned()))
}
pub fn interactive(http: &Http, logger: &Logger, session: &Path, all: bool) -> Result<()> {
    println!("\n=== Universal Shoptet Scraper ===\nEnter Shoptet shop URLs (one per line, blank line to finish):");
    let mut urls = vec![];
    while let Some(url) = prompt("URL: ")? {
        if url.is_empty() {
            break;
        }
        urls.push(url);
    }
    if urls.is_empty() {
        logger.info(" No URLs given");
        return Ok(());
    }
    logger.info(&format!(
        " Checking {} URLs for Shoptet shops...",
        urls.len()
    ));
    let mut stores: Vec<Store> = vec![];
    for url in urls {
        if let Some(store) = discover(http, logger, &url) {
            if let Some(i) = stores.iter().position(|s| s.key == store.key) {
                stores[i] = store;
            } else {
                stores.push(store);
            }
        }
    }
    logger.info(&format!(" Found {} Shoptet shops", stores.len()));
    if stores.is_empty() {
        logger.error(" No Shoptet shops found");
        return Ok(());
    }
    logger.info(" Checking which feeds are available...");
    let stores: Vec<_> = stores
        .into_iter()
        .filter_map(|s| {
            let feeds: Vec<_> = FEEDS
                .into_iter()
                .filter(|f| available(http, logger, &s, *f))
                .collect();
            if feeds.is_empty() {
                None
            } else {
                Some((s, feeds))
            }
        })
        .collect();
    logger.info(&format!(" {} shops serve at least one feed", stores.len()));
    if stores.is_empty() {
        logger.error(" No available feeds found");
        return Ok(());
    }
    println!("\n=== Available Shoptet shops ({}) ===", stores.len());
    for (i, (store, feeds)) in stores.iter().enumerate() {
        println!(
            "{}. {} ({})\n   Available feeds: {}",
            i + 1,
            store.name,
            store.base_url,
            feeds.iter().map(|f| f.name).collect::<Vec<_>>().join(", ")
        );
    }
    println!("0. Quit");
    loop {
        let Some(choice) = prompt(&format!("\nSelect shop (0-{}): ", stores.len()))? else {
            return Ok(());
        };
        if choice == "0" {
            return Ok(());
        }
        let Ok(n) = choice.parse::<usize>() else {
            println!(" Invalid input, enter a number");
            continue;
        };
        let Some((store, feeds)) = n.checked_sub(1).and_then(|i| stores.get(i)) else {
            println!(" Invalid choice, try again");
            continue;
        };
        let selected = if feeds.len() == 1 {
            println!(" Automatically selected {}", feeds[0].name);
            feeds[0]
        } else {
            println!("\nAvailable feeds for {}:", store.name);
            for (i, feed) in feeds.iter().enumerate() {
                println!("{}. {}", i + 1, feed.name);
            }
            let Some(choice) = prompt(&format!("Select feed type (1-{}): ", feeds.len()))? else {
                return Ok(());
            };
            let Ok(n) = choice.parse::<usize>() else {
                println!(" Invalid input, enter a number");
                continue;
            };
            let Some(feed) = n.checked_sub(1).and_then(|i| feeds.get(i)) else {
                println!(" Invalid feed choice");
                continue;
            };
            *feed
        };
        return run_scraper(http, logger, store, selected, session, all);
    }
}
