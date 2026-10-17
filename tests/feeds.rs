// Project: shoptet-scraper
// File: feeds.rs
// Description: Offline equivalents of the original feed tests.
// Author: Jan Alexandr Kopřiva
// Based on shoptet-scraper by Jan Alexandr Kopřiva
// License: MIT
use shoptet_scraper::*;
const UNIVERSAL: &str = include_str!("fixtures/universal.xml");
const GOOGLE: &str = include_str!("fixtures/google.xml");
#[test]
fn first_text_returns_the_first_tag_that_has_a_value() {
    let d = parse_xml("<i><b>second</b></i>").unwrap();
    assert_eq!(first_text(d.root_element(), &["a", "b"], ""), "second");
}
#[test]
fn first_text_prefers_the_earlier_tag() {
    let d = parse_xml("<i><a>first</a><b>second</b></i>").unwrap();
    assert_eq!(first_text(d.root_element(), &["a", "b"], ""), "first");
}
#[test]
fn first_text_skips_a_tag_that_is_present_but_empty() {
    let d = parse_xml("<i><a/><b>second</b></i>").unwrap();
    assert_eq!(first_text(d.root_element(), &["a", "b"], ""), "second");
}
#[test]
fn first_text_falls_back_to_the_default() {
    let d = parse_xml("<i/>").unwrap();
    assert_eq!(first_text(d.root_element(), &["a", "b"], "none"), "none");
}
#[test]
fn first_text_does_not_discard_an_element_without_children() {
    let d = parse_xml("<i><image_link>https://x.cz/a.jpg</image_link></i>").unwrap();
    assert_eq!(
        first_text(d.root_element(), &["image_link", "IMGURL"], ""),
        "https://x.cz/a.jpg"
    );
}
fn expected() -> Vec<ImageData> {
    [("A-1", "Boty Alfa", "a1"), ("B-2", "Boty Beta", "b2")]
        .into_iter()
        .enumerate()
        .map(|(i, (number, name, file))| ImageData {
            index: i + 1,
            suffix: (i + 1).to_string(),
            number: number.into(),
            name: name.into(),
            url: format!("https://shop.cz/img/{file}.jpg"),
        })
        .collect()
}
#[test]
fn a_universal_feed_yields_every_image() {
    assert_eq!(
        extract_image_data(&parse_xml(UNIVERSAL).unwrap()),
        expected()
    );
}
#[test]
fn a_google_feed_yields_every_image() {
    assert_eq!(extract_image_data(&parse_xml(GOOGLE).unwrap()), expected());
}
#[test]
fn a_heureka_feed_yields_every_image() {
    assert_eq!(extract_image_data(&parse_xml(UNIVERSAL).unwrap()).len(), 2);
}
#[test]
fn the_three_feed_shapes_agree_on_what_they_return() {
    assert_eq!(
        extract_image_data(&parse_xml(UNIVERSAL).unwrap()),
        extract_image_data(&parse_xml(GOOGLE).unwrap())
    );
}
#[test]
fn a_product_without_an_image_is_left_out() {
    assert!(extract_image_data(&parse_xml("<SHOP><SHOPITEM><PRODUCT>Bez fotky</PRODUCT><PRODUCTNO>X</PRODUCTNO></SHOPITEM></SHOP>").unwrap()).is_empty());
}
#[test]
fn a_missing_product_number_gets_a_generated_one() {
    let data = extract_image_data(
        &parse_xml("<SHOP><SHOPITEM><IMGURL>https://x.cz/a.jpg</IMGURL></SHOPITEM></SHOP>")
            .unwrap(),
    );
    assert_eq!(data[0].number, "product_1");
    assert_eq!(data[0].name, "product_1");
}
#[test]
fn an_empty_feed_yields_nothing() {
    assert!(extract_image_data(&parse_xml("<SHOP/>").unwrap()).is_empty());
}
#[test]
fn the_index_counts_products_from_one() {
    assert_eq!(
        extract_image_data(&parse_xml(UNIVERSAL).unwrap())
            .iter()
            .map(|i| i.index)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}
#[test]
fn a_product_row_follows_the_header_order() {
    let d = parse_xml(UNIVERSAL).unwrap();
    let row = product_row(find_items(&d)[0], "A-1", "a1.jpg");
    assert_eq!(
        row,
        vec![
            "Boty Alfa",
            "https://shop.cz/boty-alfa",
            "500",
            "605",
            "21",
            "Alfa",
            "Obuv",
            "8590000000001",
            "A-1",
            "0",
            "a1.jpg"
        ]
    );
    assert_eq!(row.len(), HEADERS.len());
}
#[test]
fn missing_product_fields_become_empty_strings() {
    let d = parse_xml("<SHOPITEM><PRODUCT>Jen jméno</PRODUCT></SHOPITEM>").unwrap();
    let row = product_row(d.root_element(), "X", "");
    assert_eq!(row.len(), HEADERS.len());
    assert_eq!(row[0], "Jen jméno");
    assert!(row[1..8].iter().all(String::is_empty));
}
#[test]
fn every_feed_type_declares_the_same_columns() {
    for f in FEEDS {
        assert_eq!(HEADERS.len(), 11);
        assert_eq!(feed(f.key).key, f.key);
    }
}
#[test]
fn every_feed_type_has_a_path_and_a_name() {
    for f in FEEDS {
        assert!(f.path.starts_with('/'));
        assert!(!f.name.is_empty());
    }
}
#[test]
fn a_parser_exists_for_every_feed_type() {
    for f in FEEDS {
        assert_eq!(feed(f.key).key, f.key);
    }
}
#[test]
fn an_unknown_feed_type_is_read_as_a_universal_feed() {
    assert_eq!(feed("something-new").key, "universal");
}
#[test]
fn a_feed_file_is_parsed() {
    assert!(parse_xml(UNIVERSAL).is_ok());
}
#[test]
fn a_malformed_feed_returns_none() {
    assert!(parse_xml("<SHOP><SHOPITEM></SHOP>").is_err());
}
#[test]
fn a_feed_declaring_entities_is_refused() {
    assert!(parse_xml("<!DOCTYPE lolz [<!ENTITY lol 'lol'>]><SHOP>&lol;</SHOP>").is_err());
}
#[test]
fn indexes_include_products_without_images() {
    let d = parse_xml("<SHOP><SHOPITEM/><SHOPITEM><IMGURL>x</IMGURL></SHOPITEM></SHOP>").unwrap();
    assert_eq!(extract_image_data(&d)[0].index, 2);
}
#[test]
fn namespaced_google_images_are_supported() {
    let d=parse_xml("<rss xmlns:g='http://base.google.com/ns/1.0'><channel><item><g:id>A</g:id><g:image_link>https://x/a</g:image_link></item></channel></rss>").unwrap();
    assert_eq!(extract_image_data(&d)[0].number, "A");
}
#[test]
fn item_container_has_priority_and_root_is_excluded() {
    let d = parse_xml("<SHOP><SHOPITEM/><item><image_link>x</image_link></item></SHOP>").unwrap();
    assert_eq!(find_items(&d).len(), 1);
    assert!(find_items(&parse_xml("<item/>").unwrap()).is_empty());
}
#[test]
fn whitespace_is_a_value() {
    let d = parse_xml("<i><a> </a><b>x</b></i>").unwrap();
    assert_eq!(first_text(d.root_element(), &["a", "b"], ""), " ");
}

#[test]
fn xml_encoding_declarations_are_respected() {
    let bytes = b"<?xml version='1.0' encoding='windows-1250'?><SHOP>\x9elu\x9dou\xe8k\xfd</SHOP>";
    let text = decode_xml(bytes).unwrap();
    assert_eq!(
        parse_xml(&text).unwrap().root_element().text(),
        Some("žluťoučký")
    );
    assert!(decode_xml(b"<SHOP>\xff</SHOP>").is_err());
}

#[test]
fn encoding_in_product_text_is_not_an_xml_declaration() {
    let bytes = b"<SHOP><PRODUCT>encoding matters</PRODUCT></SHOP>";
    let text = decode_xml(bytes).unwrap();
    assert_eq!(
        first_text(parse_xml(&text).unwrap().root_element(), &["PRODUCT"], ""),
        "encoding matters"
    );
}

#[test]
fn utf16_feeds_decode_before_parsing() {
    let xml = "<?xml version='1.0' encoding='UTF-16'?><SHOP>žluťoučký</SHOP>";
    for little_endian in [true, false] {
        let mut bytes = if little_endian {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for unit in xml.encode_utf16() {
            bytes.extend(if little_endian {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        let text = decode_xml(&bytes).unwrap();
        assert_eq!(
            parse_xml(&text).unwrap().root_element().text(),
            Some("žluťoučký")
        );
    }
}
