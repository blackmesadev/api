# api

the REST API for Black Mesa's dashboard. handles Discord OAuth authentication, guild config management and more

## with Docker

start the container with:

```bash
docker run --rm \
    --name bm-api \
	-p 8080:8080 \
	-e DATABASE_URL="postgres://user:pass@host.docker.internal:5432/blackmesa" \
	-e REDIS_URI="redis://host.docker.internal:6379" \
	-e JWT_SECRET="replace_with_secure_secret" \
	-e DISCORD_CLIENT_ID="your_client_id" \
	-e DISCORD_CLIENT_SECRET="your_client_secret" \
	-e DISCORD_REDIRECT_URI="http://localhost:4173/oauth/discord" \
	-e DISCORD_BOT_TOKEN="your_bot_token" \
	-e OTLP_ENDPOINT="http://host.docker.internal:4318/v1/traces" \

```

## api spec

openapi spec is available at `openapi.yaml`.

## endpoints overview

### auth
- `POST /api/oauth/discord` - exchange Discord OAuth code for JWT
- `POST /api/oauth/refresh` - refresh JWT token

### guilds
- `GET /api/guilds` - list guilds the authenticated user can manage
- `GET /api/guilds/{id}/channels` - get guild channels
- `GET /api/guilds/{id}/roles` - get guild roles

### config
- `GET /api/config/{guild_id}` - fetch guild configuration
- `POST /api/config/{guild_id}` - update guild configuration

### infractions
- `GET /api/infractions/{guild_id}` - list/search infractions (query params: `user_id`, `type`, `active`)
- `POST /api/infractions` - create a new infraction
- `POST /api/infractions/{guild_id}/{id}/deactivate` - deactivate an infraction

### logging
- `GET /api/logging/{guild_id}` - fetch all log event configs for a guild
- `POST /api/logging/{guild_id}` - upsert a single log config
- `POST /api/logging/{guild_id}/bulk` - bulk upsert log configs
- `DELETE /api/logging/{guild_id}/{event}` - delete a log config

## env vars

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `DATABASE_URL` | Yes | N/A | PostgreSQL connection string. |
| `REDIS_URI` | Yes | N/A | Redis connection string for caching guild/user data. |
| `JWT_SECRET` | Yes | N/A | Secret key for signing JWTs. |
| `DISCORD_CLIENT_ID` | Yes | N/A | Discord OAuth2 application client ID. |
| `DISCORD_CLIENT_SECRET` | Yes | N/A | Discord OAuth2 application secret. |
| `DISCORD_REDIRECT_URI` | Yes | N/A | OAuth callback URL (e.g. `http://localhost:4173/oauth/discord`). |
| `DISCORD_BOT_TOKEN` | Yes | N/A | Discord bot token for fetching guild data. |
| `OTLP_ENDPOINT` | Yes | N/A | OpenTelemetry OTLP endpoint. |
| `REDIS_PREFIX` | No | `bm-api` | Redis key prefix for API cache. |
| `BOT_REDIS_PREFIX` | No | `bm` | Redis key prefix for bot cache namespace. |
| `OTLP_AUTH` | No | unset | Authorization header value for OTLP exporter. |
| `OTLP_ORGANIZATION` | No | unset | Optional org/tenant value for telemetry. |
| `API_HOST` | No | `0.0.0.0` | HTTP listen host. |
| `API_PORT` | No | `8080` | HTTP listen port. |

## auth flow

```mermaid
sequenceDiagram
    autonumber
    participant U as User
    participant DB as Dashboard
    participant API as API Server
    participant D as Discord OAuth

    U->>DB: Click "Login with Discord"
    DB->>D: Redirect to Discord OAuth
    D->>U: Authorization prompt
    U->>D: Approve
    D->>DB: Redirect to /oauth/discord?code=...
    DB->>API: POST {{API_BASE}}/oauth/discord with code
    API->>D: Exchange code for access token
    D-->>API: User access token + user info
    API->>API: Generate JWT signed with JWT_SECRET
    API-->>DB: JWT token
    DB->>DB: Store JWT in localStorage

    Note over U,API: Subsequent requests
    DB->>API: Request with Authorization: Bearer <JWT>
    API->>API: Verify JWT signature
    API->>API: Check permissions via Discord API + config
    API-->>DB: Response data
```
