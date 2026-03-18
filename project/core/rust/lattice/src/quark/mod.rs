pub mod encryption;
pub mod event;
pub mod jail;
pub mod user;

use std::str::FromStr;

use derive_more::{Deref, Display};

use crate::LatticeError;

pub trait LtQuarkContract {
    type Response: LtQuarkRes;

    const COMMAND_PATH: &'static str;

    fn params(&self) -> Result<QuarkCommand, LatticeError>;
}

#[cfg(feature = "muon")]
pub trait LtQuarkContractExt: LtQuarkContract {
    fn to_muon_req(&self) -> Result<::muon::http::HttpReq, LatticeError>;
    fn from_muon_res(response: &::muon::http::HttpRes) -> Result<Self::Response, LatticeError>;
}

#[cfg(feature = "muon")]
impl<T: LtQuarkContract> LtQuarkContractExt for T {
    fn to_muon_req(&self) -> Result<::muon::http::HttpReq, LatticeError> {
        let url = format!("/internal/quark/raw::{}", Self::COMMAND_PATH);
        let http_req = ::muon::http::HttpReq::new(::muon::http::Method::GET, url)
            .query(("strInput", self.params()?.as_command()));
        Ok(http_req)
    }

    fn from_muon_res(response: &::muon::http::HttpRes) -> Result<Self::Response, LatticeError> {
        let s = response.status().as_u16();

        if s != 200 {
            return Err(LatticeError::UnexpectedStatusCode(
                s,
                response.body().to_vec(),
            ));
        }

        let api_response = Self::Response::from_muon_res(response)?;
        Ok(api_response)
    }
}

#[derive(Deref)]
pub struct LtQuarkResTryFrom<T: FromStr<Err = LatticeError>>(pub T);

impl<T: FromStr<Err = LatticeError>> LtQuarkRes for LtQuarkResTryFrom<T> {
    fn from_muon_res(response: &::muon::http::HttpRes) -> Result<Self, LatticeError> {
        let body = response.body();
        let body_str = String::from_utf8_lossy(body);
        // Remove the trailing newline
        let body_str = body_str.trim_end_matches('\n');
        let api_response: T = T::from_str(body_str)?;
        Ok(LtQuarkResTryFrom(api_response))
    }
}

pub struct LtQuarkResString(pub String);

impl LtQuarkRes for LtQuarkResString {
    fn from_muon_res(response: &::muon::http::HttpRes) -> Result<Self, LatticeError> {
        let body = response.body();
        let api_response: String = String::from_utf8(body.to_vec())
            .map_err(|e| LatticeError::UnexpectedResponse(e.to_string()))?;
        Ok(LtQuarkResString(api_response))
    }
}

pub trait LtQuarkRes: Sized {
    fn from_muon_res(response: &::muon::http::HttpRes) -> Result<Self, LatticeError>;
}

impl<T: serde::de::DeserializeOwned> LtQuarkRes for LtQuarkJSONRes<T> {
    fn from_muon_res(response: &::muon::http::HttpRes) -> Result<Self, LatticeError> {
        let body = response.body();
        let api_response: T = serde_json::from_slice::<T>(body)
            .map_err(|e| LatticeError::SerdeJSON(e, String::from_utf8(body.to_vec()).ok()))?;
        Ok(LtQuarkJSONRes(api_response))
    }
}
#[derive(Debug, Clone, Copy, Deref)]
pub struct LtQuarkJSONRes<T: serde::de::DeserializeOwned>(pub T);

/// The format for displaying the user
#[derive(Debug, Display, Clone, Copy)]
pub enum LtQuarkFormat {
    #[display("text")]
    Text,
    #[display("json")]
    Json,
}

#[derive(Default)]
pub struct QuarkCommand {
    query: Vec<String>,
    value: Vec<String>,
}

impl QuarkCommand {
    pub fn query(mut self, k: impl ToString, v: impl ToString) -> Self {
        self.query.push(k.to_string());
        self.query.push(v.to_string());
        self
    }

    pub fn query_flag(mut self, k: impl ToString) -> Self {
        self.query.push(k.to_string());
        self
    }

    pub fn query_if_some(self, k: impl ToString, v: Option<impl ToString>) -> Self {
        if let Some(v) = v {
            self.query(k, v)
        } else {
            self
        }
    }

    pub fn value(mut self, v: impl ToString) -> Self {
        self.value.push(v.to_string());
        self
    }

    pub fn as_command(&self) -> String {
        let mut items = Vec::new();

        for item in &self.query {
            items.push(item.to_owned());
        }

        if !self.value.is_empty() {
            items.push("--".to_owned());
            items.extend(self.value.clone());
        }

        items.join(" ")
    }
}
