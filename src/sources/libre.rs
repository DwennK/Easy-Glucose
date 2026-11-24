use super::{Error, Patient, http_client, json_response};
use crate::core::{Limit, Reading, Trend};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

#[derive(Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: Zeroizing<String>,
    #[serde(default)]
    pub patient_id: Option<String>,
}
pub struct Client {
    http: reqwest::Client,
    credentials: Credentials,
    region: String,
    version: String,
    token: Option<Zeroizing<String>>,
    account_id: String,
    pub patients: Vec<Patient>,
    #[cfg(test)]
    base_override: Option<String>,
}
impl Client {
    pub fn new(credentials: Credentials) -> Result<Self, Error> {
        Ok(Self {
            http: http_client()?,
            credentials,
            region: "eu".into(),
            version: "4.16.0".into(),
            token: None,
            account_id: String::new(),
            patients: vec![],
            #[cfg(test)]
            base_override: None,
        })
    }
    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }
    fn base(&self) -> String {
        #[cfg(test)]
        if let Some(base) = &self.base_override {
            return base.clone();
        }
        format!("https://api-{}.libreview.io", self.region)
    }
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base()))
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("product", "llu.android")
            .header("version", &self.version)
            .header("cache-control", "no-cache");
        if let Some(token) = &self.token {
            request = request
                .bearer_auth(token.as_str())
                .header("Account-Id", &self.account_id);
        }
        request
    }
    async fn login(&mut self) -> Result<(), Error> {
        self.token = None;
        for _ in 0..4 {
            let response = self.request(reqwest::Method::POST, "/llu/auth/login")
                .json(&json!({"email": self.credentials.username, "password": *self.credentials.password}))
                .send().await.map_err(|_| Error::Network)?;
            let value = json_response(response).await?;
            let data = &value["data"];
            if value["status"].as_i64() == Some(920) {
                let version = data["minimumVersion"]
                    .as_str()
                    .filter(|s| {
                        !s.is_empty()
                            && s.len() <= 24
                            && s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
                    })
                    .ok_or(Error::Protocol)?;
                self.version = version.into();
                continue;
            }
            if data["redirect"].as_bool() == Some(true) {
                // A server-provided region can only select an Abbott subdomain, never an arbitrary host.
                self.region =
                    validate_region(data["region"].as_str().ok_or(Error::Protocol)?)?.into();
                continue;
            }
            check_status(&value)?;
            let token = data["authTicket"]["token"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or(Error::Protocol)?;
            let account = data["user"]["id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or(Error::Protocol)?;
            self.account_id = format!("{:x}", Sha256::digest(account.as_bytes()));
            self.token = Some(Zeroizing::new(token.into()));
            return Ok(());
        }
        Err(Error::Protocol)
    }
    async fn get(&self, path: &str) -> Result<Value, Error> {
        let value = json_response(
            self.request(reqwest::Method::GET, path)
                .send()
                .await
                .map_err(|_| Error::Network)?,
        )
        .await?;
        check_status(&value)?;
        value.get("data").cloned().ok_or(Error::Protocol)
    }
    async fn read(&mut self) -> Result<Vec<Reading>, Error> {
        let connections = self.get("/llu/connections").await?;
        self.patients = parse_patients(&connections)?;
        if self.patients.is_empty() {
            return Err(Error::NoPatients);
        }
        match self.credentials.patient_id.as_deref() {
            Some(id) if self.patients.iter().any(|p| p.id == id) => {}
            None if self.patients.len() == 1 => {
                self.credentials.patient_id = Some(self.patients[0].id.clone())
            }
            _ => {
                self.credentials.patient_id = None;
                return Err(Error::SelectPatient);
            }
        }
        let id = self
            .credentials
            .patient_id
            .as_deref()
            .ok_or(Error::SelectPatient)?;
        let graph = self.get(&format!("/llu/connections/{id}/graph")).await?;
        if graph["connection"]["patientId"].as_str() != Some(id) {
            return Err(Error::Protocol);
        }
        parse_graph(&graph)
    }
    pub fn select_patient(&mut self, id: &str) -> Result<(), Error> {
        if !self.patients.iter().any(|p| p.id == id) {
            return Err(Error::SelectPatient);
        }
        self.credentials.patient_id = Some(id.into());
        Ok(())
    }
    pub fn selected_name(&self) -> Option<String> {
        self.patients
            .iter()
            .enumerate()
            .find(|(_, p)| Some(&p.id) == self.credentials.patient_id.as_ref())
            .map(|(index, p)| {
                if p.name.is_empty() {
                    format!("#{}", index + 1)
                } else {
                    p.name.clone()
                }
            })
    }
    pub async fn fetch(&mut self) -> Result<Vec<Reading>, Error> {
        if self.token.is_none() {
            self.login().await?;
        }
        match self.read().await {
            Err(Error::Credentials | Error::Session) => {
                self.login().await?;
                self.read().await
            }
            result => result,
        }
    }
}
fn validate_region(region: &str) -> Result<&str, Error> {
    if region.is_empty()
        || region.len() > 12
        || !region
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    {
        Err(Error::Protocol)
    } else {
        Ok(region)
    }
}
fn check_status(value: &Value) -> Result<(), Error> {
    if value["status"].as_i64() == Some(4) || value["data"].get("step").is_some() {
        return Err(Error::Terms);
    }
    match value["status"].as_i64() {
        Some(0) => Ok(()),
        Some(2) => Err(Error::Credentials),
        Some(429) => Err(Error::RateLimited),
        _ => Err(Error::Protocol),
    }
}
fn parse_patients(value: &Value) -> Result<Vec<Patient>, Error> {
    let connections = value.as_array().ok_or(Error::Protocol)?;
    if connections.len() > 100 {
        return Err(Error::Protocol);
    }
    let mut patients = Vec::new();
    for connection in connections {
        let id = connection["patientId"]
            .as_str()
            .filter(|s| {
                !s.is_empty()
                    && s.len() <= 128
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            })
            .ok_or(Error::Protocol)?;
        let name = format!(
            "{} {}",
            connection["firstName"].as_str().unwrap_or(""),
            connection["lastName"].as_str().unwrap_or("")
        );
        let name: String = name
            .trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(100)
            .collect();
        patients.push(Patient {
            id: id.into(),
            name,
        });
    }
    Ok(patients)
}
fn parse_measurement(value: &Value) -> Result<Reading, Error> {
    // FactoryTimestamp is UTC. Timestamp alone is the phone's local time and is ambiguous.
    let raw = value["FactoryTimestamp"].as_str().ok_or(Error::Protocol)?;
    let measured_at = NaiveDateTime::parse_from_str(raw, "%m/%d/%Y %I:%M:%S %p")
        .map(|t| t.and_utc())
        .or_else(|_| DateTime::parse_from_rfc3339(raw).map(|t| t.with_timezone(&Utc)))
        .map_err(|_| Error::Protocol)?;
    let mgdl = value["ValueInMgPerDl"]
        .as_u64()
        .and_then(|n| u16::try_from(n).ok())
        .ok_or(Error::Protocol)?;
    let trend = match value["TrendArrow"].as_u64() {
        Some(1) => Trend::Down,
        Some(2) => Trend::SlightDown,
        Some(3) => Trend::Flat,
        Some(4) => Trend::SlightUp,
        Some(5) => Trend::Up,
        _ => Trend::Unknown,
    };
    let limit = match (value["isLow"].as_bool(), value["isHigh"].as_bool()) {
        (Some(true), Some(true)) => return Err(Error::Protocol),
        (Some(true), _) if mgdl <= 40 => Some(Limit::Below(40)),
        (_, Some(true)) if mgdl >= 500 => Some(Limit::Above(500)),
        _ => None,
    };
    let reading = Reading {
        mgdl,
        measured_at,
        trend,
        limit,
    };
    if !(39..=501).contains(&mgdl) || !reading.valid(Utc::now()) {
        return Err(Error::Protocol);
    }
    Ok(reading)
}
pub fn parse_graph(graph: &Value) -> Result<Vec<Reading>, Error> {
    let history = graph["graphData"].as_array().ok_or(Error::Protocol)?;
    if history.len() > 2000 {
        return Err(Error::Protocol);
    }
    let current = &graph["connection"]["glucoseMeasurement"];
    let readings: Vec<_> = history
        .iter()
        .chain(Some(current).filter(|value| !value.is_null()))
        .filter_map(|value| parse_measurement(value).ok())
        .collect();
    if readings.is_empty() && (!history.is_empty() || !current.is_null()) {
        return Err(Error::Protocol);
    }
    // Current wins when it corrects an historical point at the same timestamp.
    let mut merged = Vec::new();
    crate::core::merge_readings(&mut merged, readings, Utc::now(), 1);
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn credentials() -> Credentials {
        Credentials {
            username: "fixture@example.invalid".into(),
            password: Zeroizing::new("fixture-password".into()),
            patient_id: None,
        }
    }
    fn login() -> Value {
        json!({"status":0,"data":{"authTicket":{"token":"fixture-token"},"user":{"id":"fixture-user"}}})
    }
    fn connections() -> Value {
        json!({"status":0,"data":[{"patientId":"person-a","firstName":"Alice"},{"patientId":"person-b","firstName":"Bob"}]})
    }
    fn measurement(mgdl: u16, arrow: u8) -> Value {
        json!({"FactoryTimestamp":Utc::now().format("%m/%d/%Y %I:%M:%S %p").to_string(),"Timestamp":"not-used-local-time","ValueInMgPerDl":mgdl,"TrendArrow":arrow})
    }
    fn graph(id: &str) -> Value {
        json!({"status":0,"data":{"connection":{"patientId":id,"glucoseMeasurement":measurement(123,3)},"graphData":[]}})
    }
    #[test]
    fn mixed_graph_keeps_valid_current_or_historical_measurements() {
        let valid = measurement(123, 3);
        let invalid = measurement(0, 3);
        for (history, current) in [(&invalid, &valid), (&valid, &invalid)] {
            let graph = json!({"graphData":[history],"connection":{"glucoseMeasurement":current}});
            let readings = parse_graph(&graph).unwrap();
            assert_eq!(readings.len(), 1);
            assert_eq!(readings[0].mgdl, 123);
        }
        assert_eq!(
            parse_graph(&json!({"graphData":[invalid]})),
            Err(Error::Protocol)
        );
        assert_eq!(parse_graph(&json!({"graphData":[]})), Ok(vec![]));
    }
    #[test]
    fn unnamed_selection_keeps_its_number_in_every_language() {
        let mut client = Client::new(credentials()).unwrap();
        client.patients = parse_patients(&json!([
            {"patientId":"person-a","firstName":"Alice"},
            {"patientId":"person-b"}
        ]))
        .unwrap();
        client.select_patient("person-b").unwrap();
        assert_eq!(client.selected_name().as_deref(), Some("#2"));
    }
    #[test]
    fn libre_has_its_own_trends_and_utc_timestamp() {
        for (arrow, trend) in [
            (1, Trend::Down),
            (2, Trend::SlightDown),
            (3, Trend::Flat),
            (4, Trend::SlightUp),
            (5, Trend::Up),
        ] {
            let r = parse_measurement(&measurement(110, arrow)).expect("measurement");
            assert_eq!(r.trend, trend);
            assert!(r.age_seconds(Utc::now()) <= 1);
        }
        let mut v = measurement(110, 3);
        v.as_object_mut()
            .expect("object")
            .remove("FactoryTimestamp");
        assert_eq!(parse_measurement(&v), Err(Error::Protocol));
        v["FactoryTimestamp"] = json!((Utc::now() + chrono::Duration::hours(2)).to_rfc3339());
        assert_eq!(parse_measurement(&v), Err(Error::Protocol));
        let mut high = measurement(501, 5);
        high["isHigh"] = json!(true);
        assert_eq!(
            parse_measurement(&high)
                .expect("limit")
                .display(crate::core::Unit::Mgdl),
            "> 500"
        );
        let mut low = measurement(39, 1);
        low["isLow"] = json!(true);
        assert_eq!(
            parse_measurement(&low)
                .expect("limit")
                .display(crate::core::Unit::Mgdl),
            "< 40"
        );
    }
    #[tokio::test]
    async fn multiple_people_require_selection_and_expired_session_renews_once() {
        let replies = vec![
            (200, login()),
            (200, connections()),
            (200, connections()),
            (401, json!({})),
            (200, login()),
            (200, connections()),
            (200, graph("person-b")),
        ];
        let (base, server) = super::super::test_support::server(replies).await;
        let mut client = Client::new(credentials()).expect("client");
        client.base_override = Some(base);
        assert_eq!(client.fetch().await, Err(Error::SelectPatient));
        assert_eq!(client.patients.len(), 2);
        assert_eq!(
            client.select_patient("not-a-person"),
            Err(Error::SelectPatient)
        );
        client.select_patient("person-b").expect("select");
        let data = client.fetch().await.expect("readings");
        assert_eq!(data[0].mgdl, 123);
        assert_eq!(client.selected_name().as_deref(), Some("Bob"));
        let requests = server.await.expect("server");
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.starts_with("POST /llu/auth/login"))
                .count(),
            2
        );
        assert!(requests[3].starts_with("GET /llu/connections/person-b/graph"));
        assert!(
            !requests
                .iter()
                .any(|r| r.starts_with("GET /llu/connections/person-a/graph"))
        );
        assert!(requests[1].contains(&format!(
            "account-id: {:x}",
            Sha256::digest(b"fixture-user")
        )));
    }
    #[tokio::test]
    async fn version_and_region_negotiation_are_bounded() {
        let (base, server) = super::super::test_support::server(vec![
            (
                200,
                json!({"status":0,"data":{"redirect":true,"region":"us"}}),
            ),
            (
                200,
                json!({"status":920,"data":{"minimumVersion":"4.17.0"}}),
            ),
            (200, login()),
            (200, json!({"status":0,"data":[{"patientId":"person-a"}]})),
            (200, graph("person-a")),
        ])
        .await;
        let mut client = Client::new(credentials()).expect("client");
        client.base_override = Some(base);
        assert_eq!(client.fetch().await.expect("fetch")[0].mgdl, 123);
        assert_eq!(client.region, "us");
        assert!(server.await.expect("server")[2].contains("version: 4.17.0"));
        for region in ["../evil", "eu.example.org", "eu:443", "", "EU"] {
            assert!(validate_region(region).is_err());
        }
        let (base, server) = super::super::test_support::server(vec![
            (
                200,
                json!({"status":0,"data":{"redirect":true,"region":"eu"}})
            );
            4
        ])
        .await;
        let mut client = Client::new(credentials()).expect("client");
        client.base_override = Some(base);
        assert_eq!(client.fetch().await, Err(Error::Protocol));
        assert_eq!(server.await.expect("server").len(), 4);
    }
    #[tokio::test]
    async fn terms_and_missing_people_require_user_action() {
        for (replies, error) in [
            (vec![(200, json!({"status":4,"data":{}}))], Error::Terms),
            (
                vec![(200, login()), (200, json!({"status":0,"data":[]}))],
                Error::NoPatients,
            ),
            (
                vec![
                    (200, login()),
                    (401, json!({})),
                    (200, login()),
                    (401, json!({})),
                ],
                Error::Credentials,
            ),
        ] {
            let (base, server) = super::super::test_support::server(replies).await;
            let mut client = Client::new(credentials()).expect("client");
            client.base_override = Some(base);
            assert_eq!(client.fetch().await, Err(error));
            server.await.expect("server");
        }
    }
}
