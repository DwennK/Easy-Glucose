use crate::i18n::Text;
pub mod libre;
pub mod nightscout;

use crate::{core::Reading, dexcom};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provider {
    #[default]
    Dexcom,
    LibreLinkUp,
    Nightscout,
}
impl Provider {
    pub const ALL: [Self; 3] = [Self::Dexcom, Self::LibreLinkUp, Self::Nightscout];
    pub fn label(self) -> &'static str {
        match self {
            Self::Dexcom => "Dexcom Share",
            Self::LibreLinkUp => "FreeStyle Libre · LibreLinkUp",
            Self::Nightscout => "Nightscout / Nocturne",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Dexcom => "dexcom-share",
            Self::LibreLinkUp => "librelinkup",
            Self::Nightscout => "nightscout",
        }
    }
    pub fn interval(self) -> u64 {
        if self == Self::Dexcom { 300 } else { 60 }
    }
}

// Never derive Debug: these values contain passwords or access tokens.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "provider", content = "credentials")]
pub enum Credentials {
    Dexcom(dexcom::Credentials),
    LibreLinkUp(libre::Credentials),
    Nightscout(nightscout::Credentials),
}
impl Credentials {
    pub fn provider(&self) -> Provider {
        match self {
            Self::Dexcom(_) => Provider::Dexcom,
            Self::LibreLinkUp(_) => Provider::LibreLinkUp,
            Self::Nightscout(_) => Provider::Nightscout,
        }
    }
    /// Bind cached readings to this exact account/person, without writing its identity in clear.
    pub fn cache_identity(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(self.provider().key());
        let mut part = |s: &str| {
            hash.update([0]);
            hash.update(s.as_bytes());
        };
        match self {
            Self::Dexcom(c) => {
                part(&c.username);
                part(c.region.base_url());
            }
            Self::LibreLinkUp(c) => {
                part(&c.username);
                part(c.patient_id.as_deref().unwrap_or(""));
            }
            Self::Nightscout(c) => {
                part(&c.url);
                part(&c.token);
                part(match c.auth {
                    nightscout::Auth::Token => "token",
                    nightscout::Auth::ApiSecret => "secret",
                });
            }
        }
        format!("{:x}", hash.finalize())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patient {
    pub id: String,
    pub name: String,
}

pub enum Client {
    Dexcom(dexcom::Client),
    LibreLinkUp(libre::Client),
    Nightscout(nightscout::Client),
}
impl Client {
    pub fn new(credentials: Credentials) -> Result<Self, Error> {
        match credentials {
            Credentials::Dexcom(c) => dexcom::Client::new(c).map(Self::Dexcom),
            Credentials::LibreLinkUp(c) => libre::Client::new(c).map(Self::LibreLinkUp),
            Credentials::Nightscout(c) => nightscout::Client::new(c).map(Self::Nightscout),
        }
    }
    pub fn credentials(&self) -> Credentials {
        match self {
            Self::Dexcom(c) => Credentials::Dexcom(c.credentials().clone()),
            Self::LibreLinkUp(c) => Credentials::LibreLinkUp(c.credentials().clone()),
            Self::Nightscout(c) => Credentials::Nightscout(c.credentials().clone()),
        }
    }
    pub fn patients(&self) -> Vec<Patient> {
        if let Self::LibreLinkUp(c) = self {
            c.patients.clone()
        } else {
            vec![]
        }
    }
    pub fn selected_patient(&self) -> Option<String> {
        if let Self::LibreLinkUp(c) = self {
            c.selected_name()
        } else {
            None
        }
    }
    pub fn select_patient(&mut self, id: &str) -> Result<(), Error> {
        if let Self::LibreLinkUp(c) = self {
            c.select_patient(id)
        } else {
            Err(Error::SelectPatient)
        }
    }
    pub async fn fetch(&mut self) -> Result<Vec<Reading>, Error> {
        match self {
            Self::Dexcom(c) => c.fetch().await,
            Self::LibreLinkUp(c) => c.fetch().await,
            Self::Nightscout(c) => c.fetch().await,
        }
    }
}

pub(super) fn http_client() -> Result<reqwest::Client, Error> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("EasyGlucose/0.1")
        .build()
        .map_err(|_| Error::Network)
}
pub(super) async fn json_response(
    mut response: reqwest::Response,
) -> Result<serde_json::Value, Error> {
    match response.status().as_u16() {
        200..=299 => {}
        401 | 403 => return Err(Error::Credentials),
        429 => return Err(Error::RateLimited),
        500..=599 => return Err(Error::Server),
        _ => return Err(Error::Protocol),
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Error::Network)? {
        if bytes.len() + chunk.len() > 4_194_304 {
            return Err(Error::Protocol);
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{}", Text::AuthError.english())]
    Credentials,
    #[error("{}", Text::SessionExpired.english())]
    Session,
    #[error("{}", Text::TooMany.english())]
    RateLimited,
    #[error("{}", Text::RetryNetwork.english())]
    Network,
    #[error("{}", Text::RetryProvider.english())]
    Server,
    #[error("{}", Text::InvalidResponse.english())]
    Protocol,
    #[error(
        "{}", Text::AcceptTerms.english()
    )]
    Terms,
    #[error(
        "{}", Text::NoPerson.english()
    )]
    NoPatients,
    #[error("{}", Text::ChoosePerson.english())]
    SelectPatient,
    #[error(
        "{}", Text::InvalidUrl.english()
    )]
    InvalidUrl,
}
impl Error {
    pub fn from_code(code: &str, status: u16) -> Self {
        match code {
            "AccountPasswordInvalid"
            | "SSO_AuthenticatePasswordInvalid"
            | "SSO_AuthenticateAccountNotFound" => Self::Credentials,
            "SessionIdNotFound" | "SessionNotValid" => Self::Session,
            "SSO_AuthenticateMaxAttemptsExceeded" | "SSO_AuthenticateMaxAttemptsExceeed" => {
                Self::RateLimited
            }
            _ => match status {
                401 | 403 => Self::Credentials,
                429 => Self::RateLimited,
                500..=599 => Self::Server,
                _ => Self::Protocol,
            },
        }
    }
}

impl Error {
    pub fn requires_action(&self) -> bool {
        matches!(
            self,
            Self::Credentials
                | Self::Terms
                | Self::NoPatients
                | Self::SelectPatient
                | Self::InvalidUrl
        )
    }
}

#[cfg(test)]
pub(crate) mod test_support;
