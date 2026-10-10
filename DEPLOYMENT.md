# Hosting

The entire app runs as one Rust server. It serves the browser frontend, handles planner APIs, stores data in SQLite, and polls weather. The repository and automated checks live in GitHub; the server runs on a host that supports Rust or Docker and a persistent writable volume. GitHub Pages alone cannot run this server.

## Included configuration

- `.github/workflows/ci.yml` checks formatting and frontend syntax, tests the backend, builds a release, builds the Docker image, and checks container startup with a mounted data volume.
- `Dockerfile` packages the Rust API and frontend together.
- `render.yaml` is an optional Render Blueprint for deploying directly from your GitHub repository. It configures one server and a 1 GB disk mounted at `/app/data`, then deploys changes only when GitHub checks pass.

The Render configuration uses a paid `starter` service because persistent disks are not available on free web services. Review Render's current service and disk pricing before creating the Blueprint. Nothing is provisioned just by writing these files.

## GitHub Pages frontend + local Rust backend (current setup)

The frontend is deployed by `.github/workflows/pages.yml` to:

https://aryanndharamsey10.github.io/snowboardingapp/

GitHub Pages is configured to use GitHub Actions. Only `static/` is published; the database and Rust files stay off Pages. Routes use URL fragments (for example `#/p/PLANNER_ID`), so opening or refreshing an invitation works without server rewrite rules.

The frontend's public backend origin is in `static/config.js`. An empty value uses the Rust server's origin for local development. A public HTTPS tunnel address is required for friends on other networks; their browsers cannot reach your `localhost`. The backend allows cross-origin JSON and bearer-token requests only from the `FRONTEND_ORIGIN` configured at startup. Owner/member tokens remain in browser storage and stay out of invite links.

### Start the local backend

Run from this repository in a terminal:

```bash
CARGO_HOME="$PWD/.cargo-local" cargo build --locked
FRONTEND_ORIGIN=https://aryanndharamsey10.github.io caffeinate -i ./target/debug/weekend-club
```

`caffeinate` keeps macOS awake while the server runs. SQLite trips remain in `data/weekend-club.sqlite`. Keep the server running for friends to create planners, join, or fetch weather.

### Start or restart the public tunnel

In a second terminal, use the installed `cloudflared` command (or the local binary under `.local-hosting/` if installed there):

```bash
cloudflared tunnel --url http://127.0.0.1:3000
```

Copy the `https://...trycloudflare.com` origin printed by the tunnel. In a third terminal:

```bash
python3 scripts/set-backend.py https://YOUR-TUNNEL.trycloudflare.com
git add static/config.js
git commit -m "Update local backend tunnel address"
git push
```

Wait for the Pages workflow to finish, then use the Pages URL to create/share planners. Your frontend URL and invite links stay the same when the backend tunnel address changes. Existing trips stay in your SQLite database. Owner access created on localhost is separate from owner access on Pages because browser storage is scoped by origin.

Quick tunnels use a new temporary address on restart. Update the configuration and push after each restart; a named tunnel with a domain can provide a stable backend address later. Stop the local server/tunnel with Ctrl+C. The frontend remains online when the backend is stopped, with an offline notice and a browsable mountain catalog.

### Check deployment

```bash
python3 scripts/github-pages.py
```

This reads the Pages deployment status using your existing Git credential helper without printing credentials. If configuring a new repository manually, choose **Settings > Pages > Source > GitHub Actions**.

## Deploy the whole app from GitHub

1. Connect your GitHub account/repository to Render.
2. Create a Blueprint using this repository's `render.yaml`.
3. Review the paid web service and disk before confirming creation.
4. Wait for the build and health check to pass, then open the returned HTTPS URL.
5. Create a planner at that address. Its invite links will use the public URL automatically.

Keep `DATABASE_PATH` under the disk's mount path. Deploy one instance for SQLite. Data outside the persistent disk is erased on redeploy. The container entrypoint prepares ownership of a new data volume and the SQLite files, then runs the server as `appuser` (UID 10001). Back up the database with SQLite's backup mechanism, which accounts for its WAL file.

The same container can run on another Docker host. Set `HOST=0.0.0.0`, use the host's expected `PORT`, persist `/app/data`, and put HTTPS in front of it.

## Verification after deployment

Open `/api/health`, create a planner, open its link in another browser, join a selected trip, and restart the server. Check that the planner and availability remain present. Confirm that GitHub checks block deployments when a test fails.

References: [GitHub Pages](https://docs.github.com/en/pages/getting-started-with-github-pages/what-is-github-pages), [Render Blueprint specification](https://render.com/docs/blueprint-spec), [Render persistent disks](https://render.com/docs/disks).
