import re
import sqlite3
import unicodedata
from pathlib import Path

import pandas as pd

INPUT_DIR = Path("src-tauri/geo")
COUNTRY_SOURCE_NAMES = {
    "DE": ("DE",),
    "FR": ("FR",),
    # GB_full contains full Royal Mail postcodes (~1.8M rows). Use it to derive
    # place names and compact postcode-sector/outward-code lookup rows.
    "GB": ("GB_full", "GB"),
}
COUNTRY_CODES = tuple(COUNTRY_SOURCE_NAMES)
GB_POSTAL_CODE_COUNTRIES = {"GB"}
REGULAR_POSTAL_CODE_PATTERNS = {
    "DE": r"\d{5}",
    "FR": r"\d{5}",
}
FR_METROPOLITAN_REGION_CODES = {
    "11",  # Île-de-France
    "24",  # Centre-Val de Loire
    "27",  # Bourgogne-Franche-Comté
    "28",  # Normandie
    "32",  # Hauts-de-France
    "44",  # Grand Est
    "52",  # Pays de la Loire
    "53",  # Bretagne
    "75",  # Nouvelle-Aquitaine
    "76",  # Occitanie
    "84",  # Auvergne-Rhône-Alpes
    "93",  # Provence-Alpes-Côte d'Azur
    "94",  # Corse
}
GB_UK_STATE_CODES = {
    "ENG",  # England
    "SCT",  # Scotland
    "WLS",  # Wales
    "NIR",  # Northern Ireland
}
OUTPUT_PATH = Path("src-tauri/resources/geo_loc.sqlite")


def input_path_for(country_code: str) -> Path:
    for source_name in COUNTRY_SOURCE_NAMES[country_code]:
        for suffix in (".txt", ".txt.gz"):
            input_path = INPUT_DIR / f"{source_name}{suffix}"
            if input_path.exists():
                return input_path
    return INPUT_DIR / f"{COUNTRY_SOURCE_NAMES[country_code][0]}.txt"


INPUT_PATHS = [input_path_for(country_code) for country_code in COUNTRY_CODES]

# GeoNames postal-code export columns:
# https://download.geonames.org/export/zip/readme.txt
COLUMNS = [
    "country_code",
    "postal_code",
    "place_name",
    "state_name",
    "state_code",
    "admin_name2",
    "admin_code2",
    "county_name",
    "admin_code3",
    "latitude",
    "longitude",
    "accuracy",
]

GROUP_COLUMNS = [
    "country_code",
    "place_name",
    "state_name",
    "state_code",
    "county_name",
]

GERMAN_TRANSLITERATION = str.maketrans(
    {
        "ä": "ae",
        "ö": "oe",
        "ü": "ue",
        "Ä": "ae",
        "Ö": "oe",
        "Ü": "ue",
        "ß": "ss",
    }
)


def normalize_key(value: str) -> str:
    """Normalize user/search text into the same lookup key used by the backend."""
    value = value.strip().translate(GERMAN_TRANSLITERATION).casefold()
    value = (
        unicodedata.normalize("NFKD", value).encode("ascii", "ignore").decode("ascii")
    )
    value = re.sub(r"[^a-z0-9]+", " ", value)
    return re.sub(r"\s+", " ", value).strip()


def simple_ascii_key(value: str) -> str:
    """Alternative umlaut handling: München -> munchen, useful for user input."""
    value = value.strip().casefold()
    value = (
        unicodedata.normalize("NFKD", value).encode("ascii", "ignore").decode("ascii")
    )
    value = re.sub(r"[^a-z0-9]+", " ", value)
    return re.sub(r"\s+", " ", value).strip()


def normalized_key_variants(value: object) -> set[str]:
    if pd.isna(value):
        return set()

    value = str(value)
    variants = {normalize_key(value), simple_ascii_key(value)}
    return {variant for variant in variants if variant}


def read_postal_code_rows(input_path: Path) -> pd.DataFrame:
    return pd.read_csv(
        input_path,
        sep="\t",
        header=None,
        names=COLUMNS,
        dtype={
            "country_code": "string",
            "postal_code": "string",
            "place_name": "string",
            "state_name": "string",
            "state_code": "string",
            "admin_name2": "string",
            "admin_code2": "string",
            "county_name": "string",
            "admin_code3": "string",
            "accuracy": "string",
        },
    )


def load_postal_code_rows() -> pd.DataFrame:
    missing_paths = [
        input_path for input_path in INPUT_PATHS if not input_path.exists()
    ]
    if missing_paths:
        missing = ", ".join(str(input_path) for input_path in missing_paths)
        raise FileNotFoundError(f"Missing GeoNames postal-code input(s): {missing}")

    df = pd.concat(
        [read_postal_code_rows(input_path) for input_path in INPUT_PATHS],
        ignore_index=True,
    )
    df = df.dropna(
        subset=["country_code", "postal_code", "place_name", "latitude", "longitude"]
    )

    # Keep the newer German state codes (BW, BY, ...) and rows without a state code.
    # DE.txt also contains older numeric state codes (01, 02, ...), which would
    # duplicate the same places. Other countries, e.g. FR.txt, use numeric state
    # codes as their canonical region codes and must not be filtered this way.
    has_current_german_state_code = df["state_code"].str.isalpha().fillna(True)
    df = df[(df["country_code"] != "DE") | has_current_german_state_code]

    # For countries with regular numeric postal codes, drop special-delivery /
    # organization codes. French non-numeric entries such as CEDEX, SP,
    # CITYSSIMO, or AIR are not city/town/village postal codes.
    for country_code, pattern in REGULAR_POSTAL_CODE_PATTERNS.items():
        has_regular_postal_code = df["postal_code"].str.fullmatch(pattern).fillna(False)
        df = df[(df["country_code"] != country_code) | has_regular_postal_code]

    # Keep mainland France plus Corse. This drops special rows such as Clipperton
    # Island (FR 98799) and guards against future non-metropolitan FR rows in the
    # postal-code export.
    has_metropolitan_fr_region_code = df["state_code"].isin(
        FR_METROPOLITAN_REGION_CODES
    )
    df = df[(df["country_code"] != "FR") | has_metropolitan_fr_region_code]

    # Keep the UK countries from GB_full, but drop Crown Dependencies such as
    # Guernsey/Jersey/Isle of Man that GeoNames also publishes under GB.
    has_uk_state_code = df["state_code"].isin(GB_UK_STATE_CODES)
    df = df[(df["country_code"] != "GB") | has_uk_state_code]

    return df


def build_places(postal_code_rows: pd.DataFrame) -> pd.DataFrame:
    # Combine postal-code rows into one canonical place row, but keep county_name
    # to avoid merging different places that share a name in the same state.
    places = postal_code_rows.groupby(
        GROUP_COLUMNS,
        as_index=False,
        dropna=False,
    ).agg(
        latitude=("latitude", "mean"),
        longitude=("longitude", "mean"),
        postal_code_count=("postal_code", "nunique"),
    )

    places.insert(0, "id", range(1, len(places) + 1))
    return places


def build_gb_postal_code_rows(rows_with_place_ids: pd.DataFrame) -> pd.DataFrame:
    gb_rows = rows_with_place_ids[
        rows_with_place_ids["country_code"].isin(GB_POSTAL_CODE_COUNTRIES)
    ].copy()
    if gb_rows.empty:
        return pd.DataFrame(columns=["postal_code", "id", "latitude", "longitude"])

    compact_postal_codes = gb_rows["postal_code"].str.upper().str.replace(
        r"[^A-Z0-9]",
        "",
        regex=True,
    )
    outward_codes = compact_postal_codes.str[:-3]
    inward_codes = compact_postal_codes.str[-3:]

    has_full_uk_postcode = (
        compact_postal_codes.str.len().between(5, 7)
        & outward_codes.str.fullmatch(r"[A-Z][A-Z0-9]{1,3}")
        & inward_codes.str.fullmatch(r"\d[A-Z]{2}")
    ).fillna(False)

    gb_rows = gb_rows[has_full_uk_postcode].copy()
    outward_codes = outward_codes[has_full_uk_postcode]
    inward_codes = inward_codes[has_full_uk_postcode]

    sectors = gb_rows[["id", "latitude", "longitude"]].copy()
    sectors["postal_code"] = (outward_codes + " " + inward_codes.str[0]).str.lower()

    outwards = gb_rows[["id", "latitude", "longitude"]].copy()
    outwards["postal_code"] = outward_codes.str.lower()

    return pd.concat([sectors, outwards], ignore_index=True)[
        ["postal_code", "id", "latitude", "longitude"]
    ]


def build_postal_codes(
    postal_code_rows: pd.DataFrame, places: pd.DataFrame
) -> pd.DataFrame:
    rows_with_place_ids = postal_code_rows.merge(
        places[["id", *GROUP_COLUMNS]],
        on=GROUP_COLUMNS,
        how="inner",
        validate="many_to_one",
    )

    regular_postal_code_rows = rows_with_place_ids[
        ~rows_with_place_ids["country_code"].isin(GB_POSTAL_CODE_COUNTRIES)
    ][["postal_code", "id", "latitude", "longitude"]]
    gb_postal_code_rows = build_gb_postal_code_rows(rows_with_place_ids)

    return pd.concat(
        [regular_postal_code_rows, gb_postal_code_rows],
        ignore_index=True,
    ).groupby(
        ["postal_code", "id"],
        as_index=False,
        dropna=False,
    ).agg(
        latitude=("latitude", "mean"),
        longitude=("longitude", "mean"),
    )


def build_place_keys(places: pd.DataFrame) -> list[tuple[str, int, str]]:
    keys: set[tuple[str, int, str]] = set()

    # Keep this table deliberately small: it only contains normalized place-name
    # variants. Postal codes are searchable through geo_postal_codes, which keeps
    # their more precise postal-code coordinates instead of mapping them through
    # the aggregated place coordinate.
    for row in places.itertuples(index=False):
        for key in normalized_key_variants(row.place_name):
            keys.add((key, int(row.id), "place_name"))

    return sorted(keys)


def nullable_string(value: object) -> str | None:
    if pd.isna(value):
        return None
    return str(value)


def write_sqlite(
    places: pd.DataFrame,
    postal_codes: pd.DataFrame,
    place_keys: list[tuple[str, int, str]],
) -> None:
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    if OUTPUT_PATH.exists():
        OUTPUT_PATH.unlink()

    place_rows = [
        (
            int(row.id),
            nullable_string(row.country_code),
            nullable_string(row.place_name),
            nullable_string(row.state_name),
            nullable_string(row.state_code),
            nullable_string(row.county_name),
            float(row.latitude),
            float(row.longitude),
            int(row.postal_code_count),
        )
        for row in places.itertuples(index=False)
    ]

    postal_code_rows = [
        (
            str(row.postal_code),
            int(row.id),
            float(row.latitude),
            float(row.longitude),
        )
        for row in postal_codes.itertuples(index=False)
    ]

    with sqlite3.connect(OUTPUT_PATH) as conn:
        conn.execute("PRAGMA journal_mode = OFF")
        conn.execute("PRAGMA synchronous = OFF")
        conn.execute("PRAGMA foreign_keys = ON")
        conn.execute(
            """
            CREATE TABLE geo_places (
                id INTEGER PRIMARY KEY,
                country_code TEXT NOT NULL,
                place_name TEXT NOT NULL,
                state_name TEXT,
                state_code TEXT,
                county_name TEXT,
                latitude REAL NOT NULL,
                longitude REAL NOT NULL,
                postal_code_count INTEGER NOT NULL,
                UNIQUE(country_code, place_name, state_name, state_code, county_name)
            )
            """
        )
        conn.execute(
            """
            CREATE TABLE geo_postal_codes (
                postal_code TEXT NOT NULL,
                place_id INTEGER NOT NULL REFERENCES geo_places(id),
                latitude REAL NOT NULL,
                longitude REAL NOT NULL,
                PRIMARY KEY(postal_code, place_id)
            ) WITHOUT ROWID
            """
        )
        conn.execute(
            """
            CREATE TABLE geo_place_keys (
                key TEXT NOT NULL,
                place_id INTEGER NOT NULL REFERENCES geo_places(id),
                kind TEXT NOT NULL,
                PRIMARY KEY(key, place_id, kind)
            ) WITHOUT ROWID
            """
        )

        conn.executemany(
            """
            INSERT INTO geo_places (
                id,
                country_code,
                place_name,
                state_name,
                state_code,
                county_name,
                latitude,
                longitude,
                postal_code_count
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            place_rows,
        )
        conn.executemany(
            """
            INSERT INTO geo_postal_codes (
                postal_code,
                place_id,
                latitude,
                longitude
            ) VALUES (?, ?, ?, ?)
            """,
            postal_code_rows,
        )
        conn.executemany(
            """
            INSERT INTO geo_place_keys (
                key,
                place_id,
                kind
            ) VALUES (?, ?, ?)
            """,
            place_keys,
        )

        # The primary keys already cover the resolver's lookup patterns:
        # geo_postal_codes(postal_code, place_id) and geo_place_keys(key, place_id, kind).
        # Avoid duplicate indexes to keep the bundled seed database small.
        conn.commit()

        conn.execute("ANALYZE")
        conn.commit()
        conn.execute("VACUUM")
        conn.execute("PRAGMA optimize")


def main() -> None:
    postal_code_rows = load_postal_code_rows()
    places = build_places(postal_code_rows)
    postal_codes = build_postal_codes(postal_code_rows, places)
    place_keys = build_place_keys(places)

    write_sqlite(places, postal_codes, place_keys)
    print(
        f"Wrote {len(places)} places, {len(postal_codes)} postal-code mappings, "
        f"and {len(place_keys)} lookup keys to {OUTPUT_PATH}"
    )


if __name__ == "__main__":
    main()
