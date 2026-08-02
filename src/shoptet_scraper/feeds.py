#
# Project: shoptet-scraper
# File:    feeds.py
#
# Description:
# Reads the XML product feeds a Shoptet shop serves, in all three of the shapes they come in.
#
# Author:
# Jan Alexandr Kopřiva
# jan.alexandr.kopriva@gmail.com
#
# License: MIT
#

"""The XML product feeds a Shoptet shop serves.

Every shop exposes the same product data at a handful of fixed paths. The feeds
differ in the tag names they use, not in the shape of the data, so one parser
reads all of them by trying each known spelling of a field in turn.
"""

import xml.etree.ElementTree as ET

from .security import safe_xml_parse

# Columns written to the spreadsheet. Every feed carries the same fields.
HEADERS = [
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
]

XML_FEED_TYPES = {
    "universal": {
        "name": "Universal XML Feed",
        "description": "Univerzální XML feed pro všechny produkty",
        "path": "/universal.xml",
        "headers": HEADERS,
    },
    "google": {
        "name": "Google Shopping XML Feed",
        "description": "XML feed pro Google Shopping",
        "path": "/google.xml",
        "headers": HEADERS,
    },
    "heureka": {
        "name": "Heureka XML Feed",
        "description": "XML feed pro srovnávač cen Heureka",
        "path": "/heureka.xml",
        "headers": HEADERS,
    },
    "zbozi": {
        "name": "Zboží.cz XML Feed",
        "description": "XML feed pro srovnávač cen Zboží.cz",
        "path": "/zbozi.xml",
        "headers": HEADERS,
    },
    "shoptet_supplier": {
        "name": "Shoptet Dodavatelský XML Feed",
        "description": "XML feed pro dodavatele Shoptet",
        "path": "/supplier.xml",
        "headers": HEADERS,
    },
}

# Where the products sit. Google Shopping uses <item>, the rest use <SHOPITEM>.
ITEM_PATHS = (".//item", ".//SHOPITEM")

# Field spellings, Google first. A feed carries one spelling or the other.
IMAGE_TAGS = ("image_link", "IMGURL")
NUMBER_TAGS = ("id", "PRODUCTNO")
NAME_TAGS = ("title", "PRODUCT")

# Product fields written to the spreadsheet, in header order.
PRODUCT_TAGS = [
    "PRODUCT",
    "URL",
    "PRICE",
    "PRICE_VAT",
    "VAT",
    "MANUFACTURER",
    "CATEGORYTEXT",
    "EAN",
]


def first_text(element: ET.Element, tags, default: str = "") -> str:
    """Text of the first tag that is present and carries a value.

    Written as an explicit loop on purpose. `element.find(a) or element.find(b)`
    reads the same but is wrong: an Element with text and no children is falsy,
    so the first branch is discarded exactly when it holds what was wanted.
    """
    for tag in tags:
        found = element.find(tag)
        if found is not None and found.text:
            return found.text
    return default


class FeedParser:
    """Reads any of the Shoptet feed types."""

    def __init__(self, feed_type: str, logger):
        self.feed_type = feed_type
        self.feed_config = XML_FEED_TYPES[feed_type]
        self.logger = logger

    def parse_xml(self, xml_path):
        """Root element of the feed, or None when it cannot be read."""
        try:
            root = safe_xml_parse(xml_path).getroot()
        except Exception:
            self.logger.exception(" Chyba při parsování %s", self.feed_config["name"])
            return None
        else:
            self.logger.info(" %s úspěšně parsováno: %s", self.feed_config["name"], xml_path)
            return root

    def get_headers(self) -> list[str]:
        return self.feed_config["headers"]

    def find_items(self, root: ET.Element) -> list[ET.Element]:
        """Product elements, whichever container this feed uses."""
        for path in ITEM_PATHS:
            items = root.findall(path)
            if items:
                return items
        return []

    def extract_image_data(self, root: ET.Element) -> list[tuple[int, str, str, str]]:
        """Index, image URL, product number and product name, per product.

        Products without an image URL are left out, because the caller pairs
        this list with the downloaded files.
        """
        image_data = []
        for index, item in enumerate(self.find_items(root), start=1):
            img_url = first_text(item, IMAGE_TAGS)
            if not img_url:
                continue
            image_data.append((
                index,
                img_url,
                first_text(item, NUMBER_TAGS, f"product_{index}"),
                first_text(item, NAME_TAGS, f"product_{index}"),
            ))
        self.logger.info(
            " Nalezeno %d obrázků v %s", len(image_data), self.feed_config["name"]
        )
        return image_data

    def extract_product_data(
        self, shopitem: ET.Element, product_no: str, image_filename: str
    ) -> list[str]:
        """One spreadsheet row for a product, in header order."""
        row = [shopitem.findtext(tag, default="") for tag in PRODUCT_TAGS]
        row.append(product_no)
        row.append(shopitem.findtext("DELIVERY_DATE", default=""))
        row.append(image_filename or "")
        return row


def get_xml_parser(feed_type: str, logger) -> FeedParser:
    """Parser for a feed type. Anything unknown is read as a universal feed."""
    return FeedParser(feed_type if feed_type in XML_FEED_TYPES else "universal", logger)
