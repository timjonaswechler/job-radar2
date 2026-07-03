import sqlite3
from pathlib import Path

import pandas as pd

INPUT_PATH = Path("src-tauri/geo/DE.txt")
OUTPUT_PATH = Path("src-tauri/resources/geo_seed.sqlite")

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
    "or_shape",
]


def load_places() -> pd.DataFrame:
    df = pd.read_csv(
        INPUT_PATH,
        sep="\t",
        header=None,
        names=COLUMNS,
    )

    # Keep the newer German state codes (BW, BY, ...) and rows without a state code.
    # The source file also contains older numeric state codes (01, 02, ...), which
    # would duplicate the same places.
    df = df[df["state_code"].str.isalpha() | df["state_code"].isna()]

    # Combine duplicate postal-code rows for the same place within the same state.
    # Example: Esslingen am Neckar has several postal codes; keep one row for
    # Esslingen am Neckar + Baden-Württemberg and use the average coordinates.
    df = df.groupby(
        ["country_code", "place_name", "state_name", "state_code"],
        as_index=False,
        dropna=False,
    ).agg(
        {
            "county_name": "first",
            "latitude": "mean",
            "longitude": "mean",
        }
    )

    return df.drop(columns=["state_code"])


def write_sqlite(df: pd.DataFrame) -> None:
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    if OUTPUT_PATH.exists():
        OUTPUT_PATH.unlink()

    rows = [
        (
            row.country_code,  # pyright: ignore[reportAttributeAccessIssue]
            row.place_name,  # pyright: ignore[reportAttributeAccessIssue]
            row.state_name if pd.notna(row.state_name) else None,  # pyright: ignore[reportAttributeAccessIssue]
            row.county_name if pd.notna(row.county_name) else None,  # pyright: ignore[reportAttributeAccessIssue]
            float(row.latitude),  # pyright: ignore[reportAttributeAccessIssue]
            float(row.longitude),  # pyright: ignore[reportAttributeAccessIssue]
        )
        for row in df.itertuples(index=False)
    ]

    with sqlite3.connect(OUTPUT_PATH) as conn:
        conn.execute("PRAGMA journal_mode = OFF")
        conn.execute("PRAGMA synchronous = OFF")
        conn.execute(
            """
            CREATE TABLE geo_places (
                id INTEGER PRIMARY KEY,
                country_code TEXT NOT NULL,
                place_name TEXT NOT NULL,
                state_name TEXT,
                county_name TEXT,
                latitude REAL NOT NULL,
                longitude REAL NOT NULL,
                UNIQUE(country_code, place_name, state_name)
            )
            """
        )
        conn.executemany(
            """
            INSERT INTO geo_places (
                country_code,
                place_name,
                state_name,
                county_name,
                latitude,
                longitude
            ) VALUES (?, ?, ?, ?, ?, ?)
            """,
            rows,
        )
        conn.execute("CREATE INDEX idx_geo_places_name ON geo_places(place_name)")
        conn.execute("CREATE INDEX idx_geo_places_state ON geo_places(state_name)")


def main() -> None:
    df = load_places()

    write_sqlite(df)
    print(f"Wrote {len(df)} places to {OUTPUT_PATH}")


if __name__ == "__main__":
    main()
