# shoptet-scraper

**Discovers Shoptet e-shops from a URL and downloads their product feeds (Universal, Google Shopping, Heureka, Zboží.cz) into Excel, with parallel image downloads.**

![python](https://img.shields.io/badge/python-3.7+-3776AB?style=flat-square&logo=python&logoColor=white)
![license](https://img.shields.io/badge/license-MIT-A31F34?style=flat-square)
![status](https://img.shields.io/badge/status-active-22863A?style=flat-square)
![requests](https://img.shields.io/badge/requests-2.x-000?style=flat-square)
![openpyxl](https://img.shields.io/badge/openpyxl-3.x-217346?style=flat-square)
![Pillow](https://img.shields.io/badge/Pillow-11.x-8B0000?style=flat-square)

Shoptet is a large Czech/Slovak e-commerce platform (used by ~30,000 shops). Every shop exposes product data at conventional `/universal.xml`, `/google.xml`, `/heureka.xml`, `/zbozi.xml`, `/supplier.xml` endpoints. This tool auto-detects which feeds a given shop serves, parses them, writes to Excel in session-numbered folders, and downloads product images in parallel.

## Quick start

Interactive (paste URLs, one per line, blank line to submit):

```bash
python main.py eshop
```

Direct:

```bash
python main.py eshop --store https://www.gardners-eshop.cz
```

With image gallery (slower — downloads every image, not just the main one):

```bash
python main.py eshop --store https://example.shoptet.cz --all-images
```

## What happens per run

1. Shop discovery — resolves URL to a canonical Shoptet domain and probes the five known feed endpoints
2. For every feed found, picks the matching parser (`UniversalXMLParser`, `GoogleXMLParser`, `HeurekaXMLParser`, …)
3. Streams the feed, splits into chunks for Excel's 1M-row limit
4. Parallel image download via `ThreadPoolExecutor`, saved as `productN_NNN_M.jpg` in per-shop `images/`
5. Writes one Excel per chunk with auto-sized columns

Output structure:

```
output/
└── 2026-04-17_001/
    └── GardnersShop/
        ├── images/
        │   ├── product1_001_1.jpg
        │   └── product2_001_2.jpg
        ├── feed_gardners_universal_17-04-2026_part_1.xlsx
        └── feed_gardners_universal_17-04-2026_part_2.xlsx
```

Session folders are auto-numbered (`YYYY-MM-DD_NNN`), so you can run the scraper repeatedly against the same shop without clobbering earlier snapshots.

## Flags

| flag | default | effect |
|------|---------|--------|
| `eshop` | required positional | mode: discover + scrape |
| `--store <url>` | — | skip interactive URL input |
| `--output <path>` | `./output` | session parent folder |
| `--verbose, -v` | off | enable DEBUG logs |
| `--all-images, -a` | off | download every image in gallery, not just main |

## Architecture

```
ShoptetStoreDiscovery  →  ShoptetScraper  →  XMLFeedParser (Universal / Google / Heureka …)
                                          →  OutputManager  (session folders + xlsx writer)
                                          →  ThreadPoolExecutor  (parallel image DL)
```

Parsers are abstract-class + concrete implementations, so adding a sixth feed format (e.g. `/facebook.xml`) is ~50 lines.

## Limits

- No resume — a crash mid-run means the session folder is half-populated. Just delete it and re-run.
- Image download has no bandwidth throttle; on shops with tens of thousands of products, expect temporary rate-limiting from the origin.
- Assumes Shoptet's current URL convention; custom templates with relocated feed endpoints will fail discovery.

## License

[MIT](LICENSE)
