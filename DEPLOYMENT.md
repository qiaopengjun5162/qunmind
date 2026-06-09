# Deployment

QunMind is configured with a local TOML file. Do not bake `config.toml` into the
image or commit it to git.

## Docker Compose

Prepare a deployment config:

```bash
cp config.docker.example.toml config.toml
$EDITOR config.toml
```

Make sure the database URL points at the Compose PostgreSQL service:

```toml
[storage]
database_url = "postgres://qunmind:qunmind@postgres:5432/qunmind"
```

Start the stack:

```bash
docker compose up -d --build
docker compose logs -f qunmind
```

Stop the stack:

```bash
docker compose down
```

Keep PostgreSQL data:

```bash
docker compose down
docker compose up -d
```

Remove PostgreSQL data only when you intentionally want to delete local runtime
state:

```bash
docker compose down -v
```

## Channel Notes

The Compose file is most suitable for server-side WeCom internal group bots,
PostgreSQL persistence, daily reports, and public-source fallback jobs.

The wx-cli channel usually depends on a host WeChat session and host-local CLI
or daemon access. Validate wx-cli natively first with:

```bash
cargo run -- wx-cli doctor
cargo run -- wx-cli capture --output wx-output.json
cargo run -- wx-cli doctor --input wx-output.json
```

If a future wx-cli daemon exposes a stable socket or HTTP endpoint, mount that
socket or configure that endpoint explicitly before running the channel in a
container.

## Release Images

On `v*` tags, GitHub Actions creates a GitHub Release and publishes a container
image to GitHub Container Registry:

```text
ghcr.io/qiaopengjun5162/qunmind:<tag>
ghcr.io/qiaopengjun5162/qunmind:latest
```

For a server using the registry image, update the `qunmind.image` field in
`docker-compose.yml`, then run:

```bash
docker compose pull qunmind
docker compose up -d
```

## Production Checklist

- Use a secret `config.toml` stored outside git.
- Rotate API keys if `config.toml` is ever exposed.
- Keep PostgreSQL data on a persistent volume.
- Run `docker compose config` before the first deploy.
- Run wx-cli diagnostics natively before enabling any real wx-cli send path.
- Keep the first real WeChat replay at `--limit 1` and use `--no-send` first.
