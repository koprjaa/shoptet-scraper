# Univerzální Shoptet Scraper

Univerzální scraper pro stahování XML feedů a produktů z Shoptet obchodů. Automaticky objevuje dostupné obchody a podporuje různé typy XML feedů.

## Funkce

- 🔍 **Automatické objevování** - Dynamicky objevuje Shoptet obchody z URL
- 📊 **Více typů feedů** - Podporuje Universal, Google Shopping, Heureka a další XML feedy
- 🖼️ **Stahování obrázků** - Paralelní stahování a konverze obrázků produktů
- 📁 **Strukturovaný výstup** - Organizované složky s číslováním session
- 📈 **Excel export** - Export dat do Excel souborů s automatickým formátováním
- 🔄 **Retry mechanismus** - Robustní zpracování chyb a opakování požadavků
- 📝 **Detailní logování** - Kompletní logování všech operací

## Podporované XML feedy

- **Universal XML Feed** (`/universal.xml`) - Univerzální feed pro všechny produkty
- **Google Shopping XML Feed** (`/google.xml`) - Feed pro Google Shopping
- **Heureka XML Feed** (`/heureka.xml`) - Feed pro srovnávač cen Heureka
- **Zboží.cz XML Feed** (`/zbozi.xml`) - Feed pro srovnávač cen Zboží.cz
- **Shoptet Dodavatelský XML Feed** (`/supplier.xml`) - Feed pro dodavatele

## Instalace

### Požadavky

- Python 3.7+
- Požadované balíčky (viz requirements.txt)

### Instalace závislostí

```bash
pip install -r requirements.txt
```

## Použití

### Interaktivní režim

```bash
python main.py eshop
```

Program vás vyzve k zadání URL Shoptet obchodů. Zadejte jeden URL na řádek a prázdný řádek pro ukončení zadávání.

### Přímé spuštění

```bash
python main.py eshop --store https://example.shoptet.cz
```

### Vlastní výstupní složka

```bash
python main.py eshop --store https://example.shoptet.cz --output /path/to/output
```

### Nápověda

```bash
python main.py --help
```

## Struktura výstupu

```
./output/
├── 2024-01-15_001/
│   ├── StoreName1/
│   │   ├── images/
│   │   │   ├── product1_001_1.jpg
│   │   │   └── product2_001_2.jpg
│   │   ├── feed_storename1_universal_15-01-2024_part_1.xlsx
│   │   └── feed_storename1_universal_15-01-2024_part_2.xlsx
│   └── StoreName2/
│       ├── images/
│       └── feed_storename2_google_15-01-2024_part_1.xlsx
└── 2024-01-15_002/
    └── ...
```

## Architektura

### Hlavní třídy

- **`ShoptetStoreDiscovery`** - Objevování Shoptet obchodů z URL
- **`ShoptetScraper`** - Hlavní scraper pro konkrétní obchod a typ feedu
- **`XMLFeedParser`** - Abstraktní třída pro parsování XML feedů
- **`OutputManager`** - Správa výstupních složek s číslováním

### Typy parserů

- **`UniversalXMLParser`** - Parser pro Universal XML feed
- **`GoogleXMLParser`** - Parser pro Google Shopping XML feed
- **`HeurekaXMLParser`** - Parser pro Heureka XML feed

## Konfigurace

### Logování

Logy se ukládají do `shoptet_scraper.log` a zobrazují se v konzoli.

### Výstupní složky

Výchozí výstupní složka: `./output` (v kořenové složce projektu)

Struktura číslování: `YYYY-MM-DD_XXX` (datum_session)

## Příklady použití

### Scraping jednoho obchodu

```bash
python main.py eshop --store https://www.gardners-eshop.cz
```

### Scraping více obchodů

```bash
python main.py eshop
# Zadejte URL obchodů:
# https://www.gardners-eshop.cz
# https://www.example-shop.shoptet.cz
# (prázdný řádek pro ukončení)
```

### Vlastní výstupní složka

```bash
python main.py eshop --store https://www.gardners-eshop.cz --output /Users/jan/Documents/ScrapedData
```

## Řešení problémů

### Chyby při stahování

- Zkontrolujte internetové připojení
- Ověřte, že URL obchodu je správné
- Zkontrolujte, zda obchod má dostupné XML feedy

### Chyby při parsování

- Ověřte, že XML feed má správný formát
- Zkontrolujte logy pro detailní chybové zprávy

### Problémy s obrázky

- Ověřte dostupnost obrázků na serveru
- Zkontrolujte oprávnění k zápisu do výstupní složky

## Přispívání

1. Forkněte repository
2. Vytvořte feature branch (`git checkout -b feature/AmazingFeature`)
3. Commitněte změny (`git commit -m 'Add some AmazingFeature'`)
4. Pushněte do branch (`git push origin feature/AmazingFeature`)
5. Otevřete Pull Request

## Licence

Tento projekt je licencován pod MIT licencí - viz soubor [LICENSE](LICENSE) pro detaily.

## Podpora

Pokud narazíte na problémy nebo máte návrhy na vylepšení, vytvořte issue v GitHub repository.
