# Table TV

A simple app with an API and UI, served together.

## Quick Start (Docker)

1. Build and run:

   ```bash
   docker compose up --build
   ```

2. Open in your browser:
   - **<http://localhost>** or **<http://127.0.0.1>**
   - For **<http://table-tv.local>**, add to `/etc/hosts`: `127.0.0.1 table-tv.local`

## Local Development

**Terminal 1 – API** (auto-reloads on changes; requires [cargo-watch](https://crates.io/crates/cargo-watch): `cargo install cargo-watch`):

```bash
cd api && cargo watch -x run
```

**Terminal 2 – UI:**

```bash
cd ui && npm run dev
```

The UI proxies `/api` to the API. Open <http://localhost:5173>.

To reset the database (e.g. if `initialized` is wrong): delete `api/data/` and restart the API.

## Auth0 Login

Login uses Auth0. Configure in Auth0 dashboard:

1. Create an **Application** (Single Page Application) – note the Client ID.
2. Create an **API** – note the API Identifier (this is your audience).
3. In Application settings, add **Allowed Callback URLs**: `http://localhost:5173` (and your production URL).
4. Add **Allowed Logout URLs**: `http://localhost:5173` (and production).

Set in `.env` (same vars for UI and API):

- `AUTH0_DOMAIN` – your Auth0 domain (e.g. `your-tenant.us.auth0.com`)
- `AUTH0_CLIENT_ID` – SPA Application Client ID
- `AUTH0_AUDIENCE` – your API identifier

The first user to log in becomes an admin.

### Auth0 403 troubleshooting

1. **Check Auth0 Logs** – Dashboard → Monitoring → Logs. Reproduce the 403, then find the failed event. The log shows the exact reason (e.g. `fco` = origin not in Allowed Web Origins).

2. **URL consistency** – Don’t use the API’s “Test Application”; create a new **Single Page Application** in Applications → Create Application.

2. **API User Access** – In APIs → [your API] → Application Access, set **User Access** to **Allow** (not “Allow via client-grant”) so any app can get tokens for user login.

3. **Callback URLs** – Add `http://127.0.0.1:5173` and `http://localhost:5173` to Allowed Callback URLs, Allowed Logout URLs, and Allowed Web Origins.

4. **Use ID token** – Add `AUTH0_SKIP_AUDIENCE=true` to `.env` to skip the API audience.

5. **Wrong client ID** – If Auth0 receives a different client ID than in `.env`: shell env vars override `.env`; check for `.env.local` or `.env.development`; restart the dev server. In dev mode, the console logs `[Auth0] Client ID loaded: xxxxxxxx...` so you can verify.

### Auth0 claims (username, email, profile picture)

The app requests `scope: 'openid profile email'`, which includes standard OIDC claims: `name`, `nickname`, `email`, `picture`. For social logins (Facebook, Google), these may be empty if the identity provider doesn’t share them.

**To add or fix claims in the token:**

1. **Auth0 Actions** – Dashboard → Actions → Flows → Login. Add a new Action that runs on “Login / Post Login”:

   ```javascript
   exports.onExecutePostLogin = async (event, api) => {
     const user = event.user;
     const name = user.name || (user.identities?.[0]?.profile_data?.name);
     if (name) api.idToken.setCustomClaim('name', name);
     if (user.email) api.idToken.setCustomClaim('email', user.email);
     if (user.picture) api.idToken.setCustomClaim('picture', user.picture);
     if (user.nickname) api.idToken.setCustomClaim('nickname', user.nickname);
   };
   ```

2. **Log out and log back in** – The Action only runs on new logins; your current token won't have the claims until you sign in again.

3. **Social connection settings** – Dashboard → Authentication → Social. For each connection (Facebook, Google, etc.), ensure the requested attributes include name, email, and profile picture.

4. **Facebook** – In the Facebook connection, use only `public_profile` and `email`. Remove `user_link` and any other invalid scopes. If you see "Invalid Scopes: email, user_link":
   - **Auth0**: Dashboard → Authentication → Social → Facebook → edit the connection. Set permissions to `public_profile,email` only.
   - **Meta for Developers**: Your Facebook app → Use cases → Authentication and account creation → add the `email` permission if needed.

**USB webcam:** If you use an external USB webcam instead of the built-in camera, set `CAMERA_INDEX=1` in `.env` (or `0` if the USB cam is the only/first device).

## RTMP streaming (Go Live)

RTMP export (YouTube, Facebook, etc.) uses **ffmpeg** to read the MJPEG stream and push to RTMP. The API requires ffmpeg to be installed and in `PATH`.

- **macOS:** `brew install ffmpeg`
- **Ubuntu/Debian:** `sudo apt install ffmpeg`
