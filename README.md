# shoptet-scraper

Finds a Shoptet shop's product feeds and exports them to Excel with images.

[![CI](https://github.com/koprjaa/shoptet-scraper/actions/workflows/ci.yml/badge.svg)](https://github.com/koprjaa/shoptet-scraper/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-A31F34)](LICENSE)

## What it does

Reads Shoptet XML feeds, writes product data to Excel workbooks, and saves
product images as JPEG files. Interactive mode lets you choose among available
Universal, Google Shopping, Heureka, Zboží.cz, and Shoptet Supplier feeds.

Workbooks contain up to 1,000 data rows each, including product identifiers,
prices, URLs, and image filenames. Products without an image are omitted.

## Install

Install the stable Rust toolchain, then run this from the repository directory:

```sh
cargo install --path .
```

No API keys or required environment variables are needed.

## Use

Start an interactive session:

```sh
shoptet-scraper eshop
```

Enter one shop URL per line, finish with a blank line, then choose a shop and feed.

To use a shop's universal feed directly, replace the example URL with your shop:

```sh
shoptet-scraper eshop --store https://example.shoptet.cz
```

Download gallery images and choose an output directory:

```sh
shoptet-scraper eshop --store https://example.shoptet.cz --all-images --output ./exports
```

By default, results go into `./output/YYYY-MM-DD_NNN/`, with a folder per shop
containing workbooks and an `images/` directory. Gallery mode writes one row per
image. Logs append to `shoptet_scraper.log` in the working directory.
See `shoptet-scraper --help` for the remaining options.

## License

[MIT](LICENSE)
