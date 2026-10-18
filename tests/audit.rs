// Project: shoptet-scraper
// File: tests/audit.rs
// Description: Differential observations captured from the Python original.
// Author: Jan Alexandr Kopřiva
// License: MIT
mod common;
use shoptet_scraper::{decode_xml, extract_image_data, output::save_workbook, parse_xml};
#[test]
fn python_xml_oracle() {
    let mut differences = vec![];
    for line in include_str!("fixtures/audit/oracle.tsv").lines() {
        let (name, expected) = line.split_once('\t').unwrap();
        let bytes = std::fs::read(format!("tests/fixtures/audit/{name}.xml")).unwrap();
        let actual = match decode_xml(&bytes).and_then(|s| {
            let d = parse_xml(&s)?;
            let rows = extract_image_data(&d);
            Ok(format!(
                "ok:{}:{}",
                rows.len(),
                rows.first().map(|r| r.name.as_str()).unwrap_or("")
            ))
        }) {
            Ok(s) => s,
            Err(_) => "error".into(),
        };
        let encoded: String = actual.bytes().map(|b| format!("{b:02x}")).collect();
        println!("{name}: {actual:?}");
        // Namespaced Google fields are a deliberate compatibility improvement.
        if name == "google_namespace" {
            assert_eq!(actual, "ok:1:name");
            continue;
        }
        // The Rust parser deliberately refuses every actual document type declaration.
        if name == "empty_doctype" {
            assert_eq!(actual, "error");
            continue;
        }
        if encoded != expected {
            differences.push(name);
        }
    }
    assert!(differences.is_empty(), "Differences: {differences:?}");
}
#[test]
fn long_spreadsheet_cells_are_truncated_like_openpyxl() {
    let t = common::Temp::default();
    let path = t.0.join("long.xlsx");
    save_workbook(&[vec!["x".repeat(40000)]], &path, 1).unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut xml = String::new();
    use std::io::Read;
    archive
        .by_name("xl/sharedStrings.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    assert!(xml.contains(&format!("<t>{}</t>", "x".repeat(32767))));
    assert!(!xml.contains(&"x".repeat(32768)));
}
#[test]
fn excessive_spreadsheet_columns_return_error() {
    let t = common::Temp::default();
    assert!(save_workbook(&[vec!["x".into(); 12]], &t.0.join("wide.xlsx"), 1).is_err());
}

#[test]
fn html_charset_preserves_gallery_urls_and_alt_text() {
    let bytes = include_bytes!("fixtures/audit/gallery-cp1250.html");
    let expected = include_str!("fixtures/audit/gallery-cp1250.txt");
    let (url, alt) = expected.split_once('\t').unwrap();
    let rows = vec![(url.to_owned(), alt.to_owned())];
    assert_ne!(
        shoptet_scraper::gallery(&String::from_utf8_lossy(bytes)),
        rows
    );
    assert_eq!(
        shoptet_scraper::gallery(&shoptet_scraper::decode_html(bytes)),
        rows
    );
}
