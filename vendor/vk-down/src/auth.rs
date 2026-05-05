use anyhow::{Context, Result};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Identity and API version used by Music-M's Android audio bypass layer.
pub const VK_ANDROID_APP_ID: &str = "2274003";
pub const VK_ANDROID_CLIENT_SECRET: &str = "hHbZxrka2uZ6jB1inYsH";
pub const VK_API_VERSION: &str = "8.154";
pub const VK_USER_AGENT: &str =
    "VKAndroidApp/8.154-99999 (Android 12; SDK 32; arm64-v8a; Pixel 6; ru; 2960x1440)";

const API_BASE: &str = "https://api.vk.ru";
const DEFAULT_SUPPORTED_WAYS: &str = "sms,reserve_code,codegen,email";

/// Kept as aliases so callers compiled against the original crate continue to build.
#[deprecated(note = "vk-down now uses the official VK Android identity used by Music-M")]
pub const VK_KATE_MOBILE_CLIENT_ID: &str = VK_ANDROID_APP_ID;
#[deprecated(note = "vk-down now uses the official VK Android identity used by Music-M")]
pub const VK_KATE_MOBILE_CLIENT_SECRET: &str = VK_ANDROID_CLIENT_SECRET;
#[deprecated(note = "use VK_ANDROID_APP_ID")]
pub const VK_ADMIN_CLIENT_ID: &str = VK_ANDROID_APP_ID;

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub client_id: String,
    pub scope: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            client_id: VK_ANDROID_APP_ID.to_string(),
            scope: "all".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    #[serde(default)]
    pub anonymous_token: String,
    #[serde(default)]
    pub expires_in: i64,
    #[serde(default)]
    pub expires_at: i64,
    #[serde(default)]
    pub user_id: i64,
    #[serde(default)]
    pub exchange_token: Option<String>,
    #[serde(default)]
    pub device_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValidateAccountResponse {
    #[serde(default)]
    pub is_email: bool,
    #[serde(default)]
    pub is_phone: bool,
    #[serde(default)]
    pub flow_name: String,
    #[serde(default)]
    pub flow_names: Vec<String>,
    pub sid: String,
    #[serde(default)]
    pub next_step: Option<NextVerificationStep>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NextVerificationStep {
    #[serde(default)]
    pub verification_method: String,
    #[serde(default)]
    pub has_another_verification_methods: bool,
    #[serde(default)]
    pub external_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VerificationRequired {
    pub method: String,
    pub sid: String,
    pub phone_mask: Option<String>,
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("VK captcha required (sid: {sid})")]
pub struct CaptchaRequired {
    pub sid: String,
    pub image_url: Option<String>,
    pub redirect_uri: Option<String>,
    pub context: CaptchaContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptchaContext {
    ValidateAccount,
    OAuthToken,
}

#[derive(Debug, Clone)]
pub enum LoginResult {
    Authorized(TokenSet),
    VerificationRequired(VerificationRequired),
    CaptchaRequired(CaptchaRequired),
}

#[derive(Debug, Clone)]
pub struct AndroidAuth {
    client: Client,
    device_id: String,
    anonymous_token: String,
}

impl AndroidAuth {
    /// Initializes the same anonymous-token stage as Music-M.
    pub async fn new() -> Result<Self> {
        let client = android_client()?;
        let device_id = get_or_create_device_id();
        let anonymous_token = match load_env_var("VK_ANONYMOUS_TOKEN") {
            Some(token) => token,
            None => {
                let token = get_anonymous_token(&client, &device_id).await?;
                upsert_env_var("VK_ANONYMOUS_TOKEN", &token)?;
                token
            }
        };

        Ok(Self {
            client,
            device_id,
            anonymous_token,
        })
    }

    /// Creates an application-managed auth client without reading or writing `.env`.
    pub async fn for_session(session: Option<&TokenSet>) -> Result<Self> {
        tracing::info!(target: "vk_auth", has_session = session.is_some(), "initializing VK Android auth session");
        let client = android_client()?;
        let device_id = session
            .map(|value| value.device_id.clone())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(generate_device_id);
        let anonymous_token = match session
            .map(|value| value.anonymous_token.clone())
            .filter(|value| !value.is_empty())
        {
            Some(token) => token,
            None => {
                tracing::info!(target: "vk_auth", "requesting a new anonymous token");
                get_anonymous_token(&client, &device_id).await?
            }
        };
        tracing::info!(target: "vk_auth", reused_anonymous_token = session.is_some_and(|value| !value.anonymous_token.is_empty()), "VK Android auth session initialized");
        Ok(Self {
            client,
            device_id,
            anonymous_token,
        })
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn anonymous_token(&self) -> &str {
        &self.anonymous_token
    }

    /// Runs auth.validateAccount and returns the SID required by /oauth/token.
    pub async fn validate_account(&self, login: &str) -> Result<ValidateAccountResponse> {
        self.validate_account_with_captcha(login, None).await
    }

    pub async fn validate_account_with_captcha(
        &self,
        login: &str,
        captcha: Option<(&str, &str, bool)>,
    ) -> Result<ValidateAccountResponse> {
        tracing::info!(target: "vk_auth", captcha_answer = captcha.is_some(), "calling auth.validateAccount");
        let mut params = vec![
            ("login", login),
            ("force_password", "0"),
            ("supported_ways", DEFAULT_SUPPORTED_WAYS),
            ("flow_type", "auth_without_password"),
            ("api_id", VK_ANDROID_APP_ID),
            ("passkey_supported", "0"),
            ("access_token", self.anonymous_token.as_str()),
            ("v", VK_API_VERSION),
            ("lang", "ru"),
        ];
        if let Some((sid, key, interactive)) = captcha {
            if interactive {
                // VK ID Captcha returns a signed success_token. Music-M's
                // current VkApiInvoke sends it as success_token for API calls.
                params.push(("success_token", key));
            } else {
                params.push(("captcha_sid", sid));
                params.push(("captcha_key", key));
                params.push(("confirm", "1"));
            }
        }
        let value = post_json(
            &self.client,
            &format!("{API_BASE}/method/auth.validateAccount"),
            &params,
        )
        .await?;
        let response = api_response(value)?
            .try_into()
            .context("invalid auth.validateAccount response")?;
        tracing::info!(target: "vk_auth", "auth.validateAccount succeeded");
        Ok(response)
    }

    /// Performs Music-M's password/tg_flow authorization.
    pub async fn login_password(&self, login: &str, password: &str) -> Result<LoginResult> {
        tracing::info!(target: "vk_auth", "starting password authorization");
        let validation = match self.validate_account(login).await {
            Ok(validation) => validation,
            Err(error) => match error.downcast_ref::<CaptchaRequired>() {
                Some(challenge) => return Ok(LoginResult::CaptchaRequired(challenge.clone())),
                None => return Err(error),
            },
        };
        self.oauth_password(login, password, &validation.sid, None, None)
            .await
    }

    pub async fn login_password_after_captcha(
        &self,
        login: &str,
        password: &str,
        challenge: &CaptchaRequired,
        answer: &str,
    ) -> Result<LoginResult> {
        tracing::info!(target: "vk_auth", context = ?challenge.context, "continuing authorization after captcha");
        if challenge.context == CaptchaContext::OAuthToken {
            return self
                .oauth_password(login, password, &challenge.sid, None, Some(answer))
                .await;
        }
        let validation = match self
            .validate_account_with_captcha(
                login,
                Some((&challenge.sid, answer, challenge.redirect_uri.is_some())),
            )
            .await
        {
            Ok(validation) => validation,
            Err(error) => match error.downcast_ref::<CaptchaRequired>() {
                Some(next) => return Ok(LoginResult::CaptchaRequired(next.clone())),
                None => return Err(error),
            },
        };
        self.oauth_password(login, password, &validation.sid, None, None)
            .await
    }

    /// Repeats /oauth/token with a verification code requested by the previous call.
    pub async fn continue_with_code(
        &self,
        login: &str,
        password: &str,
        verification: &VerificationRequired,
        code: &str,
    ) -> Result<LoginResult> {
        tracing::info!(target: "vk_auth", method = %verification.method, "continuing authorization with verification code");
        self.oauth_password(login, password, &verification.sid, Some(code), None)
            .await
    }

    async fn oauth_password(
        &self,
        login: &str,
        password: &str,
        sid: &str,
        code: Option<&str>,
        success_token: Option<&str>,
    ) -> Result<LoginResult> {
        tracing::info!(target: "vk_auth", has_code = code.is_some(), has_captcha_token = success_token.is_some(), "calling VK oauth/token");
        let mut params = vec![
            ("grant_type", "password"),
            ("libverify_support", "0"),
            ("sid", sid),
            ("scope", "all"),
            ("supported_ways", DEFAULT_SUPPORTED_WAYS),
            ("device_id", self.device_id.as_str()),
            ("device_os", "android"),
            ("api_id", VK_ANDROID_APP_ID),
            ("https", "1"),
            ("lang", "ru"),
            ("v", VK_API_VERSION),
            ("anonymous_token", self.anonymous_token.as_str()),
            ("username", login),
            ("password", password),
            ("flow_type", "tg_flow"),
            ("2fa_supported", "1"),
            ("vk_connect_auth", "1"),
        ];
        if let Some(code) = code {
            params.push(("code", code));
        }
        if let Some(token) = success_token {
            params.push(("success_token", token));
        }

        let value = post_json(&self.client, &format!("{API_BASE}/oauth/token"), &params).await?;
        if value.get("error").and_then(Value::as_str) == Some("need_captcha") {
            tracing::warn!(target: "vk_auth", "oauth/token requested captcha");
            return Ok(LoginResult::CaptchaRequired(CaptchaRequired {
                sid: string_field(&value, "captcha_sid").unwrap_or_else(|| sid.to_string()),
                image_url: string_field(&value, "captcha_img"),
                redirect_uri: string_field(&value, "redirect_uri"),
                context: CaptchaContext::OAuthToken,
            }));
        }
        if value.get("error").and_then(Value::as_str) == Some("need_validation") {
            tracing::warn!(target: "vk_auth", method = %value.get("validation_type").and_then(|field| field.as_str()).unwrap_or("unknown"), "oauth/token requested verification");
            return Ok(LoginResult::VerificationRequired(VerificationRequired {
                method: value
                    .get("validation_type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                sid: value
                    .get("validation_sid")
                    .and_then(Value::as_str)
                    .unwrap_or(sid)
                    .to_string(),
                phone_mask: value
                    .get("phone_mask")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                external_id: value
                    .get("validation_external_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            }));
        }
        check_auth_error(&value)?;
        tracing::info!(target: "vk_auth", "oauth/token returned an access token");
        let mut token_set = token_set_from_value(&value)?;
        if token_set.exchange_token.is_none() {
            tracing::info!(target: "vk_auth", "requesting exchange token");
            token_set.exchange_token = Some(
                self.get_exchange_token(&token_set.access_token)
                    .await
                    .context("authorized, but failed to obtain Music-M exchange token")?,
            );
        }
        token_set.anonymous_token = self.anonymous_token.clone();
        token_set.device_id = self.device_id.clone();
        tracing::info!(target: "vk_auth", user_id = token_set.user_id, expires_in = token_set.expires_in, "VK authorization completed");
        Ok(LoginResult::Authorized(token_set))
    }

    /// Music-M calls execute.getUserInfo immediately after login to obtain the
    /// exchange token used by auth.refreshTokens.
    pub async fn get_exchange_token(&self, access_token: &str) -> Result<String> {
        tracing::info!(target: "vk_auth", "calling execute.getUserInfo for exchange token");
        let params = [
            ("func_v", "30"),
            ("androidVersion", "32"),
            ("androidManufacturer", "VK M"),
            ("androidModel", "VK M"),
            ("needExchangeToken", "1"),
            ("access_token", access_token),
            ("v", VK_API_VERSION),
            ("lang", "ru"),
        ];
        let value = post_json(
            &self.client,
            &format!("{API_BASE}/method/execute.getUserInfo"),
            &params,
        )
        .await?;
        api_response(value)?
            .get("exchange_token")
            .and_then(Value::as_str)
            .map(str::to_string)
            .context("exchange_token missing from execute.getUserInfo response")
    }

    /// Equivalent to Music-M's auth.refreshTokens call.
    pub async fn refresh_tokens(&self, old_token: &str, exchange_token: &str) -> Result<TokenSet> {
        tracing::info!(target: "vk_auth", "refreshing VK access token");
        let params = [
            ("exchange_tokens", exchange_token),
            ("scope", "all"),
            ("initiator", "expired_token"),
            ("active_index", "0"),
            ("api_id", VK_ANDROID_APP_ID),
            ("client_id", VK_ANDROID_APP_ID),
            ("client_secret", VK_ANDROID_CLIENT_SECRET),
            ("access_token", old_token),
            ("v", VK_API_VERSION),
            ("lang", "ru"),
        ];
        let value = post_json(
            &self.client,
            &format!("{API_BASE}/method/auth.refreshTokens"),
            &params,
        )
        .await?;
        let response = api_response(value)?;
        let slot = response
            .get("success")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .context("VK did not refresh the access token")?;
        let access = slot
            .get("access_token")
            .context("missing access_token slot")?;
        let token = access
            .get("token")
            .and_then(Value::as_str)
            .context("missing refreshed token")?;
        let expires_in = access
            .get("expires_in")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let user_id = slot.get("user_id").and_then(Value::as_i64).unwrap_or(0);
        let result = TokenSet {
            access_token: token.to_string(),
            anonymous_token: self.anonymous_token.clone(),
            expires_in,
            expires_at: expiration_time(expires_in),
            user_id,
            exchange_token: Some(exchange_token.to_string()),
            device_id: self.device_id.clone(),
        };
        tracing::info!(target: "vk_auth", user_id, expires_in, "VK access token refreshed");
        Ok(result)
    }
}

impl TryFrom<Value> for ValidateAccountResponse {
    type Error = serde_json::Error;

    fn try_from(value: Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

fn android_client() -> Result<Client> {
    let mut headers = header::HeaderMap::new();
    for (name, value) in [
        ("x-vk-android-client", "new"),
        ("referer", "https://id.vk.ru/"),
        ("origin", "https://id.vk.ru"),
        ("sec-fetch-mode", "cors"),
        ("sec-fetch-dest", "empty"),
        ("sec-fetch-site", "same-site"),
        ("sec-ch-ua-platform", "\"Android\""),
        ("sec-ch-ua-mobile", "?1"),
        ("x-quic", "1"),
    ] {
        headers.insert(
            header::HeaderName::from_bytes(name.as_bytes())?,
            header::HeaderValue::from_str(value)?,
        );
    }
    Client::builder()
        .user_agent(VK_USER_AGENT)
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .context("failed to build VK Android HTTP client")
}

async fn get_anonymous_token(client: &Client, device_id: &str) -> Result<String> {
    tracing::info!(target: "vk_auth", "calling oauth/get_anonym_token");
    let params = [
        ("client_id", VK_ANDROID_APP_ID),
        ("api_id", VK_ANDROID_APP_ID),
        ("client_secret", VK_ANDROID_CLIENT_SECRET),
        ("device_id", device_id),
        ("https", "1"),
        ("lang", "ru"),
        ("v", VK_API_VERSION),
    ];
    let value = post_json(
        client,
        &format!("{API_BASE}/oauth/get_anonym_token"),
        &params,
    )
    .await?;
    check_auth_error(&value)?;
    value
        .get("token")
        .or_else(|| value.get("access_token"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("anonymous token missing from VK response")
}

async fn post_json(client: &Client, url: &str, params: &[(&str, &str)]) -> Result<Value> {
    tracing::info!(target: "vk_auth", endpoint = %url, parameter_count = params.len(), "sending VK HTTP request");
    let response = client
        .post(url)
        .form(params)
        .send()
        .await
        .with_context(|| format!("failed to send request to {url}"))?;
    let status = response.status();
    tracing::info!(target: "vk_auth", endpoint = %url, http_status = %status, "received VK HTTP response");
    let body = response
        .text()
        .await
        .context("failed to read VK response")?;
    if !status.is_success() {
        anyhow::bail!("VK returned HTTP {status}: {body}");
    }
    serde_json::from_str(&body).with_context(|| format!("invalid VK JSON response: {body}"))
}

fn api_response(value: Value) -> Result<Value> {
    if let Some(error) = value.get("error") {
        let code = error.get("error_code").and_then(Value::as_i64).unwrap_or(0);
        let message = error
            .get("error_msg")
            .and_then(Value::as_str)
            .unwrap_or("unknown VK API error");
        if code == 14 {
            tracing::warn!(target: "vk_auth", error_code = code, "VK API requested captcha");
            let sid = error
                .get("captcha_sid")
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_string)
                        .or_else(|| value.as_u64().map(|number| number.to_string()))
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            return Err(CaptchaRequired {
                sid,
                image_url: error
                    .get("captcha_img")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                redirect_uri: error
                    .get("redirect_uri")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                context: CaptchaContext::ValidateAccount,
            }
            .into());
        }
        tracing::error!(target: "vk_auth", error_code = code, error_message = message, "VK API returned an error");
        anyhow::bail!("VK API error {code}: {message}");
    }
    value
        .get("response")
        .cloned()
        .context("VK response field is missing")
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|field| {
        field
            .as_str()
            .map(str::to_string)
            .or_else(|| field.as_u64().map(|number| number.to_string()))
    })
}

fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn expiration_time(expires_in: i64) -> i64 {
    if expires_in > 0 {
        unix_time().saturating_add(expires_in)
    } else {
        0
    }
}

fn check_auth_error(value: &Value) -> Result<()> {
    if let Some(error) = value.get("error") {
        let name = error.as_str().unwrap_or("authorization_error");
        let description = value
            .get("error_description")
            .and_then(Value::as_str)
            .unwrap_or("unknown VK authorization error");
        anyhow::bail!("VK authorization error {name}: {description}");
    }
    Ok(())
}

fn token_set_from_value(value: &Value) -> Result<TokenSet> {
    let expires_in = value.get("expires_in").and_then(Value::as_i64).unwrap_or(0);
    Ok(TokenSet {
        access_token: value
            .get("access_token")
            .and_then(Value::as_str)
            .context("access_token missing from VK response")?
            .to_string(),
        anonymous_token: String::new(),
        expires_in,
        expires_at: expiration_time(expires_in),
        user_id: value.get("user_id").and_then(Value::as_i64).unwrap_or(0),
        exchange_token: value
            .get("exchange_token")
            .and_then(Value::as_str)
            .map(str::to_string),
        device_id: String::new(),
    })
}

/// Backwards-compatible helper, now using Music-M's official Android flow.
pub async fn direct_login(username: &str, password: &str) -> Result<String> {
    let auth = AndroidAuth::new().await?;
    let mut result = auth.login_password(username, password).await?;
    loop {
        match result {
            LoginResult::Authorized(tokens) => {
                save_token_set(&tokens)?;
                return Ok(tokens.access_token);
            }
            LoginResult::CaptchaRequired(ref challenge) => {
                let url = challenge
                    .redirect_uri
                    .as_deref()
                    .or(challenge.image_url.as_deref())
                    .unwrap_or("<captcha URL missing>");
                eprintln!("VK requires captcha. Open this URL in a browser:\n{url}");
                print!("Captcha answer / success token: ");
                io::stdout().flush()?;
                let mut answer = String::new();
                io::stdin().read_line(&mut answer)?;
                result = auth
                    .login_password_after_captcha(username, password, challenge, answer.trim())
                    .await?;
            }
            LoginResult::VerificationRequired(ref verification) => {
                eprintln!("VK requires verification via {}", verification.method);
                if let Some(mask) = &verification.phone_mask {
                    eprintln!("Destination: {mask}");
                }
                print!("Verification code: ");
                io::stdout().flush()?;
                let mut code = String::new();
                io::stdin().read_line(&mut code)?;
                result = auth
                    .continue_with_code(username, password, verification, code.trim())
                    .await?;
            }
        }
    }
}

/// Loads a stored Android access token. Browser implicit OAuth is intentionally no longer used.
pub fn interactive_auth() -> Result<String> {
    load_token_from_env().context(
        "VK_ACCESS_TOKEN is not set; use AndroidAuth::login_password or direct_login first",
    )
}

pub fn get_oauth_url(_config: &AuthConfig) -> String {
    format!("{API_BASE}/oauth/token")
}

pub fn get_default_auth_url() -> String {
    get_oauth_url(&AuthConfig::default())
}

pub fn get_auth_html() -> String {
    "<!doctype html><html><body>VK Android authorization requires the native login flow.</body></html>".to_string()
}

pub fn extract_token_from_url(url_or_token: &str) -> String {
    url_or_token
        .split("access_token=")
        .nth(1)
        .and_then(|part| part.split('&').next())
        .unwrap_or(url_or_token)
        .to_string()
}

fn generate_device_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    let android_id = hasher.finish();
    (seed ^ 0x9e37_79b9_7f4a_7c15).hash(&mut hasher);
    let security_token = hasher.finish();
    format!("{android_id}:{security_token}")
}

pub fn get_or_create_device_id() -> String {
    if let Some(id) = load_env_var("VK_DEVICE_ID") {
        return id;
    }
    let id = generate_device_id();
    let _ = upsert_env_var("VK_DEVICE_ID", &id);
    id
}

pub fn dotenv_load() {
    dotenv::dotenv().ok();
}

fn load_env_var(key: &str) -> Option<String> {
    dotenv::dotenv().ok();
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

pub fn load_token_from_env() -> Option<String> {
    load_env_var("VK_ACCESS_TOKEN")
}

pub fn load_token_set_from_env() -> Option<TokenSet> {
    Some(TokenSet {
        access_token: load_token_from_env()?,
        anonymous_token: load_env_var("VK_ANONYMOUS_TOKEN").unwrap_or_default(),
        expires_in: load_env_var("VK_TOKEN_EXPIRES_IN")?.parse().ok()?,
        expires_at: load_env_var("VK_TOKEN_EXPIRES_AT")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
        user_id: load_env_var("VK_USER_ID")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        exchange_token: load_env_var("VK_EXCHANGE_TOKEN"),
        device_id: load_env_var("VK_DEVICE_ID").unwrap_or_default(),
    })
}

fn save_token_set(tokens: &TokenSet) -> Result<()> {
    upsert_env_var("VK_ACCESS_TOKEN", &tokens.access_token)?;
    upsert_env_var("VK_TOKEN_EXPIRES_IN", &tokens.expires_in.to_string())?;
    upsert_env_var("VK_TOKEN_EXPIRES_AT", &tokens.expires_at.to_string())?;
    upsert_env_var("VK_USER_ID", &tokens.user_id.to_string())?;
    if !tokens.anonymous_token.is_empty() {
        upsert_env_var("VK_ANONYMOUS_TOKEN", &tokens.anonymous_token)?;
    }
    if !tokens.device_id.is_empty() {
        upsert_env_var("VK_DEVICE_ID", &tokens.device_id)?;
    }
    if let Some(exchange) = &tokens.exchange_token {
        upsert_env_var("VK_EXCHANGE_TOKEN", exchange)?;
    }
    Ok(())
}

pub fn upsert_env_var(key: &str, value: &str) -> Result<()> {
    let env_path = ".env";
    let line = format!("{key}={value}");
    if Path::new(env_path).exists() {
        let existing = fs::read_to_string(env_path)?;
        let prefix = format!("{key}=");
        let mut found = false;
        let mut lines = existing
            .lines()
            .map(|old| {
                if old.starts_with(&prefix) {
                    found = true;
                    line.clone()
                } else {
                    old.to_string()
                }
            })
            .collect::<Vec<_>>();
        if !found {
            lines.push(line);
        }
        fs::write(env_path, lines.join("\n") + "\n")?;
    } else {
        fs::write(env_path, line + "\n")?;
    }
    Ok(())
}

pub fn save_token_to_env(token: &str) -> Result<()> {
    upsert_env_var("VK_ACCESS_TOKEN", token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_validate_account() {
        let value = serde_json::json!({
            "is_email": false,
            "is_phone": true,
            "flow_name": "need_password_and_validation",
            "flow_names": ["password", "otp"],
            "sid": "sid-value",
            "next_step": { "verification_method": "password", "has_another_verification_methods": true }
        });
        let parsed: ValidateAccountResponse = value.try_into().unwrap();
        assert_eq!(parsed.sid, "sid-value");
        assert_eq!(parsed.next_step.unwrap().verification_method, "password");
    }

    #[test]
    fn extracts_redirect_token() {
        assert_eq!(
            extract_token_from_url("https://oauth.vk.ru/blank.html#access_token=abc&expires_in=0"),
            "abc"
        );
    }

    #[test]
    fn parses_api_captcha_challenge() {
        let error = api_response(serde_json::json!({
            "error": {
                "error_code": 14,
                "error_msg": "Captcha needed",
                "captcha_sid": "42",
                "captcha_img": "https://api.vk.ru/captcha.php?sid=42",
                "redirect_uri": "https://id.vk.ru/captcha"
            }
        }))
        .unwrap_err();
        let challenge = error.downcast_ref::<CaptchaRequired>().unwrap();
        assert_eq!(challenge.sid, "42");
        assert_eq!(challenge.context, CaptchaContext::ValidateAccount);
        assert!(challenge.redirect_uri.is_some());
    }

    #[test]
    fn legacy_token_set_json_uses_safe_defaults() {
        let session: TokenSet =
            serde_json::from_str(r#"{"access_token":"token","expires_in":0,"user_id":1}"#).unwrap();
        assert_eq!(session.access_token, "token");
        assert!(session.anonymous_token.is_empty());
        assert_eq!(session.expires_at, 0);
    }
}
