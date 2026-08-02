# shoptet-scraper

Finds the product feeds of a Shoptet online shop, parses them into Excel files, and downloads the product images in parallel.

![python](https://img.shields.io/badge/python-3.8+-3776AB?style=flat-square&logo=python&logoColor=white)
![license](https://img.shields.io/badge/license-MIT-A31F34?style=flat-square)
![status](https://img.shields.io/badge/status-active-22863A?style=flat-square)

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
ShoptetStoreDiscovery   Resolves the URL to a canonical Shoptet domain and probes the five paths.
XMLFeedParser           One concrete parser per feed format.
OutputManager           Session folders and the Excel writer.
ThreadPoolExecutor      Parallel image download.
```

The tool streams each feed and splits the rows into chunks, because Excel holds one million rows per sheet. The parsers subclass one abstract class, so a sixth feed format costs about 50 lines.

## Limits

- No resume. A crash leaves a half filled session folder. Delete it and run again.
- The image download has no bandwidth limit. A shop with tens of thousands of products can trigger rate limiting at the origin.
- Discovery assumes the current Shoptet URL convention. A custom template that moves the feed paths fails.
- No tests.

## License

[MIT](LICENSE)
