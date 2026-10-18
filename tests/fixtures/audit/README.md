# Differential audit fixtures

Captured from the supplied Python implementation before its removal, using Python
3.11.2 with Requests 2.34.2, Pillow 13.0.0, urllib3 2.8.0, BeautifulSoup, openpyxl,
defusedxml and pytest through `uv run --no-project --with ...`.

- `oracle.tsv`: XML observations as `filename stem<TAB>hex-encoded UTF-8`.
  Decoded values are `ok:<image count>:<first product name>` or `error`.
  The Rust integration test runs the same bytes and compares the observations.
- `*.xml`: exact input bytes, including empty, malformed ASCII, Latin-1,
  Windows-1250, UTF-16 in both byte orders, UTF-8 BOM/CRLF, XML comments,
  processing instructions, CDATA, namespaces and actual DTD declarations.
- `gallery-cp1250.html` / `.txt`: raw HTML bytes and Python's expected normalized
  gallery URL plus alt text, separated by a tab.
- `large-header.png`: exact PNG whose header advertises 10,000 × 6,000 pixels.
  Pillow opens the header with a warning; Rust rejects it before decoding.
- `python-results.txt`: XML observations, openpyxl's 32,767-character truncation,
  and the complete 80-case original pytest run with deterministic offline DNS.
- `python-network.txt`: loopback-server observations for duplicate galleries,
  a redirect whose destination fails the injected URL policy, custom-domain
  validation, gallery index failure, Google galleries, pixel limits, charset,
  invalid UTF-8 discovery, and concurrent directory creation. The original
  workbook crash used stubbed image downloads. Eight original OutputManagers
  were synchronized at the first existence check to demonstrate the directory race.
- `cli-results.txt`: actual Python and release Rust subprocess exit statuses and
  filesystem side effects on identical arguments and stdin. Explicit output
  directories avoid conflating the documented default-directory change.
- `rust-before.txt` / `rust-preview-before.txt`: compiled pre-fix regression failures.
- `cargo-*.txt`: successful final Rust validation transcripts.

The loopback HTTP handler redirects `/redirect` to `/gallery`; the latter serves
one main-image anchor and one thumbnail with identical `https://x/big/a` hrefs.
That placeholder is gallery data; image downloads are stubbed in the original
workbook-crash reproduction. `/universal.xml` returns bytes `< FF SHOP>` for the
UTF-8 discovery test. No public shop is part of the fixtures. Current Rust tests
recreate loopback servers with production safety replaced only by injected test
policies. Malformed XML and oversized image fixtures are intentional test data.
