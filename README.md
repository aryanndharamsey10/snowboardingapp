# Weekend — Ski trip planner

A ski weekend planner with a Rust backend, SQLite storage, and a responsive browser frontend. One person owns each planner. Friends use a shared link to choose availability only from that owner's selected mountains and weekends.

## Run locally

```bash
CARGO_HOME="$PWD/.cargo-local" cargo run --locked
```

Open **http://localhost:3000**. Rust serves the frontend and API from the same address. No Node installation, frontend build, or API key is needed for development. The first build downloads the Rust dependencies. `CARGO_HOME` keeps the dependency cache inside this workspace; omit it if you prefer your normal Cargo cache.

1. Start a planner and choose a username and planner name.
2. Pick up to 26 Saturday–Sunday weekends within the next year.
3. Assign mountains to each weekend, with up to 12 unique US Ikon destinations across the planner. Different weekends can have different mountains; "Use for every weekend" repeats a selection when desired.
4. Create the planner and copy its invite link.
5. Friends open the link, choose trips, and enter their own usernames to join.
6. The owner can edit the mountains, dates, and planner name. Removed options are also removed from friends' availability.

To try the friend experience on the same computer, open an invite link in an incognito window or another browser. The original browser recognizes the owner.

## Database and ownership

The database is SQLite, accessed through `rusqlite`. By default it lives at `data/weekend-club.sqlite`. It stores planners, participants, availability, and cached weather. WAL mode and a process-wide connection lock protect concurrent updates. Planner edits prune removed trip choices in a transaction.

This initial version uses random capability credentials instead of accounts/passwords. An owner credential is returned once during creation and saved in that browser's local storage. A participant credential lets that participant update their own availability. Neither credential appears in the public planner API or invite link. Usernames are unique within a planner, case insensitive. Someone else cannot overwrite an existing username without its credential.

Owner and participant access are tied to the original browser. Clearing local storage loses that access; there is currently no account recovery or cross-device sign-in. Everyone with an invite link can view the crew's usernames and availability and join under an unused username. There is no global username registration. Keep the SQLite file backed up.

## Weather

The Rust server calls [Open-Meteo's forecast API](https://open-meteo.com/en/docs) for daily snowfall, Fahrenheit temperatures, and maximum wind speed in mph, using resort coordinates and local time zones. Snowfall is normalized to inches based on the returned units. Forecasts are cached in SQLite for one hour. An open planner reloads weather every 15 minutes and group availability every 30 seconds.

Every future weekend gets planning weather data. Inside the 16-day forecast window, current model forecasts are used. Farther out, the server requests the previous five complete calendar years of ERA5 daily data from the [Historical Weather API](https://open-meteo.com/en/docs/historical-weather-api). For each Saturday and Sunday it averages valid historical records within seven calendar days of that date's month/day across those years, including wraparound at New Year and leap-day handling. Temperature and wind values are averages of the daily lows/highs and daily maximum wind; expected new snowfall is the sum of the two daily snowfall averages. Historical data is cached in SQLite for 30 days.

The UI explicitly labels **Seasonal estimate** and links to its method and sample years. These are approximate typical conditions, rather than predictions of a specific storm. They help compare dates and mountains. When dates approach, current forecasts automatically replace historical estimates. If only one forecast day is present, the other uses a historical estimate and the result is labeled **Forecast + seasonal estimate**. Provider failures use cached data when available; failed loads show a retry message instead of fabricated numbers.

A background job refreshes weather for actual mountain/weekend options hourly while the server runs. It refreshes near-term forecasts and maintains historical caches for future trips. Weather is modeled at representative resort coordinates, rather than measured snowpack or live lift/terrain status.

The 41 standard US destinations are based on the [official Ikon destination directory](https://www.ikonpass.com/en/destinations), checked October 9, 2026. Resort groups such as Aspen Snowmass, Big Bear, and Killington–Pico use a representative location. The directory is maintained in `static/catalog.json`; seasonal membership, pass restrictions, reservations, and opening dates should be checked directly with Ikon/the resort. Bonus mountains are not included in this initial catalog.

Open-Meteo's free endpoint is for noncommercial usage; use an appropriate paid endpoint before commercial operation. See [Open-Meteo pricing](https://open-meteo.com/en/pricing).

## Configuration

| Environment variable | Default | Purpose |
| --- | --- | --- |
| `HOST` | `127.0.0.1` | Bind address; use `0.0.0.0` for a container or LAN access |
| `PORT` | `3000` | HTTP port |
| `DATABASE_PATH` | `data/weekend-club.sqlite` | SQLite file, including a writable parent directory |
| `STATIC_DIR` | `static` | Frontend files |
| `RUST_LOG` | `weekend_club=info,tower_http=info` | Server logging |

## Checks

```bash
CARGO_HOME="$PWD/.cargo-local" cargo test --locked
cargo fmt --check
node --check static/app.js
python3 tests/smoke.py
```

The backend tests cover ownership, restricted shared-link choices, participant credentials, date validation, availability cleanup after owner edits, weather units/missing data, seasonal estimates, per-weekend mountain restrictions, database migration, and persistence across database restarts.

`tests/smoke.py` launches a temporary server and database to check actual HTTP requests. Run `cargo build` first. Its default mode uses deterministic historical fixtures for CI; `CHECK_LIVE_WEATHER=1 python3 tests/smoke.py` also checks real forecast and historical weather providers.

## Deployment

The share link uses the app's current origin. A localhost link works on the local computer; to invite remote friends, deploy the Rust server on a publicly reachable host with HTTPS and a persistent writable data volume. The included Dockerfile builds one server that serves both the frontend and API.

```bash
docker build -t weekend-club .
docker volume create weekend-club-data
docker run --rm -p 3000:3000 -v weekend-club-data:/app/data weekend-club
```

Run one app instance for SQLite. For multiple instances, replace SQLite with a shared database such as PostgreSQL. Accounts/recovery and request rate limits are appropriate next steps before broad public use. Sites hosting currently runs JavaScript Workers and cannot directly run this Rust/Axum server.

GitHub deployment configuration is included in `.github/workflows/ci.yml` and `render.yaml`. See [DEPLOYMENT.md](DEPLOYMENT.md) for pushing the repository and running the whole app from GitHub on a server with persistent storage.

## Project layout

```text
src/main.rs         Server configuration and background weather poller
src/lib.rs          Rust API, SQLite queries, weather integration
src/tests.rs        Backend behavior tests
src/climate.rs      Historical weather estimates and caching
static/index.html  App entry point
static/app.js      Welcome, setup, owner planner, and friend join flows
static/style.css   Responsive alpine design
static/catalog.json US Ikon destination catalog
static/assets/     Local landscape imagery and icon
```

Decorative mountain images are from Unsplash and do not depict the named resorts specifically.
