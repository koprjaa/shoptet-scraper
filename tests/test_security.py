"""Tests for the checks that guard the scraper against the shops it reads."""

import pytest
from defusedxml.common import DefusedXmlException

from shoptet_scraper.security import (
    FORMULA_PREFIXES,
    MAX_IMAGE_DOWNLOAD_BYTES,
    is_safe_url,
    safe_xml_parse,
    sanitize_filename,
    sanitize_xlsx_value,
)


class FakeResponse:
    def __init__(self, chunks, headers=None):
        self._chunks = chunks
        self.headers = headers or {}

    def raise_for_status(self):
        pass

    def iter_content(self, chunk_size=0):
        return iter(self._chunks)


class FakeSession:
    def __init__(self, response):
        self._response = response

    def get(self, url, timeout=None, stream=False):
        return self._response


# --- is_safe_url ------------------------------------------------------------


@pytest.mark.parametrize(
    "url",
    [
        "http://127.0.0.1/a.jpg",
        "http://localhost/a.jpg",
        "http://[::1]/a.jpg",
        "http://169.254.169.254/latest/meta-data/",
        "http://0.0.0.0/a.jpg",
    ],
)
def test_an_address_on_this_machine_or_network_is_refused(url):
    assert is_safe_url(url) is False


@pytest.mark.parametrize(
    "url",
    ["file:///etc/passwd", "ftp://shop.cz/a.jpg", "gopher://shop.cz/", "javascript:alert(1)"],
)
def test_only_http_and_https_are_allowed(url):
    assert is_safe_url(url) is False


@pytest.mark.parametrize("url", ["", "http://", "not a url", "https:///a.jpg"])
def test_a_url_without_a_host_is_refused(url):
    assert is_safe_url(url) is False


def test_a_hostname_that_does_not_resolve_is_refused():
    assert is_safe_url("https://this-name-does-not-exist.invalid/a.jpg") is False


def test_a_public_address_is_allowed():
    assert is_safe_url("https://example.com/a.jpg") is True


# --- sanitize_xlsx_value ----------------------------------------------------


@pytest.mark.parametrize("prefix", FORMULA_PREFIXES)
def test_a_value_a_spreadsheet_would_run_is_quoted(prefix):
    assert sanitize_xlsx_value(f"{prefix}cmd|calc") == f"'{prefix}cmd|calc"


def test_a_plain_product_name_is_left_alone():
    assert sanitize_xlsx_value("Boty Alfa") == "Boty Alfa"


@pytest.mark.parametrize("value", ["", None, 42, 3.5])
def test_a_value_that_is_not_text_passes_through(value):
    assert sanitize_xlsx_value(value) == value


def test_a_negative_number_written_as_text_is_quoted():
    """A leading minus starts a formula, so this is the price of being safe."""
    assert sanitize_xlsx_value("-500") == "'-500"


# --- sanitize_filename ------------------------------------------------------


@pytest.mark.parametrize("char", list('\\/*?:"<>|'))
def test_a_character_windows_forbids_is_replaced(char):
    assert sanitize_filename(f"boty{char}alfa") == "boty_alfa"


def test_a_path_cannot_be_smuggled_through_a_product_name():
    assert "/" not in sanitize_filename("../../etc/passwd")
    assert "\\" not in sanitize_filename("..\\..\\windows\\system32")


def test_czech_characters_survive():
    assert sanitize_filename("Příliš žluťoučký kůň") == "Příliš žluťoučký kůň"


# --- safe_xml_parse ---------------------------------------------------------


def test_a_plain_feed_parses(tmp_path):
    path = tmp_path / "feed.xml"
    path.write_text("<SHOP><SHOPITEM/></SHOP>", encoding="utf-8")
    assert safe_xml_parse(path).getroot().tag == "SHOP"


def test_an_entity_expansion_payload_is_refused(tmp_path):
    path = tmp_path / "bomb.xml"
    path.write_text(
        '<?xml version="1.0"?>\n'
        '<!DOCTYPE lolz [<!ENTITY lol "lol"><!ENTITY lol2 "&lol;&lol;&lol;">]>\n'
        "<SHOP>&lol2;</SHOP>",
        encoding="utf-8",
    )
    with pytest.raises((ValueError, DefusedXmlException)):
        safe_xml_parse(path)


def test_an_external_entity_is_refused(tmp_path):
    path = tmp_path / "xxe.xml"
    path.write_text(
        '<?xml version="1.0"?>\n'
        '<!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>\n'
        "<SHOP>&xxe;</SHOP>",
        encoding="utf-8",
    )
    with pytest.raises((ValueError, DefusedXmlException)):
        safe_xml_parse(path)


# --- download_image_bytes ---------------------------------------------------


def test_an_unsafe_url_is_not_fetched():
    from shoptet_scraper.security import download_image_bytes

    session = FakeSession(FakeResponse([b"never read"]))
    assert download_image_bytes(session, "http://127.0.0.1/a.jpg") is None


def test_a_declared_size_over_the_limit_is_refused(monkeypatch):
    import shoptet_scraper.security as security

    monkeypatch.setattr(security, "is_safe_url", lambda url: True)
    session = FakeSession(
        FakeResponse([b"x"], {"content-length": str(MAX_IMAGE_DOWNLOAD_BYTES + 1)})
    )
    with pytest.raises(ValueError, match="over the limit"):
        security.download_image_bytes(session, "https://shop.cz/a.jpg")


def test_a_server_that_lies_about_the_size_is_still_stopped(monkeypatch):
    """No content-length, or a false one, must not let the download run away."""
    import shoptet_scraper.security as security

    monkeypatch.setattr(security, "is_safe_url", lambda url: True)
    oversized = [b"x" * 65536] * ((MAX_IMAGE_DOWNLOAD_BYTES // 65536) + 2)
    session = FakeSession(FakeResponse(oversized, {"content-length": "10"}))
    with pytest.raises(ValueError, match="while downloading"):
        security.download_image_bytes(session, "https://shop.cz/a.jpg")


def test_a_normal_image_comes_back_whole(monkeypatch):
    import shoptet_scraper.security as security

    monkeypatch.setattr(security, "is_safe_url", lambda url: True)
    session = FakeSession(FakeResponse([b"abc", b"def"], {"content-length": "6"}))
    assert security.download_image_bytes(session, "https://shop.cz/a.jpg") == b"abcdef"


def test_a_nonsense_content_length_header_does_not_crash(monkeypatch):
    import shoptet_scraper.security as security

    monkeypatch.setattr(security, "is_safe_url", lambda url: True)
    session = FakeSession(FakeResponse([b"abc"], {"content-length": "not a number"}))
    assert security.download_image_bytes(session, "https://shop.cz/a.jpg") == b"abc"
