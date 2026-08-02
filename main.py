#
# Project: shoptet-scraper
# File:    main.py
#
# Description:
# Command line scraper: finds Shoptet shops, reads their product feeds, and writes an Excel workbook with the product images.
#
# Author:
# Jan Alexandr Kopřiva
# jan.alexandr.kopriva@gmail.com
#
# License: MIT
#

import argparse
import logging
import sys
import xml.etree.ElementTree as ET
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime
from io import BytesIO
from pathlib import Path
from urllib.parse import urlparse

import requests
from bs4 import BeautifulSoup
from openpyxl import Workbook
from PIL import Image as PILImage
from requests.adapters import HTTPAdapter
from tqdm import tqdm
from urllib3.util.retry import Retry

from shoptet_scraper.feeds import XML_FEED_TYPES, get_xml_parser
from shoptet_scraper.security import (
    download_image_bytes,
    is_safe_url,
    sanitize_filename,
    sanitize_xlsx_value,
)


class ProductImageScraper:
    """Reads every gallery image off a product page."""

    def __init__(self, logger: logging.Logger):
        self.logger = logger
        self.session = self._create_session()

    def _create_session(self) -> requests.Session:
        """Session that retries on the status codes worth retrying."""
        session = requests.Session()
        retry = Retry(
            total=3,
            read=3,
            connect=3,
            backoff_factor=1.0,
            status_forcelist=(500, 502, 504, 408, 429),
        )
        adapter = HTTPAdapter(max_retries=retry)
        session.mount('http://', adapter)
        session.mount('https://', adapter)

        session.headers.update({
            'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36'
        })

        return session

    def extract_all_product_images(self, product_url: str) -> list[tuple[str, str]]:
        """Every gallery image on a product page, as (url, alt text).

        The page serves each image at three sizes. Only the /orig/ one is kept,
        so the saved file is the full resolution the shop holds.
        """
        try:
            if not is_safe_url(product_url):
                self.logger.error(f" Refused unsafe product URL: {product_url}")
                return []

            response = self.session.get(product_url, timeout=15)
            response.raise_for_status()

            soup = BeautifulSoup(response.content, 'html.parser')
            images = []

            main_image = soup.find('a', class_='p-main-image')
            if main_image:
                img_url = main_image.get('href')
                if img_url:
                    orig_url = img_url.replace('/big/', '/orig/')
                    alt_text = main_image.find('img').get('alt', '') if main_image.find('img') else ''
                    images.append((orig_url, alt_text))

            thumbnails = soup.find_all('a', class_='p-thumbnail')
            for thumbnail in thumbnails:
                img_url = thumbnail.get('href')
                if img_url and img_url not in [img[0] for img in images]:
                    orig_url = img_url.replace('/big/', '/orig/')
                    alt_text = thumbnail.find('img').get('alt', '') if thumbnail.find('img') else ''
                    images.append((orig_url, alt_text))

            # Lazy loaded images carry the URL in data-src only, so neither lookup
            # above finds them.
            data_src_images = soup.find_all('img', {'data-src': True})
            for img in data_src_images:
                data_src = img.get('data-src')
                if data_src and '/related/' in data_src:
                    orig_url = data_src.replace('/related/', '/orig/')
                    alt_text = img.get('alt', '')
                    if orig_url not in [img[0] for img in images]:
                        images.append((orig_url, alt_text))

        except Exception:
            self.logger.exception(f" Could not read images from {product_url}")
            return []
        else:
            self.logger.info(f" Found {len(images)} images for product: {product_url}")
            return images


class ShoptetStoreDiscovery:
    """Works out whether a URL is a Shoptet shop.

    No shop is hard coded. Every one is discovered at run time by asking the
    candidate URL for a feed.
    """

    def __init__(self, logger: logging.Logger):
        self.logger = logger
        self.session = self._create_session()

    def _create_session(self) -> requests.Session:
        """Session that retries on the status codes worth retrying."""
        session = requests.Session()
        retry = Retry(
            total=3,
            read=3,
            connect=3,
            backoff_factor=1.0,
            status_forcelist=(500, 502, 504, 408, 429),
        )
        adapter = HTTPAdapter(max_retries=retry)
        session.mount('http://', adapter)
        session.mount('https://', adapter)

        session.headers.update({
            'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36'
        })

        return session

    def discover_store_from_url(self, base_url: str) -> dict[str, str] | None:
        """Shop configuration for a URL, or None when it is not a Shoptet shop."""
        try:
            parsed = urlparse(base_url)
            if not parsed.scheme or not parsed.netloc:
                return None

            clean_url = f"{parsed.scheme}://{parsed.netloc}"

            if not self._is_shoptet_store(clean_url):
                return None

            store_name = parsed.netloc.replace('.shoptet.cz', '').replace('.cz', '').replace('www.', '')
            store_key = store_name.replace('-', '_').replace('.', '_')

            return {
                'name': store_name.title(),
                'base_url': clean_url,
                'feed_filename': f'feed_{store_key}',
                'store_key': store_key
            }

        except Exception:
            self.logger.exception(f" Could not inspect shop {base_url}")
            return None

    def _is_shoptet_store(self, url: str) -> bool:
        """True when the URL serves a Shoptet feed.

        Tries each known feed path, then falls back to looking for the Shoptet
        name on the home page.
        """
        try:
            # A shop URL arrives from outside, so it is checked before any request.
            if not is_safe_url(url):
                self.logger.warning(f" Refused unsafe shop URL: {url}")
                return False

            test_urls = [
                f"{url}/universal.xml",
                f"{url}/google.xml",
                f"{url}/heureka.xml"
            ]

            for test_url in test_urls:
                try:
                    # GET rather than HEAD: some shops answer HEAD with 405, and the
                    # body is needed anyway when the content type is not set.
                    response = self.session.get(test_url, timeout=15, stream=True)
                    if response.status_code == 200:
                        content_type = response.headers.get('content-type', '').lower()
                        if 'xml' in content_type or 'text/xml' in content_type:
                            self.logger.info(f" Shoptet shop found: {url} (feed: {test_url})")
                            return True

                        # Content type was no help, so look at the start of the body.
                        try:
                            content_preview = response.content[:500].decode('utf-8', errors='ignore')
                            if '<?xml' in content_preview or '<SHOP' in content_preview or '<rss' in content_preview:
                                self.logger.info(f" Shoptet shop found: {url} (XML body detected)")
                                return True
                        except UnicodeDecodeError:
                            # Binary body, so it is not one of the XML feeds.
                            pass

                except Exception as e:  # noqa: PERF203 - one probe per feed URL, the cost is the request
                    self.logger.debug(f" Probe {test_url} failed: {e}")
                    continue

            # No feed answered. The home page still names the platform.
            try:
                response = self.session.get(url, timeout=10)
                if response.status_code == 200:
                    content = response.text.lower()
                    if 'shoptet' in content or 'shoptet.cz' in content:
                        self.logger.info(f" Shoptet shop found: {url} (named on the home page)")
                        return True
            except Exception as e:
                self.logger.debug(f" Home page check failed: {e}")

            return False  # noqa: TRY300 - guarded on purpose, the happy path stays in the try

        except Exception as e:
            self.logger.debug(f" Could not check shop {url}: {e}")
            return False

    def discover_stores_from_list(self, url_list: list[str]) -> dict[str, dict[str, str]]:
        """Shop configurations for every URL in the list that is a Shoptet shop."""
        discovered_stores = {}

        self.logger.info(f" Checking {len(url_list)} URLs for Shoptet shops...")

        for url in url_list:
            store_config = self.discover_store_from_url(url)
            if store_config:
                store_key = store_config['store_key']
                discovered_stores[store_key] = store_config

        self.logger.info(f" Found {len(discovered_stores)} Shoptet shops")
        return discovered_stores


class OutputManager:
    """Numbered output folders, one per run, grouped by date."""

    def __init__(self, base_path: str | None = None):
        if base_path is None:
            base_path = str(Path(__file__).resolve().parent / 'output')

        self.base_path = base_path
        self.today = datetime.now().strftime("%Y-%m-%d")
        self.session_path = self._create_session_path()

    def _create_session_path(self) -> str:
        """Path for this run: today's date plus the first unused sequence number."""
        session_number = 1
        while True:
            session_folder = f"{self.today}_{session_number:03d}"
            session_path = str(Path(self.base_path) / session_folder)

            if not Path(session_path).exists():
                break
            session_number += 1

        Path(session_path).mkdir(parents=True, exist_ok=True)
        return session_path

    def get_store_path(self, store_name: str) -> str:
        """Folder for one shop inside this run. The shop name comes from the shop,
        so it goes through sanitize_filename first."""
        safe_name = sanitize_filename(store_name)
        store_path = str(Path(self.session_path) / safe_name)
        Path(store_path).mkdir(parents=True, exist_ok=True)
        return store_path

    def get_images_path(self, store_path: str) -> str:
        """Image folder inside one shop's folder."""
        images_path = str(Path(store_path) / 'images')
        Path(images_path).mkdir(parents=True, exist_ok=True)
        return images_path

    def get_session_info(self) -> dict[str, str]:
        """Where this run writes, and for which date."""
        return {
            'session_path': self.session_path,
            'date': self.today,
            'base_path': self.base_path
        }



def setup_logging():
    """Log to the console and to shoptet_scraper.log."""
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(levelname)s - %(message)s',
        handlers=[
            logging.FileHandler('shoptet_scraper.log'),
            logging.StreamHandler()
        ]
    )
    return logging.getLogger(__name__)


class ShoptetScraper:
    """One shop, one feed type: download it, read it, and write the workbook."""

    def __init__(self, store_config: dict[str, str], feed_type: str, logger: logging.Logger,
                 scrape_all_images: bool = False):
        self.store_config = store_config
        self.feed_type = feed_type
        self.feed_config = XML_FEED_TYPES[feed_type]
        self.logger = logger
        self.scrape_all_images = scrape_all_images
        self.session = self._create_session()
        self.xml_parser = get_xml_parser(feed_type, logger)
        self.image_scraper = ProductImageScraper(logger) if scrape_all_images else None

    def _create_session(self) -> requests.Session:
        """Session for the feed and the images, with a pool wide enough for the
        50 image workers."""
        session = requests.Session()
        retry = Retry(
            total=5,
            read=5,
            connect=5,
            backoff_factor=0.5,
            status_forcelist=(500, 502, 504),
        )
        adapter = HTTPAdapter(
            max_retries=retry,
            pool_connections=100,
            pool_maxsize=100
        )
        session.mount('http://', adapter)
        session.mount('https://', adapter)
        return session

    def validate_url(self, url: str) -> bool:
        """True when the URL is safe to request and looks like a Shoptet feed."""
        try:
            parsed = urlparse(url)
            # is_safe_url is what actually blocks a hostile target. The name check
            # below it only keeps the tool pointed at Shoptet feeds.
            return bool(
                parsed.scheme in ['http', 'https'] and
                parsed.netloc and
                ('shoptet' in parsed.netloc or 'universal.xml' in url) and
                is_safe_url(url)
            )
        except Exception:
            return False

    def download_xml(self, save_path: str) -> str | None:
        """Download the feed to save_path. None when it cannot be fetched."""
        xml_url = f"{self.store_config['base_url'].rstrip('/')}{self.feed_config['path']}"

        if not self.validate_url(xml_url):
            self.logger.error(f" Invalid URL: {xml_url}")
            return None

        self.logger.info(f" Downloading {self.feed_config['name']} from: {xml_url}")

        try:
            response = self.session.get(xml_url, stream=True, timeout=30)
            response.raise_for_status()

            today = datetime.now().strftime('%d-%m-%Y')
            xml_filename = f"{self.store_config['feed_filename']}_{self.feed_type}_{today}.xml"
            full_xml_path = str(Path(save_path) / xml_filename)

            total_size_in_bytes = int(response.headers.get('content-length', 0))
            block_size = 1024

            with Path(full_xml_path).open('wb') as file, tqdm(
                total=total_size_in_bytes,
                unit='iB',
                unit_scale=True,
                desc=f"Stahování {self.feed_config['name']}"
            ) as t:
                for chunk in response.iter_content(chunk_size=block_size):
                    if chunk:
                        file.write(chunk)
                        t.update(len(chunk))

            self.logger.info(f" {self.feed_config['name']} downloaded: {full_xml_path}")
            return full_xml_path  # noqa: TRY300 - the log line belongs with the value it reports

        except Exception:
            self.logger.exception(f" Could not download {self.feed_config['name']}")
            return None

    def parse_xml(self, xml_path: str) -> ET.Element | None:
        """Root element of the downloaded feed, or None when it will not parse."""
        return self.xml_parser.parse_xml(xml_path)

    def extract_image_data(self, root: ET.Element) -> list[tuple[int, str, str, str]]:
        """Image rows from the feed.

        The feed lists one image per product. With scrape_all_images the product
        pages are fetched as well, which is far slower but gets the whole gallery.
        """
        if not self.scrape_all_images:
            return self.xml_parser.extract_image_data(root)

        return self._extract_all_product_images(root)

    def _extract_all_product_images(self, root: ET.Element) -> list[tuple[int, str, str, str]]:
        """Gallery images for every product, one request per product page."""
        all_images = []
        shopitems = root.findall('.//SHOPITEM')

        self.logger.info(f" Reading galleries for {len(shopitems)} products...")

        for index, shopitem in enumerate(shopitems, start=1):
            product_url = shopitem.findtext('URL', default='')
            product_no = shopitem.findtext('PRODUCTNO', default=f"product_{index}")
            product_name = shopitem.findtext('PRODUCT', default=f"product_{index}")

            if not product_url:
                continue

            product_images = self.image_scraper.extract_all_product_images(product_url)

            for img_index, (img_url, _alt_text) in enumerate(product_images):
                # Product index and image index together, so two images of one
                # product cannot overwrite each other's file.
                unique_index = f"{index}_{img_index + 1}"
                all_images.append((unique_index, img_url, product_no, product_name))

        self.logger.info(f" Found {len(all_images)} gallery images in total")
        return all_images

    def download_and_save_image(self, data: tuple[int, str, str, str], images_folder: str) -> str | None:
        """Fetch one image and write it as JPEG. None when it cannot be used."""
        index, img_url, product_no, product_name = data
        try:
            image_bytes = download_image_bytes(self.session, img_url, timeout=10)
            if image_bytes is None:
                self.logger.error(f" Refused unsafe image URL: {img_url}")
                return None

            # security.py caps MAX_IMAGE_PIXELS. The warning and the error are both
            # left to surface: a decompression bomb has to be visible, not swallowed.
            image = PILImage.open(BytesIO(image_bytes))

            # JPEG has no alpha, and a straight convert('RGB') renders transparent
            # pixels black. Compositing onto white keeps the product on the
            # background the shop intended.
            if image.mode == 'P':
                image = image.convert('RGBA')
                background = PILImage.new('RGB', image.size, (255, 255, 255))
                background.paste(image, mask=image.split()[-1])
                image = background
            elif image.mode == 'RGBA':
                background = PILImage.new('RGB', image.size, (255, 255, 255))
                background.paste(image, mask=image.split()[-1])
                image = background
            elif image.mode not in ('RGB', 'L'):
                image = image.convert('RGB')

            # The name comes from the shop, so it goes through sanitize_filename.
            sanitized_name = sanitize_filename(product_name)
            sanitized_product_no = sanitize_filename(product_no)
            image_filename = f"{sanitized_name}_{sanitized_product_no}_{index}.jpg"
            image_path = str(Path(images_folder) / image_filename)

            if image.mode != 'RGB':
                image = image.convert('RGB')

            image.save(image_path, format='JPEG', quality=85, optimize=True)
            return image_filename  # noqa: TRY300 - guarded on purpose, the happy path stays in the try

        except Exception:
            self.logger.exception(f" Could not download image {img_url}")
            return None

    def process_products(self, root: ET.Element, image_data: list[tuple[int, str, str, str]],
                        images_folder: str, save_path: str) -> None:
        """Download the images and write the products to workbooks.

        Split every 1000 rows. Excel copes with more, but the files are opened by
        hand and a smaller part loads in reasonable time.
        """
        headers = self.xml_parser.get_headers()

        wb = create_workbook(headers)
        part_number = 1
        product_counter = 0

        # The Google feed wraps products in <item>, the others in <SHOPITEM>.
        if self.feed_type == 'google':
            shopitems = root.findall('.//item') or root.findall('.//SHOPITEM')
        else:
            shopitems = root.findall('.//SHOPITEM')

        total_images = len(image_data)

        self.logger.info(f" Processing {total_images} images from {self.feed_config['name']}")

        max_workers = 50
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            futures = {
                executor.submit(self.download_and_save_image, item, images_folder): item
                for item in image_data
            }

            with tqdm(total=total_images, desc=f"Zpracování {self.feed_config['name']}", unit="obrázek") as pbar:
                for future in as_completed(futures):
                    index, _img_url, product_no, _product_name = futures[future]
                    image_filename = future.result()

                    # image_data was built from shopitems in order, so the index maps back.
                    if index - 1 < len(shopitems):
                        shopitem = shopitems[index - 1]
                        row = self.xml_parser.extract_product_data(shopitem, product_no, image_filename)
                        row = [sanitize_xlsx_value(cell) for cell in row]
                        wb.active.append(row)
                        product_counter += 1

                        if product_counter % 1000 == 0:
                            base_filename = f"{self.store_config['feed_filename']}_{self.feed_type}_{datetime.now().strftime('%d-%m-%Y')}"
                            save_workbook(wb, save_path, base_filename, part_number)
                            wb = create_workbook(headers)
                            part_number += 1

                    pbar.update(1)

        # Whatever is left below the 1000 row split.
        if product_counter % 1000 != 0:
            base_filename = f"{self.store_config['feed_filename']}_{self.feed_type}_{datetime.now().strftime('%d-%m-%Y')}"
            save_workbook(wb, save_path, base_filename, part_number)

        self.logger.info(f" {self.feed_config['name']} done ({product_counter} products)")

    def cleanup_xml(self, xml_path: str) -> None:
        """Delete the downloaded feed once the workbook is written."""
        try:
            Path(xml_path).unlink()
            self.logger.info(f" Feed file deleted: {xml_path}")
        except Exception:
            self.logger.exception(" Could not delete the feed file")

    def check_xml_availability(self) -> bool:
        """True when this shop serves this feed type."""
        xml_url = f"{self.store_config['base_url'].rstrip('/')}{self.feed_config['path']}"

        try:
            response = self.session.head(xml_url, timeout=10)
            is_available = response.status_code == 200
            if is_available:
                self.logger.info(f" {self.feed_config['name']} available: {xml_url}")
            else:
                self.logger.warning(f" {self.feed_config['name']} unavailable (status {response.status_code}): {xml_url}")
            return is_available  # noqa: TRY300 - guarded on purpose, the happy path stays in the try
        except Exception as e:
            self.logger.warning(f" {self.feed_config['name']} unavailable ({e}): {xml_url}")
            return False


def check_xml_feeds_for_stores(discovered_stores: dict[str, dict[str, str]],
                               logger: logging.Logger) -> dict[str, dict[str, str]]:
    """Which feed types each discovered shop actually serves."""
    logger.info(" Checking which feeds are available...")
    available_stores = {}

    for store_key, store_config in discovered_stores.items():
        store_feeds = {}

        for feed_type, feed_config in XML_FEED_TYPES.items():
            scraper = ShoptetScraper(store_config, feed_type, logger)
            if scraper.check_xml_availability():
                store_feeds[feed_type] = feed_config

        if store_feeds:
            available_stores[store_key] = {
                'store_config': store_config,
                'available_feeds': store_feeds
            }

    logger.info(f" {len(available_stores)} shops serve at least one feed")
    return available_stores


def show_available_stores(available_stores: dict[str, dict[str, str]]) -> tuple[str, str] | None:
    """Ask which shop and feed to scrape. None when the user quits."""
    if not available_stores:
        print(" Žádné dostupné obchody nenalezeny!")
        return None

    print(f"\n=== Dostupné Shoptet obchody ({len(available_stores)}) ===")

    store_list = list(available_stores.items())
    for i, (_store_key, store_data) in enumerate(store_list, 1):
        store_config = store_data['store_config']
        available_feeds = store_data['available_feeds']
        feed_names = [feed_config['name'] for feed_config in available_feeds.values()]
        print(f"{i}. {store_config['name']} ({store_config['base_url']})")
        print(f"   Dostupné feedy: {', '.join(feed_names)}")

    print("0. Ukončit")

    while True:
        try:
            choice = input(f"\nVyberte obchod (0-{len(store_list)}): ").strip()

            if choice == '0':
                return None
            choice_num = int(choice)
            if 1 <= choice_num <= len(store_list):
                store_key, store_data = store_list[choice_num - 1]
                available_feeds = store_data['available_feeds']

                # One feed means no second question worth asking.
                if len(available_feeds) == 1:
                    feed_type = next(iter(available_feeds))
                    print(f" Automaticky vybrán {available_feeds[feed_type]['name']}")
                    return store_key, feed_type
                print(f"\nDostupné feedy pro {store_data['store_config']['name']}:")
                feed_list = list(available_feeds.items())
                for i, (_feed_key, feed_config) in enumerate(feed_list, 1):
                    print(f"{i}. {feed_config['name']}")

                feed_choice = input(f"Vyberte typ feedu (1-{len(feed_list)}): ").strip()
                feed_num = int(feed_choice)
                if 1 <= feed_num <= len(feed_list):
                    feed_type = feed_list[feed_num - 1][0]
                    return store_key, feed_type
                print(" Neplatná volba feedu")
                continue
            print(" Neplatná volba, zkuste znovu")
        except ValueError:
            print(" Neplatný vstup, zadejte číslo")
        except KeyboardInterrupt:
            return None


def requests_retry_session(retries=5, backoff_factor=0.5, status_forcelist=(500, 502, 504)):
    """Session that retries on the status codes worth retrying."""
    session = requests.Session()
    retry = Retry(
        total=retries,
        read=retries,
        connect=retries,
        backoff_factor=backoff_factor,
        status_forcelist=status_forcelist,
    )
    adapter = HTTPAdapter(max_retries=retry)
    session.mount('http://', adapter)
    session.mount('https://', adapter)
    return session




def download_and_save_image(data, session, images_folder):
    """Fetch one image and write it as JPEG. None when it cannot be used."""
    index, img_url, product_no, product_name = data
    try:
        image_bytes = download_image_bytes(session, img_url, timeout=10)
        if image_bytes is None:
            print(f"Rejected unsafe image URL: {img_url}")
            return None
        # security.py caps MAX_IMAGE_PIXELS. The warning is left to surface.
        image = PILImage.open(BytesIO(image_bytes))
        if image.mode in ('RGBA', 'P', 'CMYK'):
            image = image.convert('RGB')
        # The name comes from the shop, so it goes through sanitize_filename.
        sanitized_name = sanitize_filename(product_name)
        sanitized_product_no = sanitize_filename(product_no)
        image_filename = f"{sanitized_name}_{sanitized_product_no}_{index}.jpg"
        image_path = str(Path(images_folder) / image_filename)
        image.save(image_path, format='JPEG', quality=85)
        return image_filename  # noqa: TRY300 - guarded on purpose, the happy path stays in the try
    except Exception as e:
        print(f"Error downloading image at {img_url}: {e}")
        return None


def create_workbook(headers):
    """A workbook with the header row already written."""
    wb = Workbook()
    ws = wb.active
    ws.append(headers)
    return wb


def autofit_column_widths(worksheet):
    """Widen every column to its longest value."""
    for col in worksheet.columns:
        max_length = 0
        column = col[0].column_letter
        for cell in col:
            if cell.value is not None:
                max_length = max(max_length, len(str(cell.value)))
        adjusted_width = (max_length + 2)  # Two characters of padding.
        worksheet.column_dimensions[column].width = adjusted_width


def save_workbook(wb, save_path, base_filename, part):
    """Write one part of the workbook to disk."""
    autofit_column_widths(wb.active)
    xlsx_filename = f"{base_filename}_part_{part}.xlsx"
    full_path = str(Path(save_path) / xlsx_filename)
    wb.save(full_path)
    print(f"Saved part {part} to {full_path}")


def parse_arguments():
    """Read the command line."""
    parser = argparse.ArgumentParser(
        description='Univerzální Shoptet scraper pro stahování XML feedů a produktů',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Příklady použití:
  python main.py eshop                    # Interaktivní režim - zadání URL obchodů
  python main.py eshop --store https://example.shoptet.cz  # Přímé spuštění pro URL
  python main.py --help                   # Zobrazení nápovědy
        """
    )

    parser.add_argument(
        'command',
        choices=['eshop'],
        help='Příkaz k provedení (eshop = objevování Shoptet obchodů)'
    )

    parser.add_argument(
        '--store',
        type=str,
        help='Přímé zadání URL Shoptet obchodu'
    )

    parser.add_argument(
        '--output',
        type=str,
        help='Cesta pro výstupní složky (default: ./output)'
    )

    parser.add_argument(
        '--verbose', '-v',
        action='store_true',
        help='Verbose výstup'
    )

    parser.add_argument(
        '--all-images', '-a',
        action='store_true',
        help='Stahovat všechny obrázky z galerie produktů (pomalejší)'
    )

    return parser.parse_args()


def main():
    """Entry point: discover shops, pick a feed, and scrape it."""
    args = parse_arguments()
    logger = setup_logging()

    output_path = args.output if args.output else None
    output_manager = OutputManager(output_path)
    session_info = output_manager.get_session_info()
    logger.info(f" Session: {session_info['session_path']}")

    if args.command == 'eshop':
        if args.store:
            discovery = ShoptetStoreDiscovery(logger)
            store_config = discovery.discover_store_from_url(args.store)

            if not store_config:
                logger.error(f" No Shoptet shop at URL: {args.store}")
                return

            logger.info(f" Scraping shop: {store_config['name']} (Universal feed)")
            run_scraper_for_store(store_config, 'universal', output_manager, logger, args.all_images)
        else:
            print("\n=== Univerzální Shoptet Scraper ===")
            print("Zadejte URL Shoptet obchodů (jeden na řádek, prázdný řádek pro ukončení):")

            url_list = []
            while True:
                url = input("URL: ").strip()
                if not url:
                    break
                url_list.append(url)

            if not url_list:
                logger.info(" No URLs given")
                return

            discovery = ShoptetStoreDiscovery(logger)
            discovered_stores = discovery.discover_stores_from_list(url_list)

            if not discovered_stores:
                logger.error(" No Shoptet shops found")
                return

            available_stores = check_xml_feeds_for_stores(discovered_stores, logger)

            if not available_stores:
                logger.error(" No available feeds found")
                return

            selection = show_available_stores(available_stores)

            if selection:
                store_key, feed_type = selection
                store_data = available_stores[store_key]
                store_config = store_data['store_config']
                run_scraper_for_store(store_config, feed_type, output_manager, logger, args.all_images)



def run_scraper_for_store(store_config: dict[str, str], feed_type: str,
                          output_manager: OutputManager, logger: logging.Logger,
                          scrape_all_images: bool = False):
    """Run the whole pipeline for one shop: download, parse, images, workbook."""
    if feed_type not in XML_FEED_TYPES:
        logger.error(f" Unknown feed type: {feed_type}")
        return

    feed_config = XML_FEED_TYPES[feed_type]
    mode_text = "všechny obrázky z galerií" if scrape_all_images else "pouze hlavní obrázky"
    logger.info(f" Scraping shop: {store_config['name']}, feed: {feed_config['name']}, mode: {mode_text}")

    scraper = ShoptetScraper(store_config, feed_type, logger, scrape_all_images)

    store_path = output_manager.get_store_path(store_config['name'])
    images_folder = output_manager.get_images_path(store_path)

    xml_path = scraper.download_xml(store_path)
    if not xml_path:
        logger.error(" Could not download the feed")
        return

    root = scraper.parse_xml(xml_path)
    if root is None:
        logger.error(" Could not parse the feed")
        return

    image_data = scraper.extract_image_data(root)
    if not image_data:
        logger.warning(" No images to download")
        return

    scraper.process_products(root, image_data, images_folder, store_path)

    scraper.cleanup_xml(xml_path)

    logger.info(" Scraping finished")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n Ukončování programu...")
        sys.exit(0)
    except Exception as e:
        print(f" Neočekávaná chyba: {e}")
        sys.exit(1)
