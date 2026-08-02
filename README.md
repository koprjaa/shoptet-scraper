# shoptet-scraper

Finds the product feeds of a Shoptet online shop, parses them into Excel files, and downloads the product images in parallel.

![python](https://img.shields.io/badge/python-3.8+-3776AB?style=flat-square&logo=python&logoColor=white)
![license](https://img.shields.io/badge/license-MIT-A31F34?style=flat-square)
![status](https://img.shields.io/badge/status-active-22863A?style=flat-square)
[![ci](https://github.com/koprjaa/shoptet-scraper/actions/workflows/ci.yml/badge.svg)](https://github.com/koprjaa/shoptet-scraper/actions/workflows/ci.yml)

## What it does

Shoptet is a Czech and Slovak e-commerce platform that runs about 30,000 shops. Every shop serves its product data at fixed paths: `/universal.xml`, `/google.xml`, `/heureka.xml`, `/zbozi.xml`, and `/supplier.xml`. This tool probes those five paths, parses each feed it finds, writes the result to Excel, and downloads the images.

## Install

```bash
uv venv
uv pip install -r requirements.txt
```

## Use

Interactive mode. Paste one URL per line and submit with a blank line:

```bash
python main.py eshop
```

Direct mode:

```bash
python main.py eshop --store https://www.gardners-eshop.cz
```

Download the full image gallery instead of the main image only. This is slower:

```bash
python main.py eshop --store https://example.shoptet.cz --all-images
```

Output goes to numbered session folders, in the form `YYYY-MM-DD_NNN`. Repeated runs against the same shop do not overwrite an earlier snapshot.

```
output/2026-04-17_001/
  feed_gardners_universal_17-04-2026_part_1.xlsx
  feed_gardners_universal_17-04-2026_part_2.xlsx
  images/
```

## Options

| Flag | Default | Effect |
|---|---|---|
| `eshop` | required | Mode. Discovers the feeds and scrapes them. |
| `--store <url>` | none | Skip the interactive URL prompt. |
| `--output <path>` | `./output` | Parent folder for the session folders. |
| `-v, --verbose` | off | Enable DEBUG logging. |
| `-a, --all-images` | off | Download every gallery image, not the main one only. |

## How it works

```
src/shoptet_scraper/security.py   URL checks, size limits, XML and file name sanitizing
src/shoptet_scraper/feeds.py      Feed definitions and the parser
main.py                           Discovery, Excel output, image download, CLI
```

One parser reads every feed. Google Shopping names its fields `id`, `title` and `image_link`, the Czech comparison sites use `PRODUCTNO`, `PRODUCT` and `IMGURL`, and the parser tries each spelling in turn. A sixth format costs one entry in `XML_FEED_TYPES` plus the tag names it uses.

The tool streams each feed and splits the rows into chunks, because Excel holds one million rows per sheet.

Three checks guard the trust boundary.

The URL is resolved before the request and refused when it points at a private, loopback, link-local or carrier-grade NAT address, so a shop cannot aim the scraper at the machine it runs on. That check alone is not enough: the HTTP library resolves the name a second time, and a host under someone else's control can answer differently that time. So the socket is asked where it actually landed, and the body is not read until that address has passed the same test.

Product names are quoted before they reach the spreadsheet, so a name beginning with `=` is not run as a formula.

## Limits

- No resume. A crash leaves a half filled session folder. Delete it and run again.
- The image download has no bandwidth limit. A shop with tens of thousands of products can trigger rate limiting at the origin.
- Discovery assumes the current Shoptet URL convention. A custom template that moves the feed paths fails.
- The image is fetched over one connection whose peer is checked, but a redirect to a different host is followed by the HTTP library without a second check. Disable redirects if the shop is not trusted at all.

## Development

```bash
uv run --extra dev ruff check .
uv run --extra dev pytest -q
```

The suite covers the feed parsing and the safety checks with no network. CI runs both on Python 3.10, 3.11, and 3.12, on Linux and Windows.

## License

[MIT](LICENSE)
