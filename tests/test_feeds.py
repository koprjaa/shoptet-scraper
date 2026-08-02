#
# Project: shoptet-scraper
# File:    test_feeds.py
#
# Description:
# Tests for reading the three Shoptet product feed shapes.
#
# Author:
# Jan Alexandr Kopřiva
# jan.alexandr.kopriva@gmail.com
#
# License: MIT
#

"""Tests for reading the Shoptet product feeds.

The three feed shapes below are what the real endpoints serve. Google Shopping
names its fields in lowercase, the Czech comparison sites keep the Shoptet
uppercase names.
"""

import logging
import warnings
import xml.etree.ElementTree as ET

import pytest

from shoptet_scraper.feeds import (
    HEADERS,
    XML_FEED_TYPES,
    FeedParser,
    first_text,
    get_xml_parser,
)

UNIVERSAL = """<?xml version="1.0" encoding="utf-8"?>
<SHOP>
  <SHOPITEM>
    <PRODUCT>Boty Alfa</PRODUCT><URL>https://shop.cz/boty-alfa</URL>
    <PRICE>500</PRICE><PRICE_VAT>605</PRICE_VAT><VAT>21</VAT>
    <MANUFACTURER>Alfa</MANUFACTURER><CATEGORYTEXT>Obuv</CATEGORYTEXT>
    <EAN>8590000000001</EAN><PRODUCTNO>A-1</PRODUCTNO>
    <DELIVERY_DATE>0</DELIVERY_DATE>
    <IMGURL>https://shop.cz/img/a1.jpg</IMGURL>
  </SHOPITEM>
  <SHOPITEM>
    <PRODUCT>Boty Beta</PRODUCT><PRODUCTNO>B-2</PRODUCTNO>
    <IMGURL>https://shop.cz/img/b2.jpg</IMGURL>
  </SHOPITEM>
</SHOP>"""

GOOGLE = """<?xml version="1.0" encoding="utf-8"?>
<rss><channel>
  <item>
    <id>A-1</id><title>Boty Alfa</title>
    <image_link>https://shop.cz/img/a1.jpg</image_link>
  </item>
  <item>
    <id>B-2</id><title>Boty Beta</title>
    <image_link>https://shop.cz/img/b2.jpg</image_link>
  </item>
</channel></rss>"""

HEUREKA = UNIVERSAL


@pytest.fixture
def logger():
    return logging.getLogger("test")


def parser(feed_type, logger):
    return FeedParser(feed_type, logger)


# --- first_text -------------------------------------------------------------


def test_first_text_returns_the_first_tag_that_has_a_value():
    item = ET.fromstring("<i><b>second</b></i>")
    assert first_text(item, ("a", "b")) == "second"


def test_first_text_prefers_the_earlier_tag():
    item = ET.fromstring("<i><a>first</a><b>second</b></i>")
    assert first_text(item, ("a", "b")) == "first"


def test_first_text_skips_a_tag_that_is_present_but_empty():
    item = ET.fromstring("<i><a></a><b>second</b></i>")
    assert first_text(item, ("a", "b")) == "second"


def test_first_text_falls_back_to_the_default():
    assert first_text(ET.fromstring("<i/>"), ("a", "b"), "none") == "none"


def test_first_text_does_not_discard_an_element_without_children():
    """An Element carrying text but no children is falsy.

    Written as `find(a) or find(b)` this returns b, which is how the Google
    feed used to lose every image it had.
    """
    item = ET.fromstring("<i><image_link>https://x.cz/a.jpg</image_link></i>")
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", DeprecationWarning)
        assert bool(item.find("image_link")) is False
    assert first_text(item, ("image_link", "IMGURL")) == "https://x.cz/a.jpg"


# --- image extraction -------------------------------------------------------


def test_a_universal_feed_yields_every_image(logger):
    root = ET.fromstring(UNIVERSAL)
    assert parser("universal", logger).extract_image_data(root) == [
        (1, "https://shop.cz/img/a1.jpg", "A-1", "Boty Alfa"),
        (2, "https://shop.cz/img/b2.jpg", "B-2", "Boty Beta"),
    ]


def test_a_google_feed_yields_every_image(logger):
    """This is the case that returned nothing before the tag lookup was fixed."""
    root = ET.fromstring(GOOGLE)
    assert parser("google", logger).extract_image_data(root) == [
        (1, "https://shop.cz/img/a1.jpg", "A-1", "Boty Alfa"),
        (2, "https://shop.cz/img/b2.jpg", "B-2", "Boty Beta"),
    ]


def test_a_heureka_feed_yields_every_image(logger):
    root = ET.fromstring(HEUREKA)
    assert len(parser("heureka", logger).extract_image_data(root)) == 2


def test_the_three_feed_shapes_agree_on_what_they_return(logger):
    """The same catalogue exported three ways must read back the same."""
    universal = parser("universal", logger).extract_image_data(ET.fromstring(UNIVERSAL))
    google = parser("google", logger).extract_image_data(ET.fromstring(GOOGLE))
    assert universal == google


def test_a_product_without_an_image_is_left_out(logger):
    root = ET.fromstring(
        "<SHOP><SHOPITEM><PRODUCT>Bez fotky</PRODUCT><PRODUCTNO>X</PRODUCTNO></SHOPITEM></SHOP>"
    )
    assert parser("universal", logger).extract_image_data(root) == []


def test_a_missing_product_number_gets_a_generated_one(logger):
    root = ET.fromstring("<SHOP><SHOPITEM><IMGURL>https://x.cz/a.jpg</IMGURL></SHOPITEM></SHOP>")
    _index, _url, number, name = parser("universal", logger).extract_image_data(root)[0]
    assert number == "product_1"
    assert name == "product_1"


def test_an_empty_feed_yields_nothing(logger):
    assert parser("universal", logger).extract_image_data(ET.fromstring("<SHOP/>")) == []


def test_the_index_counts_products_from_one(logger):
    root = ET.fromstring(UNIVERSAL)
    indexes = [row[0] for row in parser("universal", logger).extract_image_data(root)]
    assert indexes == [1, 2]


# --- rows -------------------------------------------------------------------


def test_a_product_row_follows_the_header_order(logger):
    item = ET.fromstring(UNIVERSAL).find(".//SHOPITEM")
    row = parser("universal", logger).extract_product_data(item, "A-1", "a1.jpg")
    assert len(row) == len(HEADERS)
    assert row == [
        "Boty Alfa", "https://shop.cz/boty-alfa", "500", "605", "21",
        "Alfa", "Obuv", "8590000000001", "A-1", "0", "a1.jpg",
    ]


def test_missing_product_fields_become_empty_strings(logger):
    item = ET.fromstring("<SHOPITEM><PRODUCT>Jen jméno</PRODUCT></SHOPITEM>")
    row = parser("universal", logger).extract_product_data(item, "X", "")
    assert len(row) == len(HEADERS)
    assert row[0] == "Jen jméno"
    assert row[1:8] == [""] * 7


# --- feed types -------------------------------------------------------------


def test_every_feed_type_declares_the_same_columns():
    assert all(config["headers"] == HEADERS for config in XML_FEED_TYPES.values())


def test_every_feed_type_has_a_path_and_a_name():
    for name, config in XML_FEED_TYPES.items():
        assert config["path"].startswith("/"), name
        assert config["name"], name


@pytest.mark.parametrize("feed_type", sorted(XML_FEED_TYPES))
def test_a_parser_exists_for_every_feed_type(feed_type, logger):
    assert get_xml_parser(feed_type, logger).feed_type == feed_type


def test_an_unknown_feed_type_is_read_as_a_universal_feed(logger):
    assert get_xml_parser("something-new", logger).feed_type == "universal"


# --- parse_xml --------------------------------------------------------------


def test_a_feed_file_is_parsed(tmp_path, logger):
    path = tmp_path / "feed.xml"
    path.write_text(UNIVERSAL, encoding="utf-8")
    assert parser("universal", logger).parse_xml(path) is not None


def test_a_malformed_feed_returns_none(tmp_path, logger):
    path = tmp_path / "bad.xml"
    path.write_text("<SHOP><SHOPITEM></SHOP>", encoding="utf-8")
    assert parser("universal", logger).parse_xml(path) is None


def test_a_feed_declaring_entities_is_refused(tmp_path, logger):
    path = tmp_path / "bomb.xml"
    path.write_text(
        '<?xml version="1.0"?><!DOCTYPE lolz [<!ENTITY lol "lol">]><SHOP>&lol;</SHOP>',
        encoding="utf-8",
    )
    assert parser("universal", logger).parse_xml(path) is None
