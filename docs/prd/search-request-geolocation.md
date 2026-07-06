# Search Request Geolocation

## Problem Statement

Job Radar already stores optional `locations` and `radiusKm` on a Search Request, but Search Runs currently apply matching primarily through include and exclude rules. Users expect a Search Request such as `Mainz` with a `30 km` radius to also find postings in nearby places such as Wiesbaden, even when those nearby places were not entered manually.

Source Profiles should continue to extract posting locations as ordinary strings. Search criteria such as preferred cities, postal codes, countries, and radius belong to the Search Request, not to Source Config or Source Profiles.

## Solution

Job Radar will resolve Search Request locations and posting candidate locations into coordinates, then apply local radius matching during Search Runs.

The first implementation will use a bundled GeoNames-derived SQLite seed database for Germany. The database is generated from the GeoNames postal-code export by `scripts/geo_seeding.py` and bundled as `src-tauri/resources/geo_seed.sqlite`.

The resolver will support two lookup classes:

1. **Postal-code lookup**: exact postal-code queries resolve through `geo_postal_codes` and use the postal-code coordinate.
2. **Place-name lookup**: normalized city/place queries resolve through `geo_place_keys` and `geo_places`, using the aggregated place coordinate.

Search Runs will use the resolver behind a small backend interface. The Search Run should not know whether a coordinate came from the seed database, a writable cache, or a future provider such as Nominatim.

## Goals

- Filter Search Run candidates by Search Request `locations` and `radiusKm`.
- Support common German city names and postal codes offline.
- Keep Source Profiles responsible only for extracting posting location strings.
- Keep Search Request geolocation matching generic and source-independent.
- Normalize user-entered and source-extracted location strings consistently.
- Make the seed database small enough to bundle and fast enough for local lookup.
- Leave room for a later Nominatim fallback/cache without making Search Run logic depend on Nominatim.

## Non-Goals

- Portal-specific location behavior in Source Profiles or source execution.
- Mapping Search Request criteria into ATS-specific remote query parameters.
- Polygon/boundary matching for municipalities, districts, or states.
- Full worldwide geocoding in the first slice.
- Autocomplete against online services on every keystroke.
- Choosing one representative postal code for a city.

## Data Model

The generated seed database contains these tables:

```sql
geo_places
geo_postal_codes
geo_place_keys
```

### `geo_places`

Canonical place rows aggregated from postal-code rows.

Important fields:

- `country_code`
- `place_name`
- `state_name`
- `state_code`
- `county_name`
- `latitude`
- `longitude`
- `postal_code_count`

Places are grouped by:

```txt
country_code + place_name + state_name + state_code + county_name
```

Keeping `county_name` prevents merging distinct places that share a name within the same state.

### `geo_postal_codes`

Exact postal-code mappings with postal-code-level coordinates.

Important behavior:

- A city does not choose one canonical postal code.
- Postal codes are lookup inputs, not the canonical city identity.
- `55116` may resolve to a more precise point than `Mainz`.
- `Mainz` resolves to the aggregated place coordinate.

### `geo_place_keys`

Normalized place-name lookup keys.

Examples:

```txt
Mainz    -> mainz
München  -> muenchen
München  -> munchen
```

Postal-code keys are intentionally not duplicated here. Postal-code queries should use `geo_postal_codes` so that they can use postal-code coordinates.

## Normalization

The seed script and backend resolver should use the same normalization semantics.

Required behavior:

- trim leading/trailing whitespace
- lowercase/casefold
- collapse whitespace
- remove or normalize punctuation to spaces
- support German transliteration:
  - `ä -> ae`
  - `ö -> oe`
  - `ü -> ue`
  - `ß -> ss`
- also support ASCII decomposition variants such as `München -> munchen`

Examples:

```txt
" München "              -> "muenchen", "munchen"
"Frankfurt am Main"      -> "frankfurt am main"
"Battenberg (Pfalz)"     -> "battenberg pfalz"
"Herxheim bei Landau/Pfalz" -> "herxheim bei landau pfalz"
```

## Search Run Matching Behavior

A posting candidate matches the Search Request location criteria when at least one candidate location resolves within range of at least one Search Request location.

Recommended first-slice behavior:

1. If the Search Request has no locations, skip geolocation filtering.
2. Resolve all Search Request locations before candidate filtering.
3. For each candidate, resolve each candidate location string.
4. A candidate matches when any resolved candidate point is within `radiusKm` of any resolved request point.
5. If `radiusKm` is not set, use exact normalized text match or a very small default radius only after an explicit product decision.
6. If a Search Request location cannot be resolved, the Search Run should fail validation or produce a clear diagnostic rather than silently ignoring that location.
7. If a candidate location cannot be resolved, that candidate should not match the location filter unless another candidate location resolves and matches.

Distance should be calculated with the Haversine formula.

## Ambiguity

Some place names are ambiguous, for example `Buchholz`, `Schönberg`, `Bergen`, or `Neustadt`.

First-slice behavior may use deterministic ranking:

1. exact postal-code match wins over place-name match
2. exact normalized place-name key
3. country-specific seed database, initially Germany
4. stable database order as a final fallback

The resolver should expose ambiguity internally so later UI or diagnostics can explain that a location was matched to one of several possible places.

Future improvements may add:

- explicit country or region criteria on Search Requests
- UI disambiguation
- population/ranking data from additional GeoNames files
- user-selected saved places

## Provider Strategy

First slice:

```txt
GeoNames postal-code export -> scripts/geo_seeding.py -> bundled geo_seed.sqlite
```

Future resolver chain:

```txt
writable geo cache
-> bundled geo seed database
-> optional Nominatim fallback
-> store Nominatim result in writable cache
```

Nominatim should remain optional and cached. The public Nominatim service must not be used for high-volume autocomplete or uncached bulk geocoding.

## User Stories

1. As a Job Radar user, I want `Mainz` with a `30 km` radius to find jobs in nearby places, so that I do not need to know every surrounding city.
2. As a Job Radar user, I want postal-code input such as `55116` to work, so that I can search around a more precise location.
3. As a Job Radar user, I want `München`, `Muenchen`, and `Munchen` to resolve consistently, so that spelling and keyboard limitations do not break matching.
4. As a Job Radar user, I want unknown Search Request locations to be reported clearly, so that I can correct the request.
5. As a developer, I want Source Profiles to keep returning plain location strings, so that geolocation remains independent of profile execution.
6. As a developer, I want the Search Run to depend on a geolocation resolver interface, so that seed data, cache data, and later Nominatim can be swapped without changing Search Run matching logic.

## Acceptance Criteria

- `scripts/geo_seeding.py` can regenerate `src-tauri/resources/geo_seed.sqlite` from the GeoNames Germany postal-code export.
- The generated database contains `geo_places`, `geo_postal_codes`, and `geo_place_keys`.
- `Mainz` resolves through place-name lookup.
- `55116` resolves through postal-code lookup and uses the postal-code coordinate.
- `München`, `Muenchen`, and `Munchen` resolve to München.
- A Search Request with `locations = ["Mainz"]` and `radiusKm = 30` matches a candidate located in `Wiesbaden`.
- The same Search Request does not match a candidate located in `Köln`.
- When multiple candidate locations exist, one in-radius resolved location is enough to match.
- If the Search Request has no locations, existing include/exclude rule matching behavior is preserved.
- Source Config and Source Profiles do not gain search criteria fields for geolocation.

## Implementation Notes

Potential backend module shape:

```txt
src-tauri/src/geo/
  mod.rs
  distance.rs
  normalization.rs
  seed.rs
  resolver.rs
```

Suggested interface shape:

```rust
pub struct GeoPoint {
    pub latitude: f64,
    pub longitude: f64,
}

pub struct ResolvedLocation {
    pub input: String,
    pub label: String,
    pub point: GeoPoint,
    pub source: GeoResolutionSource,
}

pub trait GeoResolver {
    fn resolve(&self, input: &str) -> Result<Vec<ResolvedLocation>, GeoResolutionError>;
}
```

The external Search Run-facing interface should stay small, for example:

```rust
matches_location_filter(
    request_locations,
    radius_km,
    candidate_locations,
) -> LocationMatchOutcome
```

That keeps geolocation logic local to the geo module and prevents Search Run code from spreading normalization, SQL lookup, ambiguity handling, and distance math across callers.

## Open Questions

- Should unresolved Search Request locations make an active Search Request invalid, fail the Search Run, or produce a warning and continue?
- What should happen when `radiusKm` is absent but locations are present?
- Should remote/hybrid postings match all location filters, no location filters, or require a separate remote-mode criterion?
- Should the first bundled seed include only Germany or multiple countries?
- Should raw GeoNames input files be committed, downloaded by script, or documented as external build inputs?
