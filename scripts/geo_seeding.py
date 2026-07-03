import re
import sqlite3
import unicodedata
from pathlib import Path

import pandas as pd

INPUT_PATH = Path("src-tauri/geo/DE.txt")
OUTPUT_PATH = Path("src-tauri/resources/geo_seed.sqlite")

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
    value = unicodedata.normalize("NFKD", value).encode("ascii", "ignore").decode("ascii")
    value = re.sub(r"[^a-z0-9]+", " ", value)
    return re.sub(r"\s+", " ", value).strip()


def simple_ascii_key(value: str) -> str:
    """Alternative umlaut handling: München -> munchen, useful for user input."""
    value = value.strip().casefold()
    value = unicodedata.normalize("NFKD", value).encode("ascii", "ignore").decode("ascii")
    value = re.sub(r"[^a-z0-9]+", " ", value)
    return re.sub(r"\s+", " ", value).strip()


def normalized_key_variants(value: object) -> set[str]:
    if pd.isna(value):
        return set()

    value = str(value)
    variants = {normalize_key(value), simple_ascii_key(value)}
    return {variant for variant in variants if variant}


def load_postal_code_rows() -> pd.DataFrame:
    df = pd.read_csv(
        INPUT_PATH,
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

    df = df.dropna(subset=["country_code", "postal_code", "place_name", "latitude", "longitude"])

    # Keep the newer German state codes (BW, BY, ...) and rows without a state code.
    # The source file also contains older numeric state codes (01, 02, ...), which
    # would duplicate the same places.
    df = df[df["state_code"].str.isalpha().fillna(True)]

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


def build_postal_codes(postal_code_rows: pd.DataFrame, places: pd.DataFrame) -> pd.DataFrame:
    rows_with_place_ids = postal_code_rows.merge(
        places[["id", *GROUP_COLUMNS]],
        on=GROUP_COLUMNS,
        how="inner",
        validate="many_to_one",
    )

    return rows_with_place_ids.groupby(
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


def write_sqlite(places: pd.DataFrame, postal_codes: pd.DataFrame, place_keys: list[tuple[str, int, str]]) -> None:
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
            )
            """
        )
        conn.execute(
            """
            CREATE TABLE geo_place_keys (
                key TEXT NOT NULL,
                place_id INTEGER NOT NULL REFERENCES geo_places(id),
                kind TEXT NOT NULL,
                PRIMARY KEY(key, place_id, kind)
            )
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

        conn.execute("CREATE INDEX idx_geo_place_keys_key ON geo_place_keys(key)")
        conn.execute("CREATE INDEX idx_geo_postal_codes_postal_code ON geo_postal_codes(postal_code)")
        conn.execute("CREATE INDEX idx_geo_places_name_state ON geo_places(place_name, state_name)")
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
