use crate::{Error, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    time::{Duration, Instant},
};
use url::Url;
#[derive(Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
}
impl ClientConfig {
    pub fn read(path: &Path) -> Result<Self> {
        if std::fs::metadata(path)?.len() > 65536 {
            return Err(Error::Remote("OAuth client file is too large".into()));
        }
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let installed = v.get("installed").ok_or_else(|| {
            Error::Remote("Choose a Google Desktop app OAuth client JSON file".into())
        })?;
        let config: Self = serde_json::from_value(installed.clone())?;
        if !config.client_id.ends_with(".apps.googleusercontent.com") {
            return Err(Error::Remote("Invalid Google desktop OAuth client".into()));
        }
        Ok(config)
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct GoogleCredentials {
    pub config: ClientConfig,
    pub refresh_token: String,
    pub access_token: String,
    pub expires_at: i64,
}
pub struct PendingOAuth {
    pub url: String,
    listener: TcpListener,
    state: String,
    verifier: String,
    redirect: String,
    config: ClientConfig,
}
fn random() -> String {
    let mut data = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut data);
    URL_SAFE_NO_PAD.encode(data)
}
pub fn begin_oauth(config: ClientConfig) -> Result<PendingOAuth> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    listener.set_nonblocking(true)?;
    let redirect = format!(
        "http://127.0.0.1:{}/callback",
        listener.local_addr()?.port()
    );
    let state = random();
    let verifier = random();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth")
        .map_err(|_| Error::Remote("OAuth endpoint unavailable".into()))?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", config.client_id.as_str()),
        ("redirect_uri", &redirect),
        ("response_type", "code"),
        ("scope", "https://www.googleapis.com/auth/drive.file"),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
        ("state", &state),
        ("access_type", "offline"),
        ("prompt", "consent select_account"),
    ]);
    Ok(PendingOAuth {
        url: url.to_string(),
        listener,
        state,
        verifier,
        redirect,
        config,
    })
}
fn callback_code(request: &str, state: &str) -> Result<String> {
    let mut parts = request
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace();
    if parts.next() != Some("GET") {
        return Err(Error::Remote("Invalid authorization callback".into()));
    }
    let path = parts.next().unwrap_or_default();
    if !path.starts_with("/callback?") {
        return Err(Error::Remote("Invalid authorization callback path".into()));
    }
    let u = Url::parse(&format!("http://127.0.0.1{path}"))
        .map_err(|_| Error::Remote("Invalid authorization callback".into()))?;
    let pairs: Vec<_> = u.query_pairs().collect();
    let states: Vec<_> = pairs.iter().filter(|(k, _)| k == "state").collect();
    if states.len() != 1 || states[0].1 != state {
        return Err(Error::Remote("Authorization state did not match".into()));
    }
    if pairs.iter().any(|(k, _)| k == "error") {
        return Err(Error::Remote("Google authorization was denied".into()));
    }
    let codes: Vec<_> = pairs.iter().filter(|(k, _)| k == "code").collect();
    if codes.len() != 1 || codes[0].1.is_empty() {
        return Err(Error::Remote("Authorization code is missing".into()));
    }
    Ok(codes[0].1.to_string())
}
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: i64,
}
async fn token(form: &[(&str, &str)]) -> Result<TokenResponse> {
    let response = reqwest::Client::new()
        .post("https://oauth2.googleapis.com/token")
        .form(form)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| {
            Error::Remote(
                "Could not contact Google authorization. Your local data is unchanged.".into(),
            )
        })?;
    if !response.status().is_success() {
        return Err(Error::Remote(
            "Google rejected authorization. Reconnect your account.".into(),
        ));
    }
    response
        .json()
        .await
        .map_err(|_| Error::Remote("Google returned an invalid token response".into()))
}
pub async fn finish_oauth(pending: PendingOAuth) -> Result<GoogleCredentials> {
    let code=tokio::task::spawn_blocking(move||->Result<(String,PendingOAuth)>{let until=Instant::now()+Duration::from_secs(180);while Instant::now()<until{match pending.listener.accept(){Ok((mut stream,addr))=>{if !addr.ip().is_loopback(){continue}stream.set_read_timeout(Some(Duration::from_secs(2)))?;let mut buf=[0;8192];let n=stream.read(&mut buf)?;let code=callback_code(&String::from_utf8_lossy(&buf[..n]),&pending.state);let(ok,message)=if code.is_ok(){("200 OK","Authorization received. Return to Cycles.")}else{("400 Bad Request","Authorization could not be completed. Return to Cycles.")};let response=format!("HTTP/1.1 {ok}\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{message}",message.len());let _=stream.write_all(response.as_bytes());return Ok((code?,pending));},Err(e)if e.kind()==std::io::ErrorKind::WouldBlock=>std::thread::sleep(Duration::from_millis(80)),Err(e)=>return Err(e.into())}}Err(Error::Remote("Google sign-in timed out. Try connecting again.".into()))}).await.map_err(|_|Error::Remote("Authorization listener stopped".into()))??;
    let (code, p) = code;
    let response = token(&[
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("client_id", &p.config.client_id),
        ("client_secret", &p.config.client_secret),
        ("redirect_uri", &p.redirect),
        ("code_verifier", &p.verifier),
    ])
    .await?;
    let refresh = response.refresh_token.ok_or_else(|| {
        Error::Remote("Google did not grant offline access. Reconnect and approve access.".into())
    })?;
    Ok(GoogleCredentials {
        config: p.config,
        refresh_token: refresh,
        access_token: response.access_token,
        expires_at: chrono::Utc::now().timestamp() + response.expires_in - 60,
    })
}
pub async fn refresh_credentials(credentials: &mut GoogleCredentials) -> Result<()> {
    if credentials.expires_at > chrono::Utc::now().timestamp() {
        return Ok(());
    }
    let response = token(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", &credentials.refresh_token),
        ("client_id", &credentials.config.client_id),
        ("client_secret", &credentials.config.client_secret),
    ])
    .await?;
    credentials.access_token = response.access_token;
    credentials.expires_at = chrono::Utc::now().timestamp() + response.expires_in - 60;
    if let Some(t) = response.refresh_token {
        credentials.refresh_token = t
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_requires_state_and_correct_path() {
        assert!(callback_code("GET /callback?state=wrong&code=secret HTTP/1.1", "right").is_err());
        assert!(callback_code("GET /elsewhere?state=right&code=secret HTTP/1.1", "right").is_err());
        assert_eq!(
            callback_code("GET /callback?state=right&code=abc HTTP/1.1", "right").unwrap(),
            "abc"
        );
        assert!(callback_code(
            "GET /callback?state=right&state=wrong&code=a HTTP/1.1",
            "right"
        )
        .is_err());
    }
}
