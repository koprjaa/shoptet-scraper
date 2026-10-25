// Project: shoptet-scraper
// File: lib.rs
// Description: Feed parsing and safe output values.
// Author: Jan Alexandr Kopřiva
// License: AGPL-3.0-or-later
pub mod app;
pub mod network;
pub mod output;
pub mod security;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
pub const HEADERS: [&str; 11] = [
    "PRODUCT",
    "URL",
    "PRICE",
    "PRICE_VAT",
    "VAT",
    "MANUFACTURER",
    "CATEGORYTEXT",
    "EAN",
    "PRODUCTNO",
    "DELIVERY_DATE",
    "IMAGE_FILENAME",
];
#[derive(Clone, Copy, Debug)]
pub struct Feed {
    pub key: &'static str,
    pub name: &'static str,
    pub path: &'static str,
}
pub const FEEDS: [Feed; 5] = [
    Feed {
        key: "universal",
        name: "Universal XML Feed",
        path: "/universal.xml",
    },
    Feed {
        key: "google",
        name: "Google Shopping XML Feed",
        path: "/google.xml",
    },
    Feed {
        key: "heureka",
        name: "Heureka XML Feed",
        path: "/heureka.xml",
    },
    Feed {
        key: "zbozi",
        name: "Zboží.cz XML Feed",
        path: "/zbozi.xml",
    },
    Feed {
        key: "shoptet_supplier",
        name: "Shoptet Supplier XML Feed",
        path: "/supplier.xml",
    },
];
pub fn feed(key: &str) -> Feed {
    FEEDS
        .iter()
        .find(|f| f.key == key)
        .copied()
        .unwrap_or(FEEDS[0])
}
pub fn parse_xml(text: &str) -> Result<roxmltree::Document<'_>> {
    Ok(roxmltree::Document::parse(text)?)
}
pub fn first_text(node: roxmltree::Node<'_, '_>, tags: &[&str], default: &str) -> String {
    tags.iter()
        .find_map(|tag| {
            node.children()
                .find(|n| n.is_element() && n.tag_name().name() == *tag)
                .map(|n| {
                    // ElementTree joins text around comments and processing instructions.
                    n.children()
                        .take_while(|child| !child.is_element())
                        .filter(|child| child.is_text())
                        .map(|child| child.text().unwrap_or(""))
                        .collect::<String>()
                })
                .filter(|t| !t.is_empty())
        })
        .unwrap_or_else(|| default.to_owned())
}
pub fn find_items<'a, 'b>(doc: &'a roxmltree::Document<'b>) -> Vec<roxmltree::Node<'a, 'b>> {
    for tag in ["item", "SHOPITEM"] {
        let nodes: Vec<_> = doc
            .root_element()
            .descendants()
            .skip(1)
            .filter(|n| n.is_element() && n.tag_name().name() == tag)
            .collect();
        if !nodes.is_empty() {
            return nodes;
        }
    }
    vec![]
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageData {
    pub index: usize,
    pub suffix: String,
    pub url: String,
    pub number: String,
    pub name: String,
}
pub fn extract_image_data(doc: &roxmltree::Document<'_>) -> Vec<ImageData> {
    find_items(doc)
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let url = first_text(*item, &["image_link", "IMGURL"], "");
            if url.is_empty() {
                return None;
            }
            let default = format!("product_{}", i + 1);
            Some(ImageData {
                index: i + 1,
                suffix: (i + 1).to_string(),
                url,
                number: first_text(*item, &["id", "PRODUCTNO"], &default),
                name: first_text(*item, &["title", "PRODUCT"], &default),
            })
        })
        .collect()
}
pub fn product_row(item: roxmltree::Node<'_, '_>, number: &str, filename: &str) -> Vec<String> {
    let mut row: Vec<_> = HEADERS[..8]
        .iter()
        .map(|tag| first_text(item, &[*tag], ""))
        .collect();
    row.extend([
        number.to_owned(),
        first_text(item, &["DELIVERY_DATE"], ""),
        filename.to_owned(),
    ]);
    row.into_iter()
        .map(|s| security::sanitize_xlsx_value(&s))
        .collect()
}
pub fn gallery(html: &str) -> Vec<(String, String)> {
    use scraper::{Html, Selector};
    let doc = Html::parse_document(html);
    let mut result = vec![];
    let img = Selector::parse("img").unwrap();
    for selector in ["a.p-main-image", "a.p-thumbnail", "img[data-src]"] {
        for (i, node) in doc.select(&Selector::parse(selector).unwrap()).enumerate() {
            if selector == "a.p-main-image" && i > 0 {
                break;
            }
            let lazy = selector.starts_with("img");
            let Some(url) = node.value().attr(if lazy { "data-src" } else { "href" }) else {
                continue;
            };
            if url.is_empty() || (lazy && !url.contains("/related/")) {
                continue;
            }
            let url = url.replace(if lazy { "/related/" } else { "/big/" }, "/orig/");
            let alt = if lazy {
                node.value().attr("alt")
            } else {
                node.select(&img).next().and_then(|n| n.value().attr("alt"))
            }
            .unwrap_or("")
            .to_owned();
            if !result.iter().any(|(u, _)| *u == url) {
                result.push((url, alt));
            }
        }
    }
    result
}

/// Decode the XML declaration before parsing; feed bytes need not be UTF-8.
pub fn decode_xml(bytes: &[u8]) -> Result<String> {
    let encoding = if let Some((encoding, _)) = encoding_rs::Encoding::for_bom(bytes) {
        encoding
    } else if bytes.starts_with(&[0, b'<', 0]) {
        encoding_rs::UTF_16BE
    } else if bytes.starts_with(&[b'<', 0]) {
        encoding_rs::UTF_16LE
    } else {
        let end = bytes
            .windows(2)
            .position(|w| w == b"?>")
            .map_or(0, |i| i + 2);
        let head = String::from_utf8_lossy(&bytes[..end]);
        let declaration = head
            .strip_prefix("<?xml ")
            .or_else(|| head.strip_prefix("<?xml\t"))
            .or_else(|| head.strip_prefix("<?xml\r"))
            .or_else(|| head.strip_prefix("<?xml\n"))
            .and_then(|s| s.split_once("?>").map(|(declaration, _)| declaration))
            .unwrap_or("");
        if let Some((_, tail)) = declaration.split_once("encoding") {
            let tail = tail
                .trim_start()
                .strip_prefix('=')
                .unwrap_or("")
                .trim_start();
            let quote = tail.chars().next().unwrap_or(' ');
            if !matches!(quote, '\'' | '"') {
                return Err("Invalid XML encoding declaration".into());
            }
            let name = tail[1..].split(quote).next().unwrap_or("");
            // Web encoding aliases differ from XML's strict ASCII and Latin-1 codecs.
            if matches!(name.to_ascii_lowercase().as_str(), "ascii" | "us-ascii") {
                if !bytes.is_ascii() {
                    return Err("Invalid ASCII XML encoding".into());
                }
                return Ok(String::from_utf8(bytes.to_vec())?);
            }
            if matches!(
                name.to_ascii_lowercase().as_str(),
                "iso-8859-1" | "iso8859-1" | "latin-1" | "latin1"
            ) {
                return Ok(bytes.iter().map(|b| char::from(*b)).collect());
            }
            encoding_rs::Encoding::for_label(name.as_bytes()).ok_or("Unknown XML encoding")?
        } else {
            encoding_rs::UTF_8
        }
    };
    let (text, _, errors) = encoding.decode(bytes);
    if errors {
        return Err("Invalid XML encoding".into());
    }
    Ok(text.into_owned())
}

/// Honor the page's byte-order mark and HTML charset metadata before selecting images.
pub fn decode_html(bytes: &[u8]) -> String {
    if let Some((encoding, _)) = encoding_rs::Encoding::for_bom(bytes) {
        return encoding.decode(bytes).0.into_owned();
    }
    let preview = String::from_utf8_lossy(bytes);
    let doc = scraper::Html::parse_document(&preview);
    let selector = scraper::Selector::parse("meta").unwrap();
    for meta in doc.select(&selector) {
        let label = meta.value().attr("charset").map(str::to_owned).or_else(|| {
            if !meta
                .value()
                .attr("http-equiv")
                .is_some_and(|s| s.eq_ignore_ascii_case("content-type"))
            {
                return None;
            }
            let content = meta.value().attr("content")?;
            let lowered = content.to_ascii_lowercase();
            let position = lowered.find("charset=")? + 8;
            Some(
                content[position..]
                    .split(';')
                    .next()?
                    .trim()
                    .trim_matches(['\'', '"'])
                    .to_owned(),
            )
        });
        if let Some(encoding) = label.and_then(|s| encoding_rs::Encoding::for_label(s.as_bytes())) {
            return encoding.decode(bytes).0.into_owned();
        }
    }
    preview.into_owned()
}
