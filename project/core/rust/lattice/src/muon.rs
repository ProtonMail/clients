use muon::{
    Session,
    common::ContentType,
    http::{HttpReq, Method as MuonMethod},
};

use crate::{LatticeError, LtApiResponseError, LtContract, LtRequestBody, LtResponseBody, Method};

impl<T: LtRequestBody> Method<T> {
    fn as_muon_method(&self) -> MuonMethod {
        match self {
            Self::Get => MuonMethod::GET,
            Self::Delete => MuonMethod::DELETE,
            Self::Post(_) => MuonMethod::POST,
            Self::Put(_) => MuonMethod::PUT,
        }
    }
}

pub trait LatticeExt: LtContract + Sized {
    fn send_with<C: muon::Context + Send + Sync>(
        &self,
        session: Session<C>,
    ) -> impl Future<Output = Result<Self::Response, LatticeError>> {
        async move {
            let http_req = self.to_muon_req()?;
            let response = session.send(http_req).await.map_err(LatticeError::Muon)?;
            Self::from_muon_res(&response)
        }
    }
}

impl<T: LtContract + Sized> LatticeExt for T {}

pub trait LtContractExt: LtContract {
    fn to_muon_req(&self) -> Result<::muon::http::HttpReq, LatticeError>;

    fn from_muon_res(res: &::muon::http::HttpRes) -> Result<Self::Response, LatticeError>;
}

impl<T: LtContract> LtContractExt for T {
    fn to_muon_req(&self) -> Result<::muon::http::HttpReq, LatticeError> {
        let method = self.method()?;
        let path = self.path()?;
        let mut http_req = HttpReq::new(method.as_muon_method(), path);

        if let Some(query) = self.query()? {
            http_req = query
                .into_iter()
                .fold(http_req, |http_req, query| http_req.query(query))
        }

        http_req = self
            .headers()?
            .into_iter()
            .fold(http_req, |http_req, header| http_req.header(header));

        if let Some(body) = method.into_body() {
            let body = body.to_body()?;

            http_req = http_req.body(body).header(ContentType::JSON);
        }

        Ok(http_req)
    }

    fn from_muon_res(response: &::muon::http::HttpRes) -> Result<Self::Response, LatticeError> {
        let s = response.status().as_u16();

        // 200-300 are success codes
        // 300-304 are redirect codes
        if (200..=304).contains(&s) {
            let body = response.body();
            return T::Response::from_body(body);
        }

        if (400..500).contains(&s) {
            let body = response.body();

            let value: LtApiResponseError = serde_json::from_slice::<LtApiResponseError>(body)
                .map_err(|e| LatticeError::SerdeJSON(e, String::from_utf8(body.to_vec()).ok()))?;

            return Err(LatticeError::ApiError(s, Box::new(value)));
        }

        Err(LatticeError::UnexpectedStatusCode(
            s,
            response.body().to_vec(),
        ))
    }
}
