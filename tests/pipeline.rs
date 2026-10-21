// Project: shoptet-scraper
// File: pipeline.rs
// Description: End-to-end local feed, gallery, image and workbook regressions.
// Author: Jan Alexandr Kopřiva
// License: MIT
mod common;
use common::Temp;
use httpmock::{Method::GET, MockServer};
use shoptet_scraper::{
    app::{gallery_data, run_scraper, store_config},
    gallery,
    network::Http,
    output::{image_filename, jpeg, session_path, Logger},
    ImageData, FEEDS,
};
use std::{
    io::{Cursor, Read},
    path::Path,
};
fn png() -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        1,
        1,
        image::Rgba([0, 0, 0, 0]),
    ))
    .write_to(&mut bytes, image::ImageFormat::Png)
    .unwrap();
    bytes.into_inner()
}
fn xml_file(path: &Path, member: &str) -> String {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut s = String::new();
    z.by_name(member).unwrap().read_to_string(&mut s).unwrap();
    s
}
#[test]
fn numbered_sessions_fill_gaps_without_overwriting() {
    let t = Temp::default();
    let a = session_path(&t.0, "2026-10-17").unwrap();
    let b = session_path(&t.0, "2026-10-17").unwrap();
    assert!(a.ends_with("2026-10-17_001"));
    assert!(b.ends_with("2026-10-17_002"));
    std::fs::remove_dir(&a).unwrap();
    assert_eq!(session_path(&t.0, "2026-10-17").unwrap(), a);
}
#[test]
fn gallery_normalizes_before_deduplicating_and_preserves_alt() {
    assert_eq!(gallery("<a class='p-main-image' href='https://x/big/a'><img alt='main'></a><a class='p-thumbnail' href='https://x/big/a'></a><a class='p-thumbnail' href='https://x/big/b'><img alt='second'></a><img data-src='https://x/related/b'><img data-src='https://x/related/c' alt='lazy'>"), vec![("https://x/orig/a".into(),"main".into()),("https://x/orig/b".into(),"second".into()),("https://x/orig/c".into(),"lazy".into())]);
}
#[test]
fn transparent_images_are_white_jpeg() {
    let t = Temp::default();
    let path = t.0.join("a.jpg");
    jpeg(&png(), &path).unwrap();
    let p = image::open(path).unwrap().to_rgb8();
    assert!(p.get_pixel(0, 0).0.iter().all(|v| *v >= 250));
}
#[test]
fn gallery_mode_produces_multiple_rows_and_cleans_xml() {
    let server = MockServer::start();
    let base = server.base_url();
    let feed=server.mock(|w,t|{w.method(GET).path("/universal.xml");t.body(format!("<SHOP><SHOPITEM><PRODUCT>=name</PRODUCT><PRODUCTNO>A/1</PRODUCTNO><URL>{base}/product</URL></SHOPITEM></SHOP>"));});
    let page=server.mock(|w,t|{w.path("/product");t.body(format!("<a class='p-main-image' href='{base}/big/a'></a><a class='p-thumbnail' href='{base}/big/b'></a>"));});
    let images = server.mock(|w, t| {
        w.path_matches("/orig/.*");
        t.body(png());
    });
    let tmp = Temp::default();
    let log = Logger::new(&tmp.0.join("log"), false).unwrap();
    let http = Http::with_policy(|_| true, |_| true).unwrap();
    let store = store_config(&base).unwrap();
    run_scraper(&http, &log, &store, FEEDS[0], &tmp.0, true).unwrap();
    let dir = tmp
        .0
        .join(shoptet_scraper::security::sanitize_filename(&store.name));
    let files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert!(!files
        .iter()
        .any(|p| p.extension().is_some_and(|e| e == "xml")));
    let xlsx = files
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "xlsx"))
        .unwrap();
    let sheet = xml_file(xlsx, "xl/worksheets/sheet1.xml");
    assert!(sheet.contains("r=\"3\""));
    let strings = xml_file(xlsx, "xl/sharedStrings.xml");
    assert!(strings.contains("'=name"));
    assert!(strings.contains("=name_A_1_1_1.jpg"));
    assert!(strings.contains("=name_A_1_1_2.jpg"));
    assert!(xml_file(xlsx, "xl/workbook.xml").contains("name=\"Sheet\""));
    feed.assert();
    page.assert();
    images.assert_calls(2);
}
#[test]
fn image_failures_still_write_rows_and_split_at_1000() {
    let server = MockServer::start();
    let xml = format!(
        "<SHOP>{}</SHOP>",
        "<SHOPITEM><PRODUCT>x</PRODUCT><IMGURL>file:///blocked</IMGURL></SHOPITEM>".repeat(1001)
    );
    let m = server.mock(|w, t| {
        w.path("/universal.xml");
        t.body(xml);
    });
    let base = server.base_url();
    let allow = base.clone();
    let http = Http::with_policy(move |u| u.starts_with(&allow), |_| true).unwrap();
    let tmp = Temp::default();
    let log = Logger::new(&tmp.0.join("log"), false).unwrap();
    let store = store_config(&base).unwrap();
    run_scraper(&http, &log, &store, FEEDS[0], &tmp.0, false).unwrap();
    let mut files: Vec<_> = std::fs::read_dir(
        tmp.0
            .join(shoptet_scraper::security::sanitize_filename(&store.name)),
    )
    .unwrap()
    .map(|e| e.unwrap().path())
    .filter(|p| p.extension().is_some_and(|e| e == "xlsx"))
    .collect();
    files.sort();
    assert_eq!(files.len(), 2);
    assert!(xml_file(&files[0], "xl/worksheets/sheet1.xml").contains("r=\"1001\""));
    assert!(xml_file(&files[1], "xl/worksheets/sheet1.xml").contains("r=\"2\""));
    m.assert();
}
#[test]
fn no_images_retains_xml_and_does_not_write_workbook() {
    let server = MockServer::start();
    let m = server.mock(|w, t| {
        w.path("/universal.xml");
        t.body("<SHOP/>");
    });
    let tmp = Temp::default();
    let log = Logger::new(&tmp.0.join("log"), false).unwrap();
    let store = store_config(&server.base_url()).unwrap();
    run_scraper(
        &Http::with_policy(|_| true, |_| true).unwrap(),
        &log,
        &store,
        FEEDS[0],
        &tmp.0,
        false,
    )
    .unwrap();
    let files: Vec<_> = std::fs::read_dir(
        tmp.0
            .join(shoptet_scraper::security::sanitize_filename(&store.name)),
    )
    .unwrap()
    .map(|e| e.unwrap().path())
    .collect();
    assert!(files
        .iter()
        .any(|p| p.extension().is_some_and(|e| e == "xml")));
    assert!(!files
        .iter()
        .any(|p| p.extension().is_some_and(|e| e == "xlsx")));
    m.assert();
}
#[test]
fn custom_domain_nonuniversal_feed_is_downloaded() {
    let server = MockServer::start();
    let m = server.mock(|w, t| {
        w.path("/google.xml");
        t.body("<rss/>");
    });
    let tmp = Temp::default();
    let log = Logger::new(&tmp.0.join("log"), false).unwrap();
    let store = store_config(&server.base_url()).unwrap();
    run_scraper(
        &Http::with_policy(|_| true, |_| true).unwrap(),
        &log,
        &store,
        FEEDS[1],
        &tmp.0,
        false,
    )
    .unwrap();
    m.assert();
}

#[test]
fn explicit_port_does_not_make_windows_filename_invalid() {
    let store = store_config("https://www.example.cz:8443/path").unwrap();
    assert_eq!(store.feed_filename, "feed_example_8443");
    assert_eq!(store.base_url, "https://www.example.cz:8443");
}
#[test]
fn image_pixel_limit_is_checked_before_decode() {
    let t = Temp::default();
    // Use the exact 60-million-pixel header opened by the Python oracle.
    let bytes = include_bytes!("fixtures/audit/large-header.png");
    let error = jpeg(bytes, &t.0.join("large.jpg")).unwrap_err().to_string();
    assert!(error.contains("50000000"), "{error}");
}

#[test]
fn malformed_feed_is_retained_and_logged_without_a_workbook() {
    let server = MockServer::start();
    let feed = server.mock(|w, t| {
        w.path("/universal.xml");
        t.body("<SHOP><SHOPITEM></SHOP>");
    });
    let tmp = Temp::default();
    let log_path = tmp.0.join("log");
    let logger = Logger::new(&log_path, false).unwrap();
    let store = store_config(&server.base_url()).unwrap();
    run_scraper(
        &Http::with_policy(|_| true, |_| true).unwrap(),
        &logger,
        &store,
        FEEDS[0],
        &tmp.0,
        false,
    )
    .unwrap_err();
    let files: Vec<_> = std::fs::read_dir(
        tmp.0
            .join(shoptet_scraper::security::sanitize_filename(&store.name)),
    )
    .unwrap()
    .map(|e| e.unwrap().path())
    .collect();
    assert!(files
        .iter()
        .any(|p| p.extension().is_some_and(|e| e == "xml")));
    assert!(!files
        .iter()
        .any(|p| p.extension().is_some_and(|e| e == "xlsx")));
    assert!(std::fs::read_to_string(log_path)
        .unwrap()
        .contains("Could not parse the feed"));
    feed.assert();
}

#[test]
fn logging_appends_and_verbose_controls_debug_messages() {
    let tmp = Temp::default();
    let path = tmp.0.join("log");
    {
        let logger = Logger::new(&path, false).unwrap();
        logger.info(" first");
        logger.debug(" hidden");
    }
    {
        let logger = Logger::new(&path, true).unwrap();
        logger.debug(" second");
    }
    let text = std::fs::read_to_string(path).unwrap();
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with(" - INFO -  first"));
    assert!(lines[1].ends_with(" - DEBUG -  second"));
    assert_eq!(lines[0].as_bytes()[19], b',');
}

#[test]
fn concurrent_runs_create_distinct_session_directories() {
    let tmp = Temp::default();
    let mut paths = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let base = &tmp.0;
                scope.spawn(move || session_path(base, "2026-10-18").unwrap())
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), 8);
}

#[test]
fn gallery_mode_accepts_google_item_links() {
    let server = MockServer::start();
    let page = server.mock(|w, t| {
        w.path("/product");
        t.body("<a class='p-main-image' href='https://x/big/a'></a>");
    });
    let xml = format!(
        "<rss><item><id>A</id><title>name</title><link>{}/product</link></item></rss>",
        server.base_url()
    );
    let doc = shoptet_scraper::parse_xml(&xml).unwrap();
    let tmp = Temp::default();
    let logger = Logger::new(&tmp.0.join("log"), false).unwrap();
    let data = shoptet_scraper::app::gallery_data(
        &Http::with_policy(|_| true, |_| true).unwrap(),
        &logger,
        &doc,
    );
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].number, "A");
    assert_eq!(data[0].suffix, "1_1");
    page.assert();
}

#[test]
fn a_failed_feed_download_is_an_error() {
    let server = MockServer::start();
    server.mock(|w, t| {
        w.path("/universal.xml");
        t.status(404);
    });
    let tmp = Temp::default();
    let log = Logger::new(&tmp.0.join("log"), false).unwrap();
    let store = store_config(&server.base_url()).unwrap();
    let error = run_scraper(
        &Http::with_policy(|_| true, |_| true).unwrap(),
        &log,
        &store,
        FEEDS[0],
        &tmp.0,
        false,
    )
    .unwrap_err();
    assert!(error.to_string().contains("Could not download"), "{error}");
}
#[test]
fn rows_follow_the_feed_not_the_download_order() {
    let server = MockServer::start();
    let base = server.base_url();
    server.mock(|w,t|{w.path("/universal.xml");t.body(format!("<SHOP><SHOPITEM><PRODUCT>First</PRODUCT><PRODUCTNO>1</PRODUCTNO><IMGURL>{base}/slow.png</IMGURL></SHOPITEM><SHOPITEM><PRODUCT>Second</PRODUCT><PRODUCTNO>2</PRODUCTNO><IMGURL>{base}/fast.png</IMGURL></SHOPITEM></SHOP>"));});
    server.mock(|w, t| {
        w.path("/slow.png");
        t.delay(std::time::Duration::from_millis(800)).body(png());
    });
    server.mock(|w, t| {
        w.path("/fast.png");
        t.body(png());
    });
    let tmp = Temp::default();
    let log = Logger::new(&tmp.0.join("log"), false).unwrap();
    let store = store_config(&base).unwrap();
    run_scraper(
        &Http::with_policy(|_| true, |_| true).unwrap(),
        &log,
        &store,
        FEEDS[0],
        &tmp.0,
        false,
    )
    .unwrap();
    let book = std::fs::read_dir(
        tmp.0
            .join(shoptet_scraper::security::sanitize_filename(&store.name)),
    )
    .unwrap()
    .map(|e| e.unwrap().path())
    .find(|p| p.extension().is_some_and(|e| e == "xlsx"))
    .unwrap();
    let strings = xml_file(&book, "xl/sharedStrings.xml");
    assert!(strings.find("First").unwrap() < strings.find("Second").unwrap());
}
#[test]
fn gallery_images_linked_relative_to_the_product_page_are_resolved() {
    let server = MockServer::start();
    let base = server.base_url();
    server.mock(|w, t| {
        w.path("/shop/product");
        t.body("<a class='p-main-image' href='/user/shop/big/a.jpg'></a>");
    });
    let xml = format!(
        "<SHOP><SHOPITEM><PRODUCT>P</PRODUCT><URL>{base}/shop/product</URL></SHOPITEM></SHOP>"
    );
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let tmp = Temp::default();
    let log = Logger::new(&tmp.0.join("log"), false).unwrap();
    let data = gallery_data(&Http::with_policy(|_| true, |_| true).unwrap(), &log, &doc);
    assert_eq!(data[0].url, format!("{base}/user/shop/orig/a.jpg"));
}
#[test]
fn a_long_product_name_cannot_exceed_the_file_name_limit() {
    let image = ImageData {
        index: 1,
        suffix: "7".into(),
        url: String::new(),
        number: "ABC".into(),
        name: "Ž".repeat(300),
    };
    let name = image_filename(&image);
    assert!(name.len() <= 255, "{}", name.len());
    assert!(name.ends_with("_ABC_7.jpg"));
}
