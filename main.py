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

# Typy XML feedů podporované Shoptetem

# Žádné hardcodované obchody - vše se objevuje dynamicky


class ProductImageScraper:
    """
    Třída pro stahování všech obrázků z galerie produktů.
    """

    def __init__(self, logger: logging.Logger):
        """
        Inicializace scraperu obrázků produktů.

        Args:
            logger: Logger instance
        """
        self.logger = logger
        self.session = self._create_session()

    def _create_session(self) -> requests.Session:
        """Vytvoření session s retry mechanizmem."""
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

        # Nastavení výchozích headers
        session.headers.update({
            'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36'
        })

        return session

    def extract_all_product_images(self, product_url: str) -> list[tuple[str, str]]:
        """
        Extrakce všech obrázků z galerie produktu.

        Args:
            product_url: URL stránky produktu

        Returns:
            Seznam tuple (image_url, alt_text) pro všechny obrázky
        """
        try:
            if not is_safe_url(product_url):
                self.logger.error(f" Odmítnuta nebezpečná URL produktu: {product_url}")
                return []

            response = self.session.get(product_url, timeout=15)
            response.raise_for_status()

            soup = BeautifulSoup(response.content, 'html.parser')
            images = []

            # Hledání hlavního obrázku
            main_image = soup.find('a', class_='p-main-image')
            if main_image:
                img_url = main_image.get('href')
                if img_url:
                    # Konverze z /big/ na /orig/ pro nejvyšší kvalitu
                    orig_url = img_url.replace('/big/', '/orig/')
                    alt_text = main_image.find('img').get('alt', '') if main_image.find('img') else ''
                    images.append((orig_url, alt_text))

            # Hledání obrázků v galerii miniatur
            thumbnails = soup.find_all('a', class_='p-thumbnail')
            for thumbnail in thumbnails:
                img_url = thumbnail.get('href')
                if img_url and img_url not in [img[0] for img in images]:  # Vyhnutí se duplicitám
                    # Konverze z /big/ na /orig/ pro nejvyšší kvalitu
                    orig_url = img_url.replace('/big/', '/orig/')
                    alt_text = thumbnail.find('img').get('alt', '') if thumbnail.find('img') else ''
                    images.append((orig_url, alt_text))

            # Alternativní hledání pomocí data-src atributů
            data_src_images = soup.find_all('img', {'data-src': True})
            for img in data_src_images:
                data_src = img.get('data-src')
                if data_src and '/related/' in data_src:
                    # Konverze z /related/ na /orig/ pro nejvyšší kvalitu
                    orig_url = data_src.replace('/related/', '/orig/')
                    alt_text = img.get('alt', '')
                    if orig_url not in [img[0] for img in images]:  # Vyhnutí se duplicitám
                        images.append((orig_url, alt_text))

        except Exception:
            self.logger.exception(f" Chyba při extrakci obrázků z {product_url}")
            return []
        else:
            self.logger.info(f" Nalezeno {len(images)} obrázků pro produkt: {product_url}")
            return images


class ShoptetStoreDiscovery:
    """
    Třída pro dynamické objevování Shoptet obchodů.
    """

    def __init__(self, logger: logging.Logger):
        """
        Inicializace discovery třídy.

        Args:
            logger: Logger instance
        """
        self.logger = logger
        self.session = self._create_session()

    def _create_session(self) -> requests.Session:
        """Vytvoření session s retry mechanizmem."""
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

        # Nastavení výchozích headers
        session.headers.update({
            'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36'
        })

        return session

    def discover_store_from_url(self, base_url: str) -> dict[str, str] | None:
        """
        Objevení Shoptet obchodu z URL.

        Args:
            base_url: Základní URL obchodu

        Returns:
            Konfigurace obchodu nebo None
        """
        try:
            parsed = urlparse(base_url)
            if not parsed.scheme or not parsed.netloc:
                return None

            # Vyčištění URL
            clean_url = f"{parsed.scheme}://{parsed.netloc}"

            # Kontrola, zda je to Shoptet obchod
            if not self._is_shoptet_store(clean_url):
                return None

            # Vytvoření názvu obchodu z URL
            store_name = parsed.netloc.replace('.shoptet.cz', '').replace('.cz', '').replace('www.', '')
            store_key = store_name.replace('-', '_').replace('.', '_')

            return {
                'name': store_name.title(),
                'base_url': clean_url,
                'feed_filename': f'feed_{store_key}',
                'store_key': store_key
            }

        except Exception:
            self.logger.exception(f" Chyba při objevování obchodu {base_url}")
            return None

    def _is_shoptet_store(self, url: str) -> bool:
        """
        Kontrola, zda je URL Shoptet obchod.

        Args:
            url: URL k ověření

        Returns:
            True pokud je to Shoptet obchod
        """
        try:
            # SSRF ochrana: odmítnutí neveřejných cílů.
            if not is_safe_url(url):
                self.logger.warning(f" Odmítnuta nebezpečná URL obchodu: {url}")
                return False

            # Kontrola některého z XML feedů
            test_urls = [
                f"{url}/universal.xml",
                f"{url}/google.xml",
                f"{url}/heureka.xml"
            ]

            for test_url in test_urls:
                try:
                    # Zvýšení timeoutu na 15 sekund a použití GET místo HEAD
                    response = self.session.get(test_url, timeout=15, stream=True)
                    if response.status_code == 200:
                        # Kontrola, zda obsahuje XML hlavičku nebo Shoptet značky
                        content_type = response.headers.get('content-type', '').lower()
                        if 'xml' in content_type or 'text/xml' in content_type:
                            self.logger.info(f" Shoptet obchod nalezen: {url} (feed: {test_url})")
                            return True

                        # Alternativní kontrola - načtení prvních 500 bytů pro kontrolu XML obsahu
                        try:
                            content_preview = response.content[:500].decode('utf-8', errors='ignore')
                            if '<?xml' in content_preview or '<SHOP' in content_preview or '<rss' in content_preview:
                                self.logger.info(f" Shoptet obchod nalezen: {url} (XML obsah detekován)")
                                return True
                        except UnicodeDecodeError:
                            # Binary body, so it is not one of the XML feeds.
                            pass

                except Exception as e:  # noqa: PERF203 - one probe per feed URL, the cost is the request
                    self.logger.debug(f" Test URL {test_url} selhal: {e}")
                    continue

            # Fallback: kontrola hlavní stránky pro Shoptet značky
            try:
                response = self.session.get(url, timeout=10)
                if response.status_code == 200:
                    content = response.text.lower()
                    if 'shoptet' in content or 'shoptet.cz' in content:
                        self.logger.info(f" Shoptet obchod nalezen: {url} (Shoptet značky na hlavní stránce)")
                        return True
            except Exception as e:
                self.logger.debug(f" Fallback kontrola hlavní stránky selhala: {e}")

            return False  # noqa: TRY300 - guarded on purpose, the happy path stays in the try

        except Exception as e:
            self.logger.debug(f" Chyba při kontrole Shoptet obchodu {url}: {e}")
            return False

    def discover_stores_from_list(self, url_list: list[str]) -> dict[str, dict[str, str]]:
        """
        Objevení více obchodů ze seznamu URL.

        Args:
            url_list: Seznam URL k testování

        Returns:
            Slovník nalezených obchodů
        """
        discovered_stores = {}

        self.logger.info(f" Objevování Shoptet obchodů z {len(url_list)} URL...")

        for url in url_list:
            store_config = self.discover_store_from_url(url)
            if store_config:
                store_key = store_config['store_key']
                discovered_stores[store_key] = store_config

        self.logger.info(f" Nalezeno {len(discovered_stores)} Shoptet obchodů")
        return discovered_stores


class OutputManager:
    """
    Třída pro správu výstupních složek s číslováním.
    """

    def __init__(self, base_path: str | None = None):
        """
        Inicializace output manageru.

        Args:
            base_path: Základní cesta pro výstupy (default: ./output)
        """
        if base_path is None:
            # Uložení do kořenové složky projektu
            base_path = str(Path(__file__).resolve().parent / 'output')

        self.base_path = base_path
        self.today = datetime.now().strftime("%Y-%m-%d")
        self.session_path = self._create_session_path()

    def _create_session_path(self) -> str:
        """
        Vytvoření cesty pro aktuální session s číslováním.

        Returns:
            Cesta k session složce
        """
        # Najít nejvyšší číslo pro dnešní datum
        session_number = 1
        while True:
            session_folder = f"{self.today}_{session_number:03d}"
            session_path = str(Path(self.base_path) / session_folder)

            if not Path(session_path).exists():
                break
            session_number += 1

        # Vytvoření složky
        Path(session_path).mkdir(parents=True, exist_ok=True)
        return session_path

    def get_store_path(self, store_name: str) -> str:
        """
        Vytvoření cesty pro konkrétní obchod.

        Args:
            store_name: Název obchodu

        Returns:
            Cesta k složce obchodu
        """
        # Sanitizace názvu obchodu
        safe_name = sanitize_filename(store_name)
        store_path = str(Path(self.session_path) / safe_name)
        Path(store_path).mkdir(parents=True, exist_ok=True)
        return store_path

    def get_images_path(self, store_path: str) -> str:
        """
        Vytvoření cesty pro obrázky obchodu.

        Args:
            store_path: Cesta k složce obchodu

        Returns:
            Cesta k složce obrázků
        """
        images_path = str(Path(store_path) / 'images')
        Path(images_path).mkdir(parents=True, exist_ok=True)
        return images_path

    def get_session_info(self) -> dict[str, str]:
        """
        Získání informací o aktuální session.

        Returns:
            Slovník s informacemi o session
        """
        return {
            'session_path': self.session_path,
            'date': self.today,
            'base_path': self.base_path
        }



def setup_logging():
    """Nastavení logging systému podle preferencí uživatele."""
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
    """
    Univerzální scraper pro Shoptet obchody.

    Podporuje stahování různých typů XML feedů z libovolného Shoptet obchodu,
    zpracování produktů a export do Excel souborů s obrázky.
    """

    def __init__(self, store_config: dict[str, str], feed_type: str, logger: logging.Logger,
                 scrape_all_images: bool = False):
        """
        Inicializace scraperu.

        Args:
            store_config: Konfigurace obchodu z SHOPTET_STORES
            feed_type: Typ XML feedu z XML_FEED_TYPES
            logger: Logger instance
            scrape_all_images: Zda stahovat všechny obrázky z galerie produktů
        """
        self.store_config = store_config
        self.feed_type = feed_type
        self.feed_config = XML_FEED_TYPES[feed_type]
        self.logger = logger
        self.scrape_all_images = scrape_all_images
        self.session = self._create_session()
        self.xml_parser = get_xml_parser(feed_type, logger)
        self.image_scraper = ProductImageScraper(logger) if scrape_all_images else None

    def _create_session(self) -> requests.Session:
        """Vytvoření session s retry mechanizmem."""
        session = requests.Session()
        retry = Retry(
            total=5,
            read=5,
            connect=5,
            backoff_factor=0.5,
            status_forcelist=(500, 502, 504),
        )
        # Zvýšení velikosti connection poolu pro paralelní stahování
        adapter = HTTPAdapter(
            max_retries=retry,
            pool_connections=100,
            pool_maxsize=100
        )
        session.mount('http://', adapter)
        session.mount('https://', adapter)
        return session

    def validate_url(self, url: str) -> bool:
        """
        Validace URL pro Shoptet XML feed.

        Args:
            url: URL k validaci

        Returns:
            True pokud je URL validní, False jinak
        """
        try:
            parsed = urlparse(url)
            # SSRF ochrana je skutečným garantem; doménová heuristika je jen
            # zachování očekávání, že jde o Shoptet feed.
            return bool(
                parsed.scheme in ['http', 'https'] and
                parsed.netloc and
                ('shoptet' in parsed.netloc or 'universal.xml' in url) and
                is_safe_url(url)
            )
        except Exception:
            return False

    def download_xml(self, save_path: str) -> str | None:
        """
        Stažení XML feedu z obchodu.

        Args:
            save_path: Cesta pro uložení XML souboru

        Returns:
            Cesta k staženému XML souboru nebo None při chybě
        """
        # Sestavení URL pro konkrétní typ feedu
        xml_url = f"{self.store_config['base_url'].rstrip('/')}{self.feed_config['path']}"

        if not self.validate_url(xml_url):
            self.logger.error(f" Neplatná URL: {xml_url}")
            return None

        self.logger.info(f" Stahování {self.feed_config['name']} z: {xml_url}")

        try:
            response = self.session.get(xml_url, stream=True, timeout=30)
            response.raise_for_status()

            # Vytvoření názvu souboru
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

            self.logger.info(f" {self.feed_config['name']} úspěšně staženo: {full_xml_path}")
            return full_xml_path  # noqa: TRY300 - the log line belongs with the value it reports

        except Exception:
            self.logger.exception(f" Chyba při stahování {self.feed_config['name']}")
            return None

    def parse_xml(self, xml_path: str) -> ET.Element | None:
        """
        Parsování XML souboru pomocí příslušného parseru.

        Args:
            xml_path: Cesta k XML souboru

        Returns:
            Root element XML nebo None při chybě
        """
        return self.xml_parser.parse_xml(xml_path)

    def extract_image_data(self, root: ET.Element) -> list[tuple[int, str, str, str]]:
        """
        Extrakce dat obrázků z XML pomocí příslušného parseru.

        Args:
            root: Root element XML

        Returns:
            Seznam tuple (index, img_url, product_no, product_name)
        """
        if not self.scrape_all_images:
            return self.xml_parser.extract_image_data(root)

        # Rozšířená extrakce pro všechny obrázky z galerie
        return self._extract_all_product_images(root)

    def _extract_all_product_images(self, root: ET.Element) -> list[tuple[int, str, str, str]]:
        """
        Extrakce všech obrázků z galerie produktů.

        Args:
            root: Root element XML

        Returns:
            Seznam tuple (index, img_url, product_no, product_name)
        """
        all_images = []
        shopitems = root.findall('.//SHOPITEM')

        self.logger.info(f" Extrakce všech obrázků z {len(shopitems)} produktů...")

        for index, shopitem in enumerate(shopitems, start=1):
            product_url = shopitem.findtext('URL', default='')
            product_no = shopitem.findtext('PRODUCTNO', default=f"product_{index}")
            product_name = shopitem.findtext('PRODUCT', default=f"product_{index}")

            if not product_url:
                continue

            # Extrakce všech obrázků z galerie produktu
            product_images = self.image_scraper.extract_all_product_images(product_url)

            for img_index, (img_url, _alt_text) in enumerate(product_images):
                # Vytvoření jedinečného indexu pro každý obrázek
                unique_index = f"{index}_{img_index + 1}"
                all_images.append((unique_index, img_url, product_no, product_name))

        self.logger.info(f" Nalezeno celkem {len(all_images)} obrázků z galerií produktů")
        return all_images

    def download_and_save_image(self, data: tuple[int, str, str, str], images_folder: str) -> str | None:
        """
        Stažení a uložení obrázku jako .jpg.

        Args:
            data: tuple (index, img_url, product_no, product_name)
            images_folder: Složka pro uložení obrázků

        Returns:
            Název souboru obrázku nebo None při chybě
        """
        index, img_url, product_no, product_name = data
        try:
            image_bytes = download_image_bytes(self.session, img_url, timeout=10)
            if image_bytes is None:
                self.logger.error(f" Odmítnuta nebezpečná URL obrázku: {img_url}")
                return None

            # MAX_IMAGE_PIXELS limit je aktivní globálně; DecompressionBombWarning
            # i Error NEpotlačujeme, aby se bomby projevily.
            image = PILImage.open(BytesIO(image_bytes))

            # Konverze na RGB s lepším zpracováním průhlednosti
            if image.mode == 'P':
                # Palette s průhledností -> RGBA -> RGB s bílým pozadím
                image = image.convert('RGBA')
                # Vytvoření bílého pozadí pro RGBA obrázky
                background = PILImage.new('RGB', image.size, (255, 255, 255))
                background.paste(image, mask=image.split()[-1])  # Použije alpha kanál jako masku
                image = background
            elif image.mode == 'RGBA':
                # RGBA -> RGB s bílým pozadím
                background = PILImage.new('RGB', image.size, (255, 255, 255))
                background.paste(image, mask=image.split()[-1])  # Použije alpha kanál jako masku
                image = background
            elif image.mode == 'CMYK':
                image = image.convert('RGB')
            elif image.mode not in ('RGB', 'L'):
                # Jiné módy -> RGB
                image = image.convert('RGB')

            # Sanitizace názvu souboru a přidání indexu pro jedinečnost
            sanitized_name = sanitize_filename(product_name)
            sanitized_product_no = sanitize_filename(product_no)
            image_filename = f"{sanitized_name}_{sanitized_product_no}_{index}.jpg"
            image_path = str(Path(images_folder) / image_filename)

            # Uložení obrázku jako JPEG s kvalitou 85
            # Zajištění, že obrázek je v RGB módu před uložením
            if image.mode != 'RGB':
                image = image.convert('RGB')

            image.save(image_path, format='JPEG', quality=85, optimize=True)
            return image_filename  # noqa: TRY300 - guarded on purpose, the happy path stays in the try

        except Exception:
            self.logger.exception(f" Chyba při stahování obrázku {img_url}")
            return None

    def process_products(self, root: ET.Element, image_data: list[tuple[int, str, str, str]],
                        images_folder: str, save_path: str) -> None:
        """
        Zpracování produktů a vytvoření Excel souborů.

        Args:
            root: Root element XML
            image_data: Seznam dat obrázků
            images_folder: Složka pro obrázky
            save_path: Cesta pro uložení Excel souborů
        """
        headers = self.xml_parser.get_headers()

        wb = create_workbook(headers)
        part_number = 1
        product_counter = 0

        # Najít všechny produkty podle typu feedu
        if self.feed_type == 'google':
            shopitems = root.findall('.//item') or root.findall('.//SHOPITEM')
        else:
            shopitems = root.findall('.//SHOPITEM')

        total_images = len(image_data)

        self.logger.info(f" Zpracovávání {total_images} obrázků z {self.feed_config['name']}")

        # Paralelní stahování obrázků
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

                    # Najít odpovídající shopitem pomocí indexu
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

        # Uložení poslední části
        if product_counter % 1000 != 0:
            base_filename = f"{self.store_config['feed_filename']}_{self.feed_type}_{datetime.now().strftime('%d-%m-%Y')}"
            save_workbook(wb, save_path, base_filename, part_number)

        self.logger.info(f" {self.feed_config['name']} zpracován úspěšně ({product_counter} produktů)")

    def cleanup_xml(self, xml_path: str) -> None:
        """
        Smazání XML souboru po zpracování.

        Args:
            xml_path: Cesta k XML souboru
        """
        try:
            Path(xml_path).unlink()
            self.logger.info(f" XML soubor smazán: {xml_path}")
        except Exception:
            self.logger.exception(" Chyba při mazání XML souboru: ")

    def check_xml_availability(self) -> bool:
        """
        Kontrola dostupnosti XML feedu pro tento obchod a typ feedu.

        Returns:
            True pokud je XML dostupný, False jinak
        """
        xml_url = f"{self.store_config['base_url'].rstrip('/')}{self.feed_config['path']}"

        try:
            response = self.session.head(xml_url, timeout=10)
            is_available = response.status_code == 200
            if is_available:
                self.logger.info(f" {self.feed_config['name']} dostupný: {xml_url}")
            else:
                self.logger.warning(f" {self.feed_config['name']} nedostupný (status {response.status_code}): {xml_url}")
            return is_available  # noqa: TRY300 - guarded on purpose, the happy path stays in the try
        except Exception as e:
            self.logger.warning(f" {self.feed_config['name']} nedostupný ({e}): {xml_url}")
            return False


def check_xml_feeds_for_stores(discovered_stores: dict[str, dict[str, str]],
                               logger: logging.Logger) -> dict[str, dict[str, str]]:
    """
    Kontrola dostupnosti XML feedů pro objevené obchody.

    Args:
        discovered_stores: Slovník objevených obchodů
        logger: Logger instance

    Returns:
        Slovník dostupných obchodů s dostupnými feedy
    """
    logger.info(" Kontrola dostupnosti XML feedů...")
    available_stores = {}

    for store_key, store_config in discovered_stores.items():
        store_feeds = {}

        # Kontrola všech typů feedů pro tento obchod
        for feed_type, feed_config in XML_FEED_TYPES.items():
            scraper = ShoptetScraper(store_config, feed_type, logger)
            if scraper.check_xml_availability():
                store_feeds[feed_type] = feed_config

        if store_feeds:
            available_stores[store_key] = {
                'store_config': store_config,
                'available_feeds': store_feeds
            }

    logger.info(f" Nalezeno {len(available_stores)} obchodů s dostupnými feedy")
    return available_stores


def show_available_stores(available_stores: dict[str, dict[str, str]]) -> tuple[str, str] | None:
    """
    Zobrazení dostupných obchodů a výběr obchodu s typem feedu.

    Args:
        available_stores: Slovník dostupných obchodů s feedy

    Returns:
        Tuple (store_key, feed_type) nebo None
    """
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

                # Výběr typu feedu
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
    """Vytvoření session s retry mechanizmem."""
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
    """
    Stažení a uložení obrázku jako .jpg.
    data: tuple (index, img_url, product_no, product_name)
    """
    index, img_url, product_no, product_name = data
    try:
        image_bytes = download_image_bytes(session, img_url, timeout=10)
        if image_bytes is None:
            print(f"Rejected unsafe image URL: {img_url}")
            return None
        # MAX_IMAGE_PIXELS limit aktivní globálně; warning nepotlačujeme.
        image = PILImage.open(BytesIO(image_bytes))
        # Konverze na RGB, pokud je to potřeba
        if image.mode in ('RGBA', 'P', 'CMYK'):
            image = image.convert('RGB')
        # Sanitizace názvu souboru a přidání indexu pro jedinečnost
        sanitized_name = sanitize_filename(product_name)
        sanitized_product_no = sanitize_filename(product_no)
        image_filename = f"{sanitized_name}_{sanitized_product_no}_{index}.jpg"
        image_path = str(Path(images_folder) / image_filename)
        # Uložení obrázku jako JPEG s kvalitou 85
        image.save(image_path, format='JPEG', quality=85)
        return image_filename  # noqa: TRY300 - guarded on purpose, the happy path stays in the try
    except Exception as e:
        print(f"Error downloading image at {img_url}: {e}")
        return None


def create_workbook(headers):
    """Vytvoření Excelového workbooku s danými hlavičkami."""
    wb = Workbook()
    ws = wb.active
    ws.append(headers)
    return wb


def autofit_column_widths(worksheet):
    """Automatické přizpůsobení šířky sloupců na základě obsahu."""
    for col in worksheet.columns:
        max_length = 0
        column = col[0].column_letter  # Získání názvu sloupce
        for cell in col:
            if cell.value is not None:
                max_length = max(max_length, len(str(cell.value)))
        adjusted_width = (max_length + 2)  # Přidání malého prostoru navíc
        worksheet.column_dimensions[column].width = adjusted_width


def save_workbook(wb, save_path, base_filename, part):
    """Uložení workbooku na disk."""
    autofit_column_widths(wb.active)
    xlsx_filename = f"{base_filename}_part_{part}.xlsx"
    full_path = str(Path(save_path) / xlsx_filename)
    wb.save(full_path)
    print(f"Saved part {part} to {full_path}")


def parse_arguments():
    """
    Parsování command line argumentů.

    Returns:
        Parsed arguments
    """
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
    """
    Hlavní funkce pro spuštění univerzálního Shoptet scraperu.

    Podporuje automatické objevování dostupných obchodů a typů feedů.
    """
    # Parsování argumentů
    args = parse_arguments()

    # Nastavení loggingu
    logger = setup_logging()

    # Vytvoření output manageru
    output_path = args.output if args.output else None
    output_manager = OutputManager(output_path)
    session_info = output_manager.get_session_info()
    logger.info(f" Session: {session_info['session_path']}")

    if args.command == 'eshop':
        if args.store:
            # Přímé spuštění pro konkrétní URL obchodu
            discovery = ShoptetStoreDiscovery(logger)
            store_config = discovery.discover_store_from_url(args.store)

            if not store_config:
                logger.error(f" Nepodařilo se objevit Shoptet obchod na URL: {args.store}")
                return

            logger.info(f" Spouštění scraperu pro obchod: {store_config['name']} (Universal feed)")
            run_scraper_for_store(store_config, 'universal', output_manager, logger, args.all_images)
        else:
            # Interaktivní režim - zadání URL obchodů
            print("\n=== Univerzální Shoptet Scraper ===")
            print("Zadejte URL Shoptet obchodů (jeden na řádek, prázdný řádek pro ukončení):")

            url_list = []
            while True:
                url = input("URL: ").strip()
                if not url:
                    break
                url_list.append(url)

            if not url_list:
                logger.info(" Žádné URL nezadány")
                return

            # Objevení obchodů
            discovery = ShoptetStoreDiscovery(logger)
            discovered_stores = discovery.discover_stores_from_list(url_list)

            if not discovered_stores:
                logger.error(" Žádné Shoptet obchody nenalezeny")
                return

            # Kontrola dostupnosti feedů
            available_stores = check_xml_feeds_for_stores(discovered_stores, logger)

            if not available_stores:
                logger.error(" Žádné dostupné feedy nenalezeny")
                return

            # Výběr obchodu a feedu
            selection = show_available_stores(available_stores)

            if selection:
                store_key, feed_type = selection
                store_data = available_stores[store_key]
                store_config = store_data['store_config']
                run_scraper_for_store(store_config, feed_type, output_manager, logger, args.all_images)


# Staré funkce odstraněny - nahrazeny novou architekturou


def run_scraper_for_store(store_config: dict[str, str], feed_type: str,
                          output_manager: OutputManager, logger: logging.Logger,
                          scrape_all_images: bool = False):
    """
    Spuštění scraperu pro konkrétní obchod a typ feedu.

    Args:
        store_config: Konfigurace obchodu
        feed_type: Typ XML feedu z XML_FEED_TYPES
        output_manager: Manager pro výstupní složky
        logger: Logger instance
        scrape_all_images: Zda stahovat všechny obrázky z galerie produktů
    """
    if feed_type not in XML_FEED_TYPES:
        logger.error(f" Neznámý typ feedu: {feed_type}")
        return

    feed_config = XML_FEED_TYPES[feed_type]
    mode_text = "všechny obrázky z galerií" if scrape_all_images else "pouze hlavní obrázky"
    logger.info(f" Spouštění scraperu pro obchod: {store_config['name']}, feed: {feed_config['name']}, režim: {mode_text}")

    # Vytvoření scraperu
    scraper = ShoptetScraper(store_config, feed_type, logger, scrape_all_images)

    # Vytvoření složek pro výstup
    store_path = output_manager.get_store_path(store_config['name'])
    images_folder = output_manager.get_images_path(store_path)

    # Stažení XML
    xml_path = scraper.download_xml(store_path)
    if not xml_path:
        logger.error(" Nepodařilo se stáhnout XML feed")
        return

    # Parsování XML
    root = scraper.parse_xml(xml_path)
    if root is None:
        logger.error(" Nepodařilo se parsovat XML")
        return

    # Extrakce dat obrázků
    image_data = scraper.extract_image_data(root)
    if not image_data:
        logger.warning(" Žádné obrázky k stažení")
        return

    # Zpracování produktů
    scraper.process_products(root, image_data, images_folder, store_path)

    # Smazání XML souboru
    scraper.cleanup_xml(xml_path)

    logger.info(" Scraping dokončen úspěšně!")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n Ukončování programu...")
        sys.exit(0)
    except Exception as e:
        print(f" Neočekávaná chyba: {e}")
        sys.exit(1)
