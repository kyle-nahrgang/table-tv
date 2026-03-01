//! Auth0 JWT validation and user sync.
//!
//! Requires: AUTH0_DOMAIN, and either AUTH0_AUDIENCE or AUTH0_CLIENT_ID.
//! - AUTH0_AUDIENCE: for access tokens (SPA requests with audience)
//! - AUTH0_CLIENT_ID: for ID tokens (when VITE_AUTH0_SKIP_AUDIENCE=true to avoid 403)

use axum::{
    extract::{Request, State},
    http::header,
    routing::get,
    Json,
};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::api::AppState;
use crate::error::ApiError;

#[derive(Debug, Deserialize)]
struct Auth0Claims {
    sub: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    aud: serde_json::Value,
    exp: u64,
    iss: String,
}

#[derive(Serialize)]
pub struct AuthMeResponse {
    pub sub: String,
    pub email: String,
    pub is_admin: bool,
}

/// JWKS cache - fetches from Auth0 and refreshes on miss.
pub struct JwksCache {
    domain: String,
    jwks: RwLock<Option<JwkSet>>,
}

impl JwksCache {
    pub fn new(domain: &str) -> Self {
        let domain = domain.trim_end_matches('/').to_string();
        if !domain.starts_with("http") {
            return Self {
                domain: format!("https://{}", domain),
                jwks: RwLock::new(None),
            };
        }
        Self {
            domain,
            jwks: RwLock::new(None),
        }
    }

    async fn get_decoding_key(&self, kid: &str) -> Result<DecodingKey, ApiError> {
        let mut guard = self.jwks.write().await;
        if guard.is_none() {
            let url = format!("{}/.well-known/jwks.json", self.domain);
            let jwks: JwkSet = reqwest::get(&url)
                .await
                .map_err(|e| ApiError::Auth0ClientError(format!("Failed to fetch JWKS: {}", e)))?
                .json()
                .await
                .map_err(|e| ApiError::Auth0ClientError(format!("Failed to parse JWKS: {}", e)))?;
            *guard = Some(jwks);
        }
        let jwks = guard.as_ref().unwrap();
        let jwk = jwks
            .find(kid)
            .ok_or_else(|| ApiError::Auth0ClientError("JWK not found for kid".to_string()))?;
        DecodingKey::from_jwk(jwk)
            .map_err(|e| ApiError::Auth0ClientError(format!("Invalid JWK: {}", e)))
    }

    /// Invalidate cache (e.g. on 401 from Auth0) - allows retry with fresh JWKS.
    #[allow(dead_code)]
    async fn invalidate(&self) {
        *self.jwks.write().await = None;
    }
}

fn auth0_config() -> Result<(String, Vec<String>), ApiError> {
    let domain = std::env::var("AUTH0_DOMAIN")
        .map_err(|_| ApiError::BadRequest("AUTH0_DOMAIN must be set".to_string()))?;
    let mut audiences: Vec<String> = Vec::new();
    if let Ok(a) = std::env::var("AUTH0_AUDIENCE") {
        if !a.is_empty() {
            audiences.push(a);
        }
    }
    if let Ok(c) = std::env::var("AUTH0_CLIENT_ID") {
        if !c.is_empty() {
            tracing::info!(client_id = %c, "found client id");
            audiences.push(c);
        }
    }
    if audiences.is_empty() {
        return Err(ApiError::BadRequest(
            "AUTH0_AUDIENCE or AUTH0_CLIENT_ID must be set for Auth0 login".to_string(),
        ));
    }
    Ok((domain, audiences))
}

/// Validate JWT and return claims. Accepts tokens with any of the given audiences.
pub async fn validate_token(
    jwks: &JwksCache,
    token: &str,
    audiences: &[String],
    issuer: &str,
) -> Result<Auth0Claims, ApiError> {
    let header = decode_header(token)
        .map_err(|e| ApiError::Auth0ClientError(format!("Invalid token header: {}", e)))?;
    let kid = header
        .kid
        .ok_or_else(|| ApiError::Auth0ClientError("Token missing kid".to_string()))?;
    let key = jwks.get_decoding_key(&kid).await?;

    let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_audience(audiences);
    validation.set_issuer(&[issuer]);

    let token_data = decode::<Auth0Claims>(token, &key, &validation)
        .map_err(|e| ApiError::Auth0ClientError(format!("Invalid token: {}", e)))?;
    Ok(token_data.claims)
}

/// Extract email from claims. Auth0 may put it in different places.
fn email_from_claims(claims: &Auth0Claims) -> String {
    claims
        .email
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}@auth0.local", claims.sub))
}

/// Extract Bearer token from Authorization header.
fn bearer_token_from_request(req: &Request<axum::body::Body>) -> impl Iterator<Item = &str> {
    req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .into_iter()
        .filter_map(|s| s.strip_prefix("Bearer "))
}

/// GET /api/auth/me - Validate Bearer token, sync user to DB, return user info.
pub async fn auth_me(
    State(app): State<AppState>,
    req: Request<axum::body::Body>,
) -> Result<Json<AuthMeResponse>, ApiError> {
    let jwks = app
        .jwks
        .as_ref()
        .ok_or(ApiError::BadRequest("Auth0 not configured".to_string()))?;
    let (domain, audiences) = auth0_config()?;
    let domain_clean = domain.trim_start_matches("https://").trim_end_matches('/');
    let issuer = format!("https://{}/", domain_clean);

    let token = bearer_token_from_request(&req)
        .next()
        .ok_or(ApiError::InvalidCredentials)?;

    let claims = validate_token(jwks, token, &audiences, &issuer).await?;
    let email = email_from_claims(&claims);

    let user = app.db.upsert_user(claims.sub.clone(), email)?;

    Ok(Json(AuthMeResponse {
        sub: user.auth0_sub,
        email: user.email,
        is_admin: user.is_admin,
    }))
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new().route("/api/auth/me", get(auth_me))
}
