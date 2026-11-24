use super::{Error, http_client, json_response};
use crate::core::{Reading, Trend};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Auth {
    #[default]
    Token,
    ApiSecret,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub url: String,
    pub token: Zeroizing<String>,
    #[serde(default)]
    pub auth: Auth,
}
impl Credentials {
    pub(crate) fn from_form(url: &str, auth: Auth, password: &mut String) -> Result<Self, Error> {
        let url = normalize_url(url)?.to_string();
        Ok(Self {
            url,
            auth,
            token: Zeroizing::new(std::mem::take(password)),
        })
    }
}

pub struct Client {
    http: reqwest::Client,
    credentials: Credentials,
    base: reqwest::Url,
}
impl Client {
    pub fn new(mut credentials: Credentials) -> Result<Self, Error> {
        let base = normalize_url(&credentials.url)?;
        credentials.url = base.to_string();
        if credentials.auth == Auth::ApiSecret && credentials.token.is_empty() {
            return Err(Error::Credentials);
        }
        Ok(Self {
            http: http_client()?,
            credentials,
            base,
        })
    }
    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }
    pub async fn fetch(&mut self) -> Result<Vec<Reading>, Error> {
        let url = self
            .base
            .join("api/v1/entries/sgv.json")
            .map_err(|_| Error::InvalidUrl)?;
        let since = (Utc::now() - chrono::Duration::hours(24))
            .timestamp_millis()
            .to_string();
        let mut request = self
            .http
            .get(url)
            .header("Accept", "application/json")
            .query(&[("count", "2000"), ("find[date][$gte]", since.as_str())]);
        if !self.credentials.token.is_empty() {
            request = match self.credentials.auth {
                Auth::Token => request.query(&[("token", self.credentials.token.as_str())]),
                Auth::ApiSecret => request.header(
                    "api-secret",
                    format!("{:x}", Sha1::digest(self.credentials.token.as_bytes())),
                ),
            };
        }
        let value = json_response(request.send().await.map_err(|_| Error::Network)?).await?;
        parse_readings(value)
    }
}

pub fn normalize_url(input: &str) -> Result<reqwest::Url, Error> {
    let input = input.trim();
    if input.is_empty() || input.len() > 2048 {
        return Err(Error::InvalidUrl);
    }
    let address = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let mut url = reqwest::Url::parse(&address).map_err(|_| Error::InvalidUrl)?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::InvalidUrl);
    }
    let path = url.path().trim_end_matches('/').to_owned();
    // Accept either the site root or the API root pasted from Nightscout settings.
    let path = path.strip_suffix("/api/v1").unwrap_or(&path);
    url.set_path(&format!("{path}/"));
    Ok(url)
}

pub fn parse_readings(value: Value) -> Result<Vec<Reading>, Error> {
    let entries = value.as_array().ok_or(Error::Protocol)?;
    if entries.len() > 2000 {
        return Err(Error::Protocol);
    }
    let mut readings = Vec::with_capacity(entries.len());
    let now = Utc::now();
    let mut invalid_entry = false;
    for entry in entries {
        if entry
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| t != "sgv")
        {
            continue;
        }
        if let Some(reading) = parse_entry(entry, now) {
            readings.push(reading);
        } else {
            invalid_entry = true;
        }
    }
    if invalid_entry && readings.is_empty() {
        return Err(Error::Protocol);
    }
    readings.sort_by_key(|r| r.measured_at);
    readings.dedup_by_key(|r| r.measured_at);
    Ok(readings)
}

fn parse_entry(entry: &Value, now: DateTime<Utc>) -> Option<Reading> {
    let mgdl = entry
        .get("sgv")
        .and_then(Value::as_u64)
        .and_then(|n| u16::try_from(n).ok())?;
    let measured_at = if let Some(date) = entry.get("date") {
        // The v1 date is UTC milliseconds, never seconds or a guessed local time.
        date.as_i64().and_then(DateTime::from_timestamp_millis)?
    } else {
        DateTime::parse_from_rfc3339(entry.get("dateString").and_then(Value::as_str)?)
            .ok()?
            .with_timezone(&Utc)
    };
    if measured_at.timestamp() < 946684800 {
        return None;
    }
    let reading = Reading {
        mgdl,
        measured_at,
        trend: Trend::parse(
            entry
                .get("direction")
                .filter(|v| v.is_string())
                .unwrap_or(&entry["trend"]),
        ),
        limit: None,
    };
    if !reading.valid(now) {
        return None;
    }
    Some(reading)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn site_urls_preserve_prefix_and_reject_credential_leaks() {
        assert_eq!(
            normalize_url(" example.org/cgm/api/v1/ ")
                .expect("url")
                .as_str(),
            "https://example.org/cgm/"
        );
        for url in [
            "",
            "ftp://example.org",
            "http://example.org",
            "https://user:secret@example.org",
            "https://example.org/?token=secret",
            "https://example.org/#secret",
        ] {
            assert!(normalize_url(url).is_err(), "{url}");
        }
        assert!(normalize_url("http://127.0.0.1:1234").is_ok());
    }
    #[test]
    fn native_nightscout_values_do_not_inherit_dexcom_limits() {
        let t = Utc::now().timestamp_millis();
        let data = parse_readings(json!([{"sgv":501,"date":t,"direction":"SingleUp"},{"sgv":39,"date":t-60000,"trend":4}])).expect("readings");
        assert_eq!(data[0].display(crate::core::Unit::Mgdl), "39");
        assert_eq!(data[1].display(crate::core::Unit::Mgdl), "501");
        assert_eq!(data[1].trend, Trend::Up);
        assert_eq!(crate::core::recent_delta(&data), Some((462.0, 1)));
        for item in [
            json!({"sgv":100,"date":t+120000}),
            json!({"sgv":100,"date":t/1000}),
            json!({"sgv":100,"dateString":"2026-01-01 12:00"}),
            json!({"sgv":null,"date":t}),
            json!({"sgv":100.3,"date":t}),
        ] {
            assert_eq!(parse_readings(json!([item])), Err(Error::Protocol));
        }
        assert!(parse_readings(json!([])).expect("empty").is_empty());
        assert_eq!(
            parse_readings(json!([{"sgv":125,"dateString":"2026-01-01T12:00:00+01:00"}]))
                .expect("offset")[0]
                .measured_at
                .timestamp(),
            1767265200
        );
    }
    #[tokio::test]
    async fn corrected_site_reuses_the_token_entered_before_validation() {
        let mut token = String::from("fixture-token");
        assert!(matches!(
            Credentials::from_form("bad site", Auth::Token, &mut token),
            Err(Error::InvalidUrl)
        ));
        assert_eq!(token, "fixture-token");
        let (url, server) = super::super::test_support::server(vec![(200, json!([]))]).await;
        let credentials = Credentials::from_form(&url, Auth::Token, &mut token).unwrap();
        assert!(token.is_empty());
        Client::new(credentials).unwrap().fetch().await.unwrap();
        assert!(server.await.unwrap()[0].contains("token=fixture-token"));
    }

    #[tokio::test]
    async fn mixed_response_keeps_only_valid_sensor_readings() {
        let now = Utc::now().timestamp_millis();
        let response = json!([
            {"date": now, "sgv": 123},
            {"date": now - 7_200_000, "sgv": 0},
            {"dateString": "bad timestamp", "sgv": 105},
            {"type": "cal"}
        ]);
        let (url, server) = super::super::test_support::server(vec![(200, response)]).await;
        let mut client = Client::new(Credentials {
            url,
            token: Default::default(),
            auth: Auth::Token,
        })
        .unwrap();
        let readings = client.fetch().await.unwrap();
        assert_eq!(readings.len(), 1);
        assert_eq!(readings[0].mgdl, 123);
        assert_eq!(readings[0].measured_at.timestamp_millis(), now);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn token_secret_and_public_access_only_issue_get_requests() {
        for (auth, token) in [
            (Auth::Token, "noc_fixture+/token"),
            (Auth::ApiSecret, "fixture-api-secret"),
            (Auth::Token, ""),
        ] {
            let (url, server) = super::super::test_support::server(vec![(200, json!([]))]).await;
            let mut client = Client::new(Credentials {
                url: format!("{url}/cgm"),
                token: Zeroizing::new(token.into()),
                auth,
            })
            .expect("client");
            assert!(client.fetch().await.expect("fetch").is_empty());
            let request = server.await.expect("server").remove(0);
            assert!(request.starts_with("GET /cgm/api/v1/entries/sgv.json?"));
            assert!(request.contains("count=2000"));
            if auth == Auth::ApiSecret {
                assert!(
                    request.contains(&format!("api-secret: {:x}", Sha1::digest(token.as_bytes())))
                );
                assert!(!request.contains(token));
            } else if !token.is_empty() {
                assert!(request.contains("token=noc_fixture%2B%2Ftoken"));
            } else {
                assert!(!request.contains("token="));
            }
        }
    }
    #[tokio::test]
    async fn errors_are_redacted_and_redirects_never_forward_token() {
        for (status, error) in [
            (401, Error::Credentials),
            (429, Error::RateLimited),
            (503, Error::Server),
            (302, Error::Protocol),
        ] {
            let (url, server) = super::super::test_support::server(vec![(
                status,
                json!({"secret":"DO_NOT_DISPLAY"}),
            )])
            .await;
            let mut client = Client::new(Credentials {
                url,
                token: Zeroizing::new("fixture-token".into()),
                auth: Auth::Token,
            })
            .expect("client");
            assert_eq!(client.fetch().await, Err(error.clone()));
            assert!(!error.to_string().contains("DO_NOT_DISPLAY"));
            assert_eq!(server.await.expect("server").len(), 1);
        }
    }
}
