// Project: shoptet-scraper
// File: network.rs
// Description: Local HTTP mocks for retries, peers, discovery and redirects.
// Author: Jan Alexandr Kopřiva
// Based on shoptet-scraper by Jan Alexandr Kopřiva
// License: MIT
mod common;
use common::Temp;
use httpmock::{
    Method::{GET, HEAD},
    MockServer,
};
use shoptet_scraper::{
    app::{available, discover},
    network::{backoff, Http, Session, BROWSER_UA, REQUESTS_UA},
    output::Logger,
    FEEDS,
};
use std::time::Duration;
fn local() -> Http {
    Http::with_policy(|_| true, |_| true).unwrap()
}
#[test]
fn the_peer_address_is_read_off_the_socket() {
    let server = MockServer::start();
    let m = server.mock(|w, t| {
        w.method(GET).path("/");
        t.body("ok");
    });
    let r = local()
        .request(reqwest::Method::GET, &server.base_url(), 10, Session::Feed)
        .unwrap();
    assert_eq!(r.remote_addr().unwrap().ip(), server.address().ip());
    m.assert();
}
#[test]
fn a_connection_that_landed_on_a_private_address_is_dropped() {
    let server = MockServer::start();
    let m = server.mock(|w, t| {
        w.path("/image");
        t.body("never read");
    });
    let http = Http::with_policy(|_| true, |_| false).unwrap();
    assert!(http
        .image(&server.url("/image"))
        .unwrap_err()
        .to_string()
        .contains("peer"));
    m.assert();
}
#[test]
fn a_public_peer_is_read_normally_with_mocked_policy() {
    let server = MockServer::start();
    let m = server.mock(|w, t| {
        w.path("/image");
        t.body("abcdef");
    });
    assert_eq!(
        local().image(&server.url("/image")).unwrap().unwrap(),
        b"abcdef"
    );
    m.assert();
}
#[test]
fn unsafe_redirect_is_not_followed() {
    let server = MockServer::start();
    let a = server.mock(|w, t| {
        w.path("/image");
        t.status(302)
            .header("location", "http://127.0.0.1:1/private");
    });
    let base = server.base_url();
    let allowed = base.clone();
    let http = Http::with_policy(move |u| u.starts_with(&allowed), |_| true).unwrap();
    assert!(http.image(&server.url("/image")).is_err());
    a.assert();
}
#[test]
fn a_valid_redirect_is_followed() {
    let server = MockServer::start();
    let a = server.mock(|w, t| {
        w.path("/a");
        t.status(302).header("location", "/b");
    });
    let b = server.mock(|w, t| {
        w.path("/b");
        t.body("abc");
    });
    assert_eq!(local().image(&server.url("/a")).unwrap().unwrap(), b"abc");
    a.assert();
    b.assert();
}
#[test]
fn retry_status_and_retry_after_are_honored() {
    let server = MockServer::start();
    let m = server.mock(|w, t| {
        w.path("/");
        t.status(500).header("retry-after", "0");
    });
    let r = local()
        .request(
            reqwest::Method::GET,
            &server.base_url(),
            10,
            Session::Discovery,
        )
        .unwrap();
    assert_eq!(r.status(), 500);
    m.assert_calls(4);
}
#[test]
fn feed_has_six_attempts() {
    let server = MockServer::start();
    let m = server.mock(|w, t| {
        w.path("/");
        t.status(502).header("retry-after", "0");
    });
    let r = local()
        .request(reqwest::Method::GET, &server.base_url(), 10, Session::Feed)
        .unwrap();
    assert_eq!(r.status(), 502);
    m.assert_calls(6);
}
#[test]
fn nonretryable_status_is_not_retried() {
    let server = MockServer::start();
    let m = server.mock(|w, t| {
        w.path("/");
        t.status(404);
    });
    assert!(local().image(&server.base_url()).is_err());
    m.assert_calls(1);
}
#[test]
fn backoff_matches_urllib3_schedule() {
    assert_eq!(
        (0..5).map(|i| backoff(i, 0.5)).collect::<Vec<_>>(),
        [0, 1, 2, 4, 8].map(Duration::from_secs)
    );
    assert_eq!(
        (0..3).map(|i| backoff(i, 1.0)).collect::<Vec<_>>(),
        [0, 2, 4].map(Duration::from_secs)
    );
}
#[test]
fn discovery_uses_browser_headers_and_xml_content_type() {
    let server = MockServer::start();
    let m = server.mock(|w, t| {
        w.method(GET)
            .path("/universal.xml")
            .header("user-agent", BROWSER_UA);
        t.header("content-type", "APPLICATION/XML");
    });
    let tmp = Temp::default();
    let log = Logger::new(&tmp.0.join("log"), false).unwrap();
    assert!(discover(&local(), &log, &server.url("/ignored?q=x")).is_some());
    m.assert();
}
#[test]
fn discovery_falls_back_to_body_and_homepage() {
    let server = MockServer::start();
    let mut m = server.mock(|w, t| {
        w.method(GET).path("/google.xml");
        t.body("<?xml version='1.0'?><rss/>");
    });
    let tmp = Temp::default();
    let log = Logger::new(&tmp.0.join("log"), false).unwrap();
    assert!(discover(&local(), &log, &server.base_url()).is_some());
    m.assert();
    m.delete();
    let home = server.mock(|w, t| {
        w.path("/");
        t.body("Powered by SHOPTeT");
    });
    assert!(discover(&local(), &log, &server.base_url()).is_some());
    home.assert();
}
#[test]
fn availability_uses_head_requests_default_headers_and_no_redirects() {
    let server = MockServer::start();
    let m = server.mock(|w, t| {
        w.method(HEAD)
            .path("/google.xml")
            .header("user-agent", REQUESTS_UA);
        t.status(302).header("location", "/other");
    });
    let tmp = Temp::default();
    let log = Logger::new(&tmp.0.join("log"), false).unwrap();
    let store = shoptet_scraper::app::store_config(&server.base_url()).unwrap();
    assert!(!available(&local(), &log, &store, FEEDS[1]));
    m.assert();
}
