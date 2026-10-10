# GitHub deployment

The entire app runs as one Rust server. It serves the browser frontend, handles planner APIs, stores data in SQLite, and polls weather. The repository and automated checks live in GitHub; the server runs on a host that supports Rust or Docker and a persistent writable volume. GitHub Pages alone cannot run this server.

## Included configuration

- `.github/workflows/ci.yml` checks formatting and frontend syntax, tests the backend, builds a release, builds the Docker image, and checks container startup with a mounted data volume.
- `Dockerfile` packages the Rust API and frontend together.
- `render.yaml` is an optional Render Blueprint for deploying directly from your GitHub repository. It configures one server and a 1 GB disk mounted at `/app/data`, then deploys changes only when GitHub checks pass.

The Render configuration uses a paid `starter` service because persistent disks are not available on free web services. Review Render's current service and disk pricing before creating the Blueprint. Nothing is provisioned just by writing these files.

## Push the app to GitHub

Create an empty GitHub repository, then run from this project directory, replacing `OWNER/REPOSITORY`:

```bash
git init -b main
git add .
git commit -m "Build Rust ski weekend planner"
git remote add origin https://github.com/OWNER/REPOSITORY.git
git push -u origin main
```

The dependency cache, build output, database, browser credentials, and local configuration are excluded by `.gitignore`. Do not commit database files or personal planner data.

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
