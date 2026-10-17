# shoptet-scraper

Find Shoptet product feeds, export Excel workbooks, and download product images.

![rust](https://img.shields.io/badge/rust-stable-DEA584?style=flat-square&logo=rust&logoColor=white)
![license](https://img.shields.io/badge/license-MIT-A31F34?style=flat-square)
[![ci](https://github.com/koprjaa/shoptet-scraper/actions/workflows/ci.yml/badge.svg)](https://github.com/koprjaa/shoptet-scraper/actions/workflows/ci.yml)


## Install

Install the stable Rust toolchain, then run:

```sh
cargo install --path .
```

## Use

Interactive mode accepts one URL per line, followed by a blank line. Choose a
shop and an available feed from the menu.

```sh
shoptet-scraper eshop
shoptet-scraper eshop --store https://example.shoptet.cz
shoptet-scraper eshop --store https://example.shoptet.cz --all-images
```

| Argument | Default | Effect |
| --- | --- | --- |
| `eshop` | required | Discover and scrape shops |
| `--store URL` | none | Discover this shop and scrape its universal feed |
| `--output PATH` | `./output` | Parent directory for numbered sessions |
| `-v`, `--verbose` | off | Enable DEBUG logging |
| `-a`, `--all-images` | off | Read product pages for all gallery images |
| `-h`, `--help` | | Display help |

Sessions use local dates and the first unused `YYYY-MM-DD_NNN` directory.
Each shop has a sanitized title-cased subdirectory with workbooks and `images/`:

```text
output/2026-10-17_001/Example/
  feed_example_universal_17-10-2026_part_1.xlsx
  images/
```

The five feed paths are `/universal.xml`, `/google.xml`, `/heureka.xml`,
`/zbozi.xml`, and `/supplier.xml`. Each workbook contains at most 1,000 data
rows on the `Sheet` worksheet, with the following string columns in order:

```text
PRODUCT URL PRICE PRICE_VAT VAT MANUFACTURER CATEGORYTEXT EAN PRODUCTNO DELIVERY_DATE IMAGE_FILENAME
```

Products without an image are omitted. Failed downloads still produce a row
with an empty image filename. Rows follow image download completion order.
Gallery mode produces one row per image. Images are JPEG quality 85 on white.
The XML is retained after parsing failure or when no images are found; after
successful workbook processing it is deleted. No CSV or JSON outputs exist.

Logs append to `shoptet_scraper.log` and stderr using the original timestamp and
level format. Argument errors exit 2; unexpected runtime failures exit 1;
ordinary unavailable shops/feeds, no input, quitting, and Ctrl-C exit 0.

## Network and safety

Discovery and galleries use the original Chrome 91 macOS User-Agent and three
retries (500/502/504/408/429; backoff 0, 2, 4 seconds). Feed and image requests
use `python-requests/2.33.0` and five retries (500/502/504; backoff 0, 1, 2, 4, 8
seconds). Retry-After is honored, including standard retryable 413/429/503
responses. Discovery/gallery requests use 15 seconds, home page and HEAD/image
requests 10 seconds, feed GET requests 30 seconds. Fifty workers download
images; there is no rate or bandwidth limit. System proxy environment settings
(`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`, and supported lowercase
forms) are inherited by reqwest. There are no application config files or keys.

Every request and redirect checks DNS and the connected peer before consuming
the body. Private, loopback, link-local, reserved, multicast, unspecified and
carrier-grade NAT addresses are rejected. XML declarations cannot expand
entities. Images are capped at 30 MiB and 50 million pixels. Spreadsheet formula
prefixes are quoted; Windows-forbidden filename characters are replaced by `_`.

Feeds are parsed in memory. There is no resume, bandwidth limiting, or support
for nonstandard feed endpoint locations. Very long product names or names
containing filesystem-specific control characters may fail to save.

## Changes from the Python version

- The executable is `shoptet-scraper`; Python module imports and unused legacy
  helper functions are replaced by typed Rust library APIs. There is no Python
  runtime. Dynamic non-string spreadsheet values cannot enter the string row API.
- All CLI help, interactive prompts, progress descriptions and previously Czech
  messages are English. Clap formats usage and argument errors. The same flags,
  normal success/failure exit codes and direct universal-feed choice are retained.
  Unambiguous long-option abbreviations and repeated options remain accepted.
- `--verbose` now enables DEBUG logs; it was previously ignored.
- Default output is `./output` relative to the working directory, allowing an
  installed executable to run anywhere. Python used the directory of main.py.
- Session allocation is atomic, preventing concurrent runs sharing a folder.
  Feed filename prefixes also sanitize forbidden characters, fixing URLs with
  explicit ports producing invalid Windows filenames.
- Gallery indexes keep the product index separate from the filename suffix,
  fixing the string-subtraction crash in `--all-images`. Gallery duplicates are
  removed after size normalization. Gallery mode also supports Google item
  containers and their lowercase URL/identifier/name fields.
- Namespaced XML field names are recognized, fixing standard Google feeds losing
  their images. Spreadsheet fields still use the original uppercase field
  mapping; lowercase Google titles/prices do not populate uppercase columns.
  Extraction and row lookup share the same item selection, preferring `item`
  over `SHOPITEM` even in mixed non-Google feeds, avoiding mismatched rows.
- Other feed types on custom domains pass validation; the incorrect
  `shoptet`/`universal.xml` substring restriction is removed.
- DNS and peer checks also protect feeds, discovery, product pages, and redirect
  hops. Unsafe redirect destinations are refused. Responses close on every error.
  Public-address classification uses explicit conservative IP ranges rather
  than Python ipaddress; special-purpose ranges and mapped IPv6 addresses can
  therefore be classified differently across Python releases.
  Missing peer information remains allowed, as in Python; proxies can obscure
  the final destination's peer. TLS uses rustls roots instead of Requests' CA
  bundle; Requests-specific CA environment overrides are not implemented.
- XML DOCTYPE/ENTITY declarations are always refused throughout the document,
  independent of optional dependencies. Rust XML parsing is stricter about some
  invalid XML. UTF-8, BOM-marked UTF-16 and XML-declared encodings supported by
  encoding_rs are decoded explicitly; unsupported encodings fail safely.
  Encoding detection reads the XML declaration only, so product text containing
  the word `encoding` is parsed normally.
  The conservative declaration scan also rejects declaration-like text in XML
  comments or CDATA. Adjacent XML text/CDATA nodes may be exposed differently
  by the parser.
- Images exceeding 50 million pixels are rejected before decoding, rather than
  Pillow warning at 50 million and failing at 100 million. Codec support is
  JPEG/PNG/GIF/WebP/BMP/TIFF; unusual Pillow-only formats are unsupported.
  JPEG encoder implementation, optimization and resulting bytes differ from
  Pillow, while quality, dimensions, RGB output and white alpha compositing are
  preserved. Rust's first-frame decoding can differ for animated images.
- XLSX uses rust_xlsxwriter: ZIP metadata, XML serialization, shared strings,
  empty-cell representation and document properties differ from openpyxl.
  Headers, row values, worksheet name and part naming are preserved. Widths
  follow maximum Unicode length plus two, capped at Excel's 255-character limit.
- Requests use reqwest's HTTP/TLS stack: generated transport headers, compression
  implementation, cookie handling, proxy behavior and error wording can differ.
  Timeouts bound the entire attempt, whereas Requests uses connect/read inactivity
  timeouts. The documented timeout values, retry counts/statuses/backoff and
  concurrency are retained. Stream-body failures are not retried, as in Python's
  streamed downloads. HTTP errors include Rust diagnostics instead of Python
  exception tracebacks. Discovery reads only the 500-byte preview it needs.
- URL parsing uses reqwest's URL normalization (including hostname case, IDNs
  and default ports), which can change derived shop names and keys for unusual
  input URLs. Unicode shop-name casing uses Rust character mappings and can
  differ from Python's `str.title()` for expanding case mappings.
- Console output uses English log messages and workbook-save notices rather than
  tqdm's terminal progress bars. Ctrl-C exits successfully; EOF at interactive
  prompts now exits cleanly instead of raising an unexpected-input error.
- No live website or public DNS access is needed by tests. DNS answers and
  safety policies are injected in tests only; the CLI has no bypass switch.

## Development

```sh
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Tests preserve both embedded Python XML fixtures and cover their original
feed/security cases, CLI exits, local HTTP requests, retry counts, redirects,
gallery output, JPEG transparency, XLSX contents and part splitting. CI runs
formatting, Clippy, tests and release builds on Ubuntu and Windows.

Dependencies have specific roles: clap for arguments, chrono for local dates,
ctrlc for interrupts, reqwest for HTTP/TLS/proxies/cookies/compression, roxmltree
for XML, encoding_rs for declared feed encodings, scraper for HTML, image for
image conversion, rust_xlsxwriter for Excel. Test-only httpmock serves local
responses and zip inspects workbook contents.

## License

[MIT](LICENSE)
