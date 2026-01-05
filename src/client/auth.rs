//! OAuth PKCE authentication flow for the CLI.
//!
//! Implements the Authorization Code flow with PKCE for secure
//! browser-based authentication.

use std::{net::TcpListener, sync::mpsc, thread, time::Duration};

use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope,
    TokenResponse, TokenUrl, basic::BasicClient,
};

use crate::{
    config::{CredentialStore, Credentials},
    error::{Error, Result},
};

/// Default OAuth configuration.
const DEFAULT_AUTH_URL: &str = "https://auth.inferadb.com/oauth/authorize";
const DEFAULT_TOKEN_URL: &str = "https://auth.inferadb.com/oauth/token";
const DEFAULT_CLIENT_ID: &str = "inferadb-cli";
const CALLBACK_PORT: u16 = 8787;

/// OAuth PKCE authentication flow.
#[derive(Debug, Clone)]
pub struct OAuthFlow {
    auth_url: String,
    token_url: String,
    client_id: String,
    redirect_url: String,
}

impl OAuthFlow {
    /// Create a new OAuth flow with default configuration.
    pub fn new() -> Result<Self> {
        Self::with_config(DEFAULT_AUTH_URL, DEFAULT_TOKEN_URL, DEFAULT_CLIENT_ID)
    }

    /// Create a new OAuth flow with custom configuration.
    pub fn with_config(auth_url: &str, token_url: &str, client_id: &str) -> Result<Self> {
        let redirect_url = format!("http://localhost:{CALLBACK_PORT}/callback");

        Ok(Self {
            auth_url: auth_url.to_string(),
            token_url: token_url.to_string(),
            client_id: client_id.to_string(),
            redirect_url,
        })
    }

    /// Start the OAuth flow and return credentials.
    ///
    /// This will:
    /// 1. Generate PKCE challenge
    /// 2. Open browser to authorization URL
    /// 3. Start local callback server
    /// 4. Exchange authorization code for tokens
    /// 5. Return credentials
    pub async fn authenticate(&self) -> Result<Credentials> {
        // Build the OAuth client with all endpoints set
        let client = BasicClient::new(ClientId::new(self.client_id.clone()))
            .set_auth_uri(
                AuthUrl::new(self.auth_url.clone()).map_err(|e| Error::oauth(e.to_string()))?,
            )
            .set_token_uri(
                TokenUrl::new(self.token_url.clone()).map_err(|e| Error::oauth(e.to_string()))?,
            )
            .set_redirect_uri(
                RedirectUrl::new(self.redirect_url.clone())
                    .map_err(|e| Error::oauth(e.to_string()))?,
            );

        // Generate PKCE challenge
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Generate authorization URL
        let (auth_url, csrf_token) = client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .add_scope(Scope::new("offline_access".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();

        eprintln!("Opening browser for authentication...");
        eprintln!("If the browser doesn't open, visit: {auth_url}");

        // Open browser
        if let Err(e) = webbrowser::open(auth_url.as_str()) {
            eprintln!("Failed to open browser: {e}");
            eprintln!("Please open this URL manually: {auth_url}");
        }

        // Wait for callback
        let (code, _state) = wait_for_callback(csrf_token.secret().clone())?;

        // Exchange code for tokens
        let http_client = reqwest::Client::new();

        let token_result = client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(&|request: oauth2::HttpRequest| {
                let http_client = http_client.clone();
                async move {
                    let response = http_client
                        .request(request.method().clone(), request.uri().to_string())
                        .headers(request.headers().clone())
                        .body(request.body().clone())
                        .send()
                        .await
                        .map_err(std::io::Error::other)?;

                    let status = response.status();
                    let body = response.bytes().await.map_err(std::io::Error::other)?;

                    Ok::<_, std::io::Error>(
                        http::Response::builder().status(status).body(body.to_vec()).unwrap(),
                    )
                }
            })
            .await
            .map_err(|e| Error::oauth(format!("Token exchange failed: {e}")))?;

        let access_token = token_result.access_token().secret().clone();
        let refresh_token = token_result.refresh_token().map(|t| t.secret().clone());
        let expires_at = token_result
            .expires_in()
            .map(|d| chrono::Utc::now() + chrono::Duration::seconds(d.as_secs() as i64));

        if let Some(refresh) = refresh_token {
            if let Some(expires) = expires_at {
                return Ok(Credentials::with_refresh(access_token, refresh, expires));
            }
        }

        Ok(Credentials::new(access_token))
    }
}

impl Default for OAuthFlow {
    fn default() -> Self {
        Self::new().expect("Failed to create OAuth flow")
    }
}

/// Wait for the OAuth callback.
fn wait_for_callback(expected_state: String) -> Result<(String, String)> {
    let listener = TcpListener::bind(format!("127.0.0.1:{CALLBACK_PORT}"))
        .map_err(|e| Error::oauth(format!("Failed to start callback server: {e}")))?;

    listener
        .set_nonblocking(false)
        .map_err(|e| Error::oauth(format!("Failed to configure listener: {e}")))?;

    let (tx, rx) = mpsc::channel();

    // Spawn listener thread
    let handle = thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    use std::io::{BufRead, BufReader, Write};

                    let reader = BufReader::new(&stream);
                    let request_line = reader.lines().next();

                    if let Some(Ok(line)) = request_line {
                        // Parse the GET request
                        if let Some(query) = extract_query(&line) {
                            // Parse query parameters
                            let params: std::collections::HashMap<_, _> = query
                                .split('&')
                                .filter_map(|p| {
                                    let mut parts = p.splitn(2, '=');
                                    Some((parts.next()?, parts.next()?))
                                })
                                .collect();

                            if let (Some(code), Some(state)) =
                                (params.get("code"), params.get("state"))
                            {
                                // Send success response
                                let response = "HTTP/1.1 200 OK\r\n\
                                    Content-Type: text/html\r\n\
                                    Connection: close\r\n\r\n\
                                    <html><body><h1>Authentication successful!</h1>\
                                    <p>You can close this window and return to the terminal.</p>\
                                    </body></html>";

                                let _ = stream.write_all(response.as_bytes());
                                let _ = tx.send(Ok(((*code).to_string(), (*state).to_string())));
                                break;
                            } else if let Some(error) = params.get("error") {
                                let description = params
                                    .get("error_description")
                                    .map_or_else(|| (*error).to_string(), |s| (*s).to_string());

                                let response = format!(
                                    "HTTP/1.1 400 Bad Request\r\n\
                                    Content-Type: text/html\r\n\
                                    Connection: close\r\n\r\n\
                                    <html><body><h1>Authentication failed</h1>\
                                    <p>{description}</p></body></html>"
                                );

                                let _ = stream.write_all(response.as_bytes());
                                let _ = tx.send(Err(Error::oauth(description)));
                                break;
                            }
                        }
                    }
                },
                Err(e) => {
                    let _ = tx.send(Err(Error::oauth(format!("Failed to accept connection: {e}"))));
                    break;
                },
            }
        }
    });

    // Wait for result with timeout
    let result = rx
        .recv_timeout(Duration::from_secs(300))
        .map_err(|_| Error::oauth("Authentication timed out"))?;

    let _ = handle.join();

    let (code, state) = result?;

    // Verify state matches
    if state != expected_state {
        return Err(Error::oauth("State mismatch - possible CSRF attack"));
    }

    Ok((code, state))
}

/// Store credentials for a profile after successful authentication.
pub fn store_credentials(profile: &str, credentials: &Credentials) -> Result<()> {
    let store = CredentialStore::new();
    store.store(profile, credentials)
}

/// Clear credentials for a profile (logout).
pub fn clear_credentials(profile: &str) -> Result<()> {
    let store = CredentialStore::new();
    store.delete(profile)
}

/// Check if credentials exist for a profile.
#[must_use]
pub fn has_credentials(profile: &str) -> bool {
    let store = CredentialStore::new();
    store.exists(profile)
}

/// Extract query string from HTTP request line.
fn extract_query(request_line: &str) -> Option<String> {
    // Format: "GET /callback?code=xxx&state=yyy HTTP/1.1"
    let path = request_line.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;
    Some(query.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_query() {
        let line = "GET /callback?code=abc&state=xyz HTTP/1.1";
        assert_eq!(extract_query(line), Some("code=abc&state=xyz".to_string()));

        let no_query = "GET /callback HTTP/1.1";
        assert!(extract_query(no_query).is_none());
    }
}
