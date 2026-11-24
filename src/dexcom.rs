use crate::core::{Limit, Reading, Trend, parse_timestamp};
use crate::i18n::Text;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Region {
    #[default]
    International,
    Us,
    Japan,
}
impl Region {
    pub fn label(self) -> &'static str {
        match self {
            Self::International => Text::International.get(),
            Self::Us => Text::UnitedStates.get(),
            Self::Japan => Text::Japan.get(),
        }
    }
    pub fn base_url(self) -> &'static str {
        match self {
            Self::International => "https://shareous1.dexcom.com/ShareWebServices/Services/",
            Self::Us => "https://share2.dexcom.com/ShareWebServices/Services/",
            Self::Japan => "https://share.dexcom.jp/ShareWebServices/Services/",
        }
    }
    fn application_id(self) -> &'static str {
        match self {
            Self::Japan => "d8665ade-9673-4e27-9ff6-92db4ce13d13",
            _ => "d89443d2-327c-4a6f-89e5-496bbb0317db",
        }
    }
}

// Intentionally no Debug: neither credentials nor sessions belong in logs.
#[derive(Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: Zeroizing<String>,
    pub region: Region,
}

pub use crate::sources::Error;

pub struct Client {
    http: reqwest::Client,
    base: String,
    credentials: Credentials,
    session: Option<Zeroizing<String>>,
}
impl Client {
    pub fn new(credentials: Credentials) -> Result<Self, Error> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("EasyGlucose/0.1")
            .build()
            .map_err(|_| Error::Network)?;
        Ok(Self {
            base: credentials.region.base_url().into(),
            credentials,
            http,
            session: None,
        })
    }
    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }
    async fn post(
        &self,
        endpoint: &str,
        body: Value,
        query: &[(&str, &str)],
    ) -> Result<Value, Error> {
        let mut response = self
            .http
            .post(format!("{}{endpoint}", self.base))
            .header("Accept", "application/json")
            .query(query)
            .json(&body)
            .send()
            .await
            .map_err(|_| Error::Network)?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Network)? {
            if bytes.len() + chunk.len() > 1_048_576 {
                return Err(Error::Protocol);
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
            if status >= 500 {
                Error::Server
            } else {
                Error::from_code("", status)
            }
        })?;
        if let Some(code) = value.get("Code").and_then(Value::as_str) {
            return Err(Error::from_code(code, status));
        }
        if !(200..300).contains(&status) {
            return Err(Error::from_code("", status));
        }
        Ok(value)
    }
    async fn login(&mut self) -> Result<(), Error> {
        self.session = None;
        let c = &self.credentials;
        let account = self.post("General/AuthenticatePublisherAccount", json!({"accountName": c.username, "password": *c.password, "applicationId": c.region.application_id()}), &[]).await?;
        let account = valid_id(&account)?;
        let session = self.post("General/LoginPublisherAccountById", json!({"accountId": account, "password": *c.password, "applicationId": c.region.application_id()}), &[]).await?;
        self.session = Some(Zeroizing::new(valid_id(&session)?.into()));
        Ok(())
    }
    async fn read(&self) -> Result<Vec<Reading>, Error> {
        let session = self.session.as_ref().ok_or(Error::Session)?;
        let value = self
            .post(
                "Publisher/ReadPublisherLatestGlucoseValues",
                json!({}),
                &[
                    ("sessionId", session),
                    ("minutes", "1440"),
                    ("maxCount", "288"),
                ],
            )
            .await?;
        parse_readings(value)
    }
    pub async fn fetch(&mut self) -> Result<Vec<Reading>, Error> {
        if self.session.is_none() {
            self.login().await?;
        }
        match self.read().await {
            Err(Error::Session) => {
                self.login().await?;
                self.read().await
            }
            result => result,
        }
    }
}
fn valid_id(value: &Value) -> Result<&str, Error> {
    let id = value.as_str().ok_or(Error::Protocol)?;
    if id == "00000000-0000-0000-0000-000000000000" || id.is_empty() {
        return Err(Error::Credentials);
    }
    if id.len() != 36
        || !id.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
    {
        return Err(Error::Protocol);
    }
    Ok(id)
}

pub fn parse_readings(value: Value) -> Result<Vec<Reading>, Error> {
    let array = value.as_array().ok_or(Error::Protocol)?;
    if array.len() > 288 {
        return Err(Error::Protocol);
    }
    let now = Utc::now();
    let mut readings: Vec<Reading> = array
        .iter()
        .filter_map(|v| {
            let mgdl = v
                .get("Value")
                .and_then(Value::as_u64)
                .and_then(|v| u16::try_from(v).ok())?;
            let stamp = v
                .get("WT")
                .or_else(|| v.get("ST"))
                .and_then(Value::as_str)?;
            let r = Reading {
                mgdl,
                measured_at: parse_timestamp(stamp)?,
                trend: Trend::parse(&v["Trend"]),
                limit: match mgdl {
                    39 => Some(Limit::Below(40)),
                    401 => Some(Limit::Above(400)),
                    _ => None,
                },
            };
            if !(39..=401).contains(&mgdl) || !r.valid(now) {
                return None;
            }
            Some(r)
        })
        .collect();
    if readings.is_empty() && !array.is_empty() {
        return Err(Error::Protocol);
    }
    readings.sort_by_key(|r| r.measured_at);
    readings.dedup_by_key(|r| r.measured_at);
    Ok(readings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_payload() {
        let r = parse_readings(json!([{"Value":100,"WT":"Date(1691455258000)","Trend":"Flat"}]))
            .expect("fixture");
        assert_eq!(r[0].trend, Trend::Flat);
        assert_eq!(r[0].measured_at.timestamp(), 1691455258);
        assert!(parse_readings(json!([{"Value":100,"Trend":4}])).is_err());
        assert!(parse_readings(json!(null)).is_err());
        assert_eq!(parse_readings(json!([])), Ok(vec![]));
    }
    #[test]
    fn mixed_batch_keeps_valid_measurements() {
        let valid = json!({"Value":123,"WT":"Date(1691455258000)"});
        let invalid = json!({"Value":0,"WT":"Date(1691454958000)"});
        let readings = parse_readings(json!([invalid, valid])).unwrap();
        assert_eq!(readings.len(), 1);
        assert_eq!(readings[0].mgdl, 123);
    }
    #[test]
    fn error_mapping() {
        assert_eq!(Error::from_code("SessionNotValid", 500), Error::Session);
        assert_eq!(
            Error::from_code("AccountPasswordInvalid", 500),
            Error::Credentials
        );
        assert_eq!(Error::from_code("", 429), Error::RateLimited);
    }
    #[tokio::test]
    async fn mock_session_renewed_once() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("address");
        let server = tokio::spawn(async move {
            let paths = [
                "AuthenticatePublisherAccount",
                "LoginPublisherAccountById",
                "ReadPublisherLatestGlucoseValues",
                "AuthenticatePublisherAccount",
                "LoginPublisherAccountById",
                "ReadPublisherLatestGlucoseValues",
            ];
            for (i, path) in paths.iter().enumerate() {
                let (mut socket, _) = listener.accept().await.expect("accept");
                let mut buffer = [0; 8192];
                let count = socket.read(&mut buffer).await.expect("read");
                assert!(String::from_utf8_lossy(&buffer[..count]).contains(path));
                let body = if i == 2 {
                    r#"{"Code":"SessionNotValid"}"#
                } else if i == 5 {
                    "[]"
                } else {
                    r#""11111111-1111-1111-1111-111111111111""#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes()).await.expect("write");
            }
        });
        let mut client = Client::new(Credentials {
            username: "fixture".into(),
            password: Zeroizing::new("fixture".into()),
            region: Region::International,
        })
        .expect("client");
        client.base = format!("http://{addr}/");
        assert!(client.fetch().await.expect("fetch").is_empty());
        server.await.expect("server");
    }
}
