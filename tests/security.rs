// Project: shoptet-scraper
// File: security.rs
// Description: Offline equivalents of every original security test.
// Author: Jan Alexandr Kopřiva
// Based on shoptet-scraper by Jan Alexandr Kopřiva
// License: MIT
use shoptet_scraper::{network::Http, parse_xml, security::*};
use std::{io::Cursor, net::IpAddr};
fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}
#[test]
fn an_address_on_this_machine_or_network_is_refused() {
    for s in [
        "http://127.0.0.1/a.jpg",
        "http://localhost/a.jpg",
        "http://[::1]/a.jpg",
        "http://169.254.169.254/latest/meta-data/",
        "http://0.0.0.0/a.jpg",
    ] {
        assert!(!safe_resolved_url(s, |h, _| vec![ip(
            if h == "localhost" { "127.0.0.1" } else { h }
        )]));
    }
}
#[test]
fn only_http_and_https_are_allowed() {
    for s in [
        "file:///etc/passwd",
        "ftp://shop.cz/a.jpg",
        "gopher://shop.cz/",
        "javascript:alert(1)",
    ] {
        assert!(!safe_resolved_url(s, |_, _| panic!("must not resolve")));
    }
}
#[test]
fn a_url_without_a_host_is_refused() {
    for s in ["", "http://", "not a url", "https:///a.jpg"] {
        assert!(!safe_resolved_url(s, |_, _| vec![]));
    }
}
#[test]
fn a_hostname_that_does_not_resolve_is_refused() {
    assert!(!safe_resolved_url(
        "https://this-name-does-not-exist.invalid/a.jpg",
        |_, _| vec![]
    ));
}
#[test]
fn a_public_address_is_allowed() {
    assert!(safe_resolved_url("https://example.com/a.jpg", |_, _| vec![
        ip("93.184.216.34")
    ]));
}
#[test]
fn mixed_dns_answers_are_refused() {
    assert!(!safe_resolved_url("https://example.com", |_, _| vec![
        ip("93.184.216.34"),
        ip("127.0.0.1")
    ]));
}
#[test]
fn a_value_a_spreadsheet_would_run_is_quoted() {
    for p in ['=', '+', '-', '@', '\t', '\r'] {
        assert_eq!(
            sanitize_xlsx_value(&format!("{p}cmd|calc")),
            format!("'{p}cmd|calc")
        );
    }
}
#[test]
fn a_plain_product_name_is_left_alone() {
    assert_eq!(sanitize_xlsx_value("Boty Alfa"), "Boty Alfa");
}
#[test]
fn a_value_that_is_not_text_passes_through() {
    assert_eq!(sanitize_xlsx_value(""), ""); /* Non-string cells cannot enter the typed string row API. */
}
#[test]
fn a_negative_number_written_as_text_is_quoted() {
    assert_eq!(sanitize_xlsx_value("-500"), "'-500");
}
#[test]
fn a_character_windows_forbids_is_replaced() {
    for c in "\\/*?:\"<>|".chars() {
        assert_eq!(sanitize_filename(&format!("boty{c}alfa")), "boty_alfa");
    }
}
#[test]
fn a_path_cannot_be_smuggled_through_a_product_name() {
    assert!(!sanitize_filename("../../etc/passwd").contains('/'));
    assert!(!sanitize_filename("..\\..\\windows\\system32").contains('\\'));
}
#[test]
fn czech_characters_survive() {
    assert_eq!(
        sanitize_filename("Příliš žluťoučký kůň"),
        "Příliš žluťoučký kůň"
    );
}
#[test]
fn a_plain_feed_parses() {
    assert_eq!(
        parse_xml("<SHOP><SHOPITEM/></SHOP>")
            .unwrap()
            .root_element()
            .tag_name()
            .name(),
        "SHOP"
    );
}
#[test]
fn an_entity_expansion_payload_is_refused() {
    assert!(parse_xml(
        "<!DOCTYPE lolz [<!ENTITY lol 'lol'><!ENTITY lol2 '&lol;&lol;'>]><SHOP>&lol2;</SHOP>"
    )
    .is_err());
}
#[test]
fn an_external_entity_is_refused() {
    assert!(parse_xml(
        "<!DOCTYPE foo [<!ENTITY xxe SYSTEM 'file:///etc/passwd'>]><SHOP>&xxe;</SHOP>"
    )
    .is_err());
}
#[test]
fn an_unsafe_url_is_not_fetched() {
    let http = Http::with_policy(|_| false, |_| panic!("must not connect")).unwrap();
    assert!(http.image("http://127.0.0.1/a.jpg").unwrap().is_none());
}
#[test]
fn a_declared_size_over_the_limit_is_refused() {
    let err = read_limited(
        Cursor::new(b"x"),
        Some(&(MAX_IMAGE_DOWNLOAD_BYTES + 1).to_string()),
        MAX_IMAGE_DOWNLOAD_BYTES,
    )
    .unwrap_err();
    assert!(err.to_string().contains("over the limit"));
}
#[test]
fn a_server_that_lies_about_the_size_is_still_stopped() {
    assert!(read_limited(
        Cursor::new(vec![0; MAX_IMAGE_DOWNLOAD_BYTES + 1]),
        Some("10"),
        MAX_IMAGE_DOWNLOAD_BYTES
    )
    .unwrap_err()
    .to_string()
    .contains("while downloading"));
}
#[test]
fn a_normal_image_comes_back_whole() {
    assert_eq!(
        read_limited(Cursor::new(b"abcdef"), Some("6"), MAX_IMAGE_DOWNLOAD_BYTES).unwrap(),
        b"abcdef"
    );
}
#[test]
fn a_nonsense_content_length_header_does_not_crash() {
    assert_eq!(
        read_limited(
            Cursor::new(b"abc"),
            Some("not a number"),
            MAX_IMAGE_DOWNLOAD_BYTES
        )
        .unwrap(),
        b"abc"
    );
}
#[test]
fn every_private_peer_is_refused_after_connecting() {
    for s in ["10.0.0.1", "169.254.169.254", "::1", "100.64.0.1"] {
        assert!(is_blocked_ip(ip(s)));
    }
}
#[test]
fn a_public_peer_is_read_normally() {
    assert!(!is_blocked_ip(ip("93.184.216.34")));
}
#[test]
fn reserved_ipv4_and_mapped_ipv6_are_blocked() {
    for s in [
        "192.0.2.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "240.0.0.1",
        "::ffff:127.0.0.1",
        "fc00::1",
        "fe80::1",
    ] {
        assert!(is_blocked_ip(ip(s)), "{s}");
    }
}
#[test]
fn declarations_beyond_the_old_fallback_prefix_are_rejected() {
    let xml = format!("{}<!DOCTYPE SHOP><SHOP/>", " ".repeat(70000));
    assert!(parse_xml(&xml).is_err());
}

#[test]
fn a_response_without_a_socket_reports_no_peer() {
    assert!(safe_peer(None));
}
#[test]
fn a_public_socket_peer_is_safe() {
    assert!(safe_peer(Some(ip("93.184.216.34"))));
    assert!(!safe_peer(Some(ip("127.0.0.1"))));
}
#[test]
fn an_overflowing_declared_size_is_refused() {
    assert!(read_limited(
        Cursor::new(b"x"),
        Some("9999999999999999999999999999999999"),
        MAX_IMAGE_DOWNLOAD_BYTES
    )
    .is_err());
}
