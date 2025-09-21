# rest api for black mesa

## Endpoints

### Authentication
- `POST /auth/login` - Discord OAuth login
- `POST /auth/refresh` - Refresh JWT token
- `POST /auth/logout` - Invalidate token

### Guild Management
- `GET /guilds/{id}` - Get guild information
- `PUT /guilds/{id}/config` - Update guild configuration
- `GET /guilds/{id}/members` - List guild members

### Moderation
- `GET /guilds/{id}/infractions` - List infractions
- `POST /guilds/{id}/infractions` - Create infraction
- `DELETE /infractions/{id}` - Remove infraction
