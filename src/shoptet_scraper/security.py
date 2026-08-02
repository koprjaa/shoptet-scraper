#
# Project: shoptet-scraper
# File:    security.py
#
# Description:
# Checks that guard the scraper against the shop data it reads: server side request forgery, entity expansion, spreadsheet formulas, and unsafe file names.
#
# Author:
# Jan Alexandr Kopřiva
# jan.alexandr.kopriva@gmail.com
#
# License: MIT
#

"""Checks that protect the scraper from the data it reads.

The scraper follows URLs and opens images that a third-party shop controls, so
everything in here guards a trust boundary.
"""

import ipaddress
import re
import socket
import xml.etree.ElementTree as ET
from pathlib import Path
from urllib.parse import urlparse

from PIL import Image as PILImage

try:
    import defusedxml.ElementTree as DefusedET

    HAVE_DEFUSEDXML = True
except ImportError:  # pragma: no cover - exercised by the fallback test
    DefusedET = None
    HAVE_DEFUSEDXML = False

# Cap on the pixel count of a decoded image, against a decompression bomb.
# The Pillow default is about 89M, which is more than any product photo needs.
PILImage.MAX_IMAGE_PIXELS = 50_000_000

# Cap on the bytes read for one image, against a memory blow-up.
MAX_IMAGE_DOWNLOAD_BYTES = 30 * 1024 * 1024

# Characters a spreadsheet reads as the start of a formula.
FORMULA_PREFIXES = ("=", "+", "-", "@", "\t", "\r")

# Characters that are not allowed in a file name on Windows.
UNSAFE_FILENAME_CHARS = re.compile(r'[\\/*?:"<>|]')

# Carrier-grade NAT, RFC 6598. ipaddress.is_private does not cover it.
CGNAT_NETWORK = ipaddress.ip_network("100.64.0.0/10")


def is_blocked_ip(ip_str: str) -> bool:
    """True when an address must not be read from.

    Anything that is not globally routable is refused. An address that will not
    parse is refused too, rather than assumed safe.
    """
    try:
        ip = ipaddress.ip_address(ip_str)
    except ValueError:
        return True
    # Carrier-grade NAT, RFC 6598. is_private does not cover it.
    if ip.version == 4 and ip in CGNAT_NETWORK:
        return True
    return (
        ip.is_private
        or ip.is_loopback
        or ip.is_link_local
        or ip.is_reserved
        or ip.is_multicast
        or ip.is_unspecified
    )


def is_safe_url(url: str) -> bool:
    """True when the URL looks safe to request.

    Requires http or https, resolves the hostname, and refuses private,
    loopback, link-local and other reserved addresses. This blocks a shop from
    pointing the scraper at something on the machine or the local network.

    This is the cheap check that runs first. It cannot stand alone: the name is
    resolved here and again by the HTTP library, so a hostname that answers
    differently the second time still gets connected to. `peer_address` closes
    that window by looking at where the socket actually went.
    """
    try:
        parsed = urlparse(url)
    except ValueError:
        return False

    if parsed.scheme not in ("http", "https"):
        return False
    if not parsed.hostname:
        return False

    try:
        addr_infos = socket.getaddrinfo(parsed.hostname, None)
    except (OSError, UnicodeError):
        return False

    if not addr_infos:
        return False

    return not any(is_blocked_ip(info[4][0]) for info in addr_infos)


def peer_address(response) -> str | None:
    """Address the response is actually connected to, or None when unavailable.

    Read off the live socket, so it reflects where the connection went rather
    than what a second name lookup would say now.
    """
    conn = getattr(getattr(response, "raw", None), "_connection", None)
    sock = getattr(conn, "sock", None)
    if sock is None:
        return None
    try:
        return sock.getpeername()[0]
    except OSError:
        return None


def sanitize_xlsx_value(value):
    """Stop a spreadsheet from running a product name as a formula."""
    if isinstance(value, str) and value and value[0] in FORMULA_PREFIXES:
        return "'" + value
    return value


def sanitize_filename(filename: str) -> str:
    """Replace the characters a file name may not contain."""
    return UNSAFE_FILENAME_CHARS.sub("_", filename)


def safe_xml_parse(xml_path):
    """Parse an XML feed without letting it expand entities.

    defusedxml does the work when it is installed. Without it, a file carrying a
    DOCTYPE or an ENTITY is refused outright rather than handed to ElementTree,
    which would expand it.
    """
    if HAVE_DEFUSEDXML:
        return DefusedET.parse(xml_path)

    head = Path(xml_path).read_bytes()[:65536].lower()
    if b"<!doctype" in head or b"<!entity" in head:
        raise ValueError("XML declares a DOCTYPE or an ENTITY, refused")
    return ET.parse(xml_path)


def download_image_bytes(session, url: str, timeout: int = 10) -> bytes | None:
    """Read an image, or None when the URL is not safe to request.

    Two address checks, not one. The first refuses a hostname that already
    resolves somewhere private. The second looks at the socket after it is
    open, because the HTTP library resolves the name a second time and a
    hostname under someone else's control can answer differently that time.
    Nothing is read from the body until the peer has been checked.

    The declared length is checked first, then the bytes are counted while they
    arrive. A server that lies in its header, or sends none, still cannot push
    the scraper past the limit.
    """
    if not is_safe_url(url):
        return None

    response = session.get(url, timeout=timeout, stream=True)

    peer = peer_address(response)
    if peer is not None and is_blocked_ip(peer):
        response.close()
        return None

    response.raise_for_status()

    declared = response.headers.get("content-length")
    if declared and declared.isdigit() and int(declared) > MAX_IMAGE_DOWNLOAD_BYTES:
        raise ValueError(f"image declares {declared} bytes, over the limit")

    chunks, total = [], 0
    for chunk in response.iter_content(chunk_size=65536):
        if not chunk:
            continue
        total += len(chunk)
        if total > MAX_IMAGE_DOWNLOAD_BYTES:
            raise ValueError("image went over the size limit while downloading")
        chunks.append(chunk)
    return b"".join(chunks)
