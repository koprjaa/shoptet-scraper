// Project: shoptet-scraper
// File: pipeline.rs
// Description: End-to-end local feed, gallery, image and workbook regressions.
// Author: Jan Alexandr Kopřiva
// License: MIT
mod common;
use common::Temp;
use httpmock::{Method::GET, MockServer};
use shoptet_scraper::{
    app::{run_scraper, store_config},
    gallery,
    network::Http,
    output::{jpeg, session_path, Logger},
    FEEDS,
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
    // A valid PNG header advertises a bomb without allocating its pixel buffer.
    let mut bytes = png();
    bytes[16..20].copy_from_slice(&10000u32.to_be_bytes());
    bytes[20..24].copy_from_slice(&10000u32.to_be_bytes());
    let mut crc = u32::MAX;
    for byte in &bytes[12..29] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    bytes[29..33].copy_from_slice(&(!crc).to_be_bytes());
    let error = jpeg(&bytes, &t.0.join("large.jpg"))
        .unwrap_err()
        .to_string();
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
