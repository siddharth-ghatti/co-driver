use crate::config::AuthConfig;
use crate::error::{GatewayError, Result};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,         // Subject (user ID)
    pub exp: usize,          // Expiration time
    pub iat: usize,          // Issued at
    pub iss: String,         // Issuer
    pub aud: String,         // Audience
    pub roles: Vec<String>,  // User roles
    pub scopes: Vec<String>, // API scopes
}

/// User information extracted from authentication
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub roles: Vec<String>,
    pub scopes: Vec<String>,
}

/// Authentication middleware
pub struct AuthMiddleware {
    config: AuthConfig,
    jwt_encoding_key: Option<EncodingKey>,
    jwt_decoding_key: Option<DecodingKey>,
    validation: Validation,
}

impl AuthMiddleware {
    pub fn new(config: AuthConfig) -> Result<Self> {
        let (jwt_encoding_key, jwt_decoding_key) = if let Some(ref secret) = config.jwt_secret {
            let encoding_key = EncodingKey::from_secret(secret.as_bytes());
            let decoding_key = DecodingKey::from_secret(secret.as_bytes());
            (Some(encoding_key), Some(decoding_key))
        } else {
            (None, None)
        };

        let mut validation = Validation::default();
        if let Some(ref issuer) = config.jwt_issuer {
            validation.set_issuer(&[issuer]);
        }
        if let Some(ref audience) = config.jwt_audience {
            validation.set_audience(&[audience]);
        }

        Ok(Self {
            config,
            jwt_encoding_key,
            jwt_decoding_key,
            validation,
        })
    }

    /// Authenticate a request
    pub async fn authenticate(&self, headers: &HeaderMap) -> Result<Option<AuthenticatedUser>> {
        if !self.config.enabled {
            return Ok(None);
        }

        // Try JWT authentication first
        if let Some(user) = self.authenticate_jwt(headers).await? {
            return Ok(Some(user));
        }

        // Try API key authentication
        if let Some(user) = self.authenticate_api_key(headers).await? {
            return Ok(Some(user));
        }

        // Try basic authentication
        if let Some(user) = self.authenticate_basic(headers).await? {
            return Ok(Some(user));
        }

        Err(GatewayError::Authentication(
            "No valid authentication provided".to_string(),
        ))
    }

    /// Authenticate using JWT token
    async fn authenticate_jwt(&self, headers: &HeaderMap) -> Result<Option<AuthenticatedUser>> {
        let auth_header = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .filter(|h| h.starts_with("Bearer "));

        if let Some(auth_header) = auth_header {
            let token = &auth_header[7..]; // Remove "Bearer " prefix

            if let (Some(ref decoding_key), Some(ref validation)) =
                (&self.jwt_decoding_key, Some(&self.validation))
            {
                match decode::<Claims>(token, decoding_key, validation) {
                    Ok(token_data) => {
                        let claims = token_data.claims;
                        debug!("JWT authentication successful for user: {}", claims.sub);

                        return Ok(Some(AuthenticatedUser {
                            user_id: claims.sub,
                            roles: claims.roles,
                            scopes: claims.scopes,
                        }));
                    }
                    Err(e) => {
                        warn!("JWT authentication failed: {}", e);
                        return Err(GatewayError::Authentication(format!(
                            "Invalid JWT token: {}",
                            e
                        )));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Authenticate using API key
    async fn authenticate_api_key(&self, headers: &HeaderMap) -> Result<Option<AuthenticatedUser>> {
        if let Some(ref api_key_header) = self.config.api_key_header {
            if let Some(api_key) = headers.get(api_key_header).and_then(|h| h.to_str().ok()) {
                // In a real implementation, you would validate the API key against a database
                // For now, we'll use a simple validation
                if self.validate_api_key(api_key).await? {
                    debug!("API key authentication successful");
                    return Ok(Some(AuthenticatedUser {
                        user_id: format!("api_key_{}", api_key),
                        roles: vec!["api_user".to_string()],
                        scopes: vec!["read".to_string(), "write".to_string()],
                    }));
                } else {
                    return Err(GatewayError::Authentication("Invalid API key".to_string()));
                }
            }
        }

        Ok(None)
    }

    /// Authenticate using basic authentication
    async fn authenticate_basic(&self, headers: &HeaderMap) -> Result<Option<AuthenticatedUser>> {
        if let Some(ref basic_auth_users) = self.config.basic_auth_users {
            if let Some(auth_header) = headers
                .get("authorization")
                .and_then(|h| h.to_str().ok())
                .filter(|h| h.starts_with("Basic "))
            {
                let encoded = &auth_header[6..]; // Remove "Basic " prefix

                match base64::engine::general_purpose::STANDARD.decode(encoded) {
                    Ok(decoded) => {
                        if let Ok(credentials) = String::from_utf8(decoded) {
                            if let Some((username, password)) = credentials.split_once(':') {
                                if let Some(stored_password) = basic_auth_users.get(username) {
                                    if stored_password == password {
                                        debug!(
                                            "Basic authentication successful for user: {}",
                                            username
                                        );
                                        return Ok(Some(AuthenticatedUser {
                                            user_id: username.to_string(),
                                            roles: vec!["basic_user".to_string()],
                                            scopes: vec!["read".to_string()],
                                        }));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to decode basic auth: {}", e);
                    }
                }

                return Err(GatewayError::Authentication(
                    "Invalid basic authentication credentials".to_string(),
                ));
            }
        }

        Ok(None)
    }

    /// Validate an API key (placeholder implementation)
    async fn validate_api_key(&self, _api_key: &str) -> Result<bool> {
        // In a real implementation, this would check against a database or external service
        // For now, we'll accept any non-empty API key
        Ok(!_api_key.is_empty())
    }

    /// Generate a JWT token for a user
    pub fn generate_jwt_token(
        &self,
        user_id: &str,
        roles: Vec<String>,
        scopes: Vec<String>,
    ) -> Result<String> {
        if let Some(ref encoding_key) = self.jwt_encoding_key {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as usize;

            let exp = now + self.config.jwt_expiration.unwrap_or(3600) as usize;

            let claims = Claims {
                sub: user_id.to_string(),
                exp,
                iat: now,
                iss: self
                    .config
                    .jwt_issuer
                    .clone()
                    .unwrap_or_else(|| "mcp-gateway".to_string()),
                aud: self
                    .config
                    .jwt_audience
                    .clone()
                    .unwrap_or_else(|| "mcp-gateway".to_string()),
                roles,
                scopes,
            };

            let token = encode(&Header::default(), &claims, encoding_key).map_err(|e| {
                GatewayError::Authentication(format!("Failed to encode JWT: {}", e))
            })?;

            Ok(token)
        } else {
            Err(GatewayError::Authentication(
                "JWT secret not configured".to_string(),
            ))
        }
    }

    /// Check if user has required role
    pub fn has_role(user: &AuthenticatedUser, required_role: &str) -> bool {
        user.roles.contains(&required_role.to_string())
    }

    /// Check if user has required scope
    pub fn has_scope(user: &AuthenticatedUser, required_scope: &str) -> bool {
        user.scopes.contains(&required_scope.to_string())
    }

    /// Authorize a user for a specific operation
    pub fn authorize(
        &self,
        user: &AuthenticatedUser,
        required_roles: &[String],
        required_scopes: &[String],
    ) -> Result<()> {
        // Check roles
        if !required_roles.is_empty() {
            let has_required_role = required_roles.iter().any(|role| Self::has_role(user, role));

            if !has_required_role {
                return Err(GatewayError::Authorization(format!(
                    "User {} does not have required role. Required: {:?}, Has: {:?}",
                    user.user_id, required_roles, user.roles
                )));
            }
        }

        // Check scopes
        if !required_scopes.is_empty() {
            let has_required_scope = required_scopes
                .iter()
                .any(|scope| Self::has_scope(user, scope));

            if !has_required_scope {
                return Err(GatewayError::Authorization(format!(
                    "User {} does not have required scope. Required: {:?}, Has: {:?}",
                    user.user_id, required_scopes, user.scopes
                )));
            }
        }

        Ok(())
    }
}

/// Authentication middleware function
pub async fn auth_middleware(
    State(auth): State<Arc<AuthMiddleware>>,
    mut request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    if !auth.config.enabled {
        return Ok(next.run(request).await);
    }

    match auth.authenticate(request.headers()).await {
        Ok(Some(user)) => {
            // Add user to request extensions
            request.extensions_mut().insert(user);
            Ok(next.run(request).await)
        }
        Ok(None) => {
            // No authentication required for this request
            Ok(next.run(request).await)
        }
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Authorization middleware function
pub async fn require_auth(
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    if request.extensions().get::<AuthenticatedUser>().is_some() {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Role-based authorization middleware
pub fn require_role(
    required_role: String,
) -> impl Fn(Request, Next) -> std::result::Result<Response, StatusCode> + Clone {
    move |request: Request, next: Next| {
        let required_role = required_role.clone();
        async move {
            if let Some(user) = request.extensions().get::<AuthenticatedUser>() {
                if AuthMiddleware::has_role(user, &required_role) {
                    Ok(next.run(request).await)
                } else {
                    Err(StatusCode::FORBIDDEN)
                }
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
    }
}

/// Scope-based authorization middleware
pub fn require_scope(
    required_scope: String,
) -> impl Fn(Request, Next) -> std::result::Result<Response, StatusCode> + Clone {
    move |request: Request, next: Next| {
        let required_scope = required_scope.clone();
        async move {
            if let Some(user) = request.extensions().get::<AuthenticatedUser>() {
                if AuthMiddleware::has_scope(user, &required_scope) {
                    Ok(next.run(request).await)
                } else {
                    Err(StatusCode::FORBIDDEN)
                }
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn create_test_config() -> AuthConfig {
        AuthConfig {
            enabled: true,
            jwt_secret: Some("test_secret_key_that_is_long_enough".to_string()),
            jwt_issuer: Some("test-issuer".to_string()),
            jwt_audience: Some("test-audience".to_string()),
            jwt_expiration: Some(3600),
            api_key_header: Some("X-API-Key".to_string()),
            basic_auth_users: Some(
                vec![("testuser".to_string(), "testpass".to_string())]
                    .into_iter()
                    .collect(),
            ),
        }
    }

    #[tokio::test]
    async fn test_auth_middleware_creation() {
        let config = create_test_config();
        let auth = AuthMiddleware::new(config);
        assert!(auth.is_ok());
    }

    #[tokio::test]
    async fn test_jwt_token_generation() {
        let config = create_test_config();
        let auth = AuthMiddleware::new(config).unwrap();

        let token = auth.generate_jwt_token(
            "test_user",
            vec!["admin".to_string()],
            vec!["read".to_string(), "write".to_string()],
        );

        assert!(token.is_ok());
        let token = token.unwrap();
        assert!(!token.is_empty());
    }

    #[tokio::test]
    async fn test_jwt_authentication() {
        let config = create_test_config();
        let auth = AuthMiddleware::new(config).unwrap();

        let token = auth
            .generate_jwt_token(
                "test_user",
                vec!["admin".to_string()],
                vec!["read".to_string(), "write".to_string()],
            )
            .unwrap();

        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
        );

        let result = auth.authenticate(&headers).await;
        assert!(result.is_ok());

        let user = result.unwrap().unwrap();
        assert_eq!(user.user_id, "test_user");
        assert!(user.roles.contains(&"admin".to_string()));
        assert!(user.scopes.contains(&"read".to_string()));
    }

    #[tokio::test]
    async fn test_basic_authentication() {
        let config = create_test_config();
        let auth = AuthMiddleware::new(config).unwrap();

        let credentials = base64::encode("testuser:testpass");
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Basic {}", credentials)).unwrap(),
        );

        let result = auth.authenticate(&headers).await;
        assert!(result.is_ok());

        let user = result.unwrap().unwrap();
        assert_eq!(user.user_id, "testuser");
    }

    #[tokio::test]
    async fn test_api_key_authentication() {
        let config = create_test_config();
        let auth = AuthMiddleware::new(config).unwrap();

        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_str("test_api_key").unwrap());

        let result = auth.authenticate(&headers).await;
        assert!(result.is_ok());

        let user = result.unwrap().unwrap();
        assert!(user.user_id.starts_with("api_key_"));
    }

    #[test]
    fn test_role_authorization() {
        let user = AuthenticatedUser {
            user_id: "test_user".to_string(),
            roles: vec!["admin".to_string(), "user".to_string()],
            scopes: vec!["read".to_string(), "write".to_string()],
        };

        assert!(AuthMiddleware::has_role(&user, "admin"));
        assert!(AuthMiddleware::has_role(&user, "user"));
        assert!(!AuthMiddleware::has_role(&user, "super_admin"));
    }

    #[test]
    fn test_scope_authorization() {
        let user = AuthenticatedUser {
            user_id: "test_user".to_string(),
            roles: vec!["admin".to_string()],
            scopes: vec!["read".to_string(), "write".to_string()],
        };

        assert!(AuthMiddleware::has_scope(&user, "read"));
        assert!(AuthMiddleware::has_scope(&user, "write"));
        assert!(!AuthMiddleware::has_scope(&user, "delete"));
    }
}
