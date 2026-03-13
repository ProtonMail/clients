use muon::{
    Session,
    common::ContentType,
    http::{HttpReq, HttpRes, Method as MuonMethod},
};

use crate::{
    LatticeContract, LatticeError, LtApiResponseError, Method, api_definitions::LtApiResponse,
};

impl<T: serde::Serialize> Method<T> {
    fn as_muon_method(&self) -> MuonMethod {
        match self {
            Self::Get => MuonMethod::GET,
            Self::Delete => MuonMethod::DELETE,
            Self::Post(_) => MuonMethod::POST,
            Self::Put(_) => MuonMethod::PUT,
        }
    }
}

pub trait LatticeExt: LatticeContract + Sized {
    fn send_with<C: muon::Context + Send + Sync>(
        &self,
        session: Session<C>,
    ) -> impl Future<Output = Result<Self::Response, LatticeError>> {
        async move {
            let http_req = as_muon_req(self)?;
            let response = session.send(http_req).await.map_err(LatticeError::Muon)?;
            from_muon_res::<Self>(&response)
        }
    }
}

impl<T: LatticeContract + Sized> LatticeExt for T {}

pub fn from_muon_res<T: LatticeContract>(response: &HttpRes) -> Result<T::Response, LatticeError> {
    let s = response.status().as_u16();

    if (200..300).contains(&s) {
        let body = response.body();
        // TODO: Handle status code and proton status codes.

        let api_response: LtApiResponse<T::Response> =
            serde_json::from_slice::<LtApiResponse<T::Response>>(body)
                .map_err(|e| LatticeError::SerdeJSON(e, String::from_utf8(body.to_vec()).ok()))?;

        return Ok(api_response.body);
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

pub fn as_muon_req(contract: &impl LatticeContract) -> Result<HttpReq, LatticeError> {
    let method = contract.method()?;
    let path = contract.path()?;
    let mut http_req = HttpReq::new(method.as_muon_method(), path);

    if let Some(query) = contract.query()? {
        http_req = query
            .into_iter()
            .fold(http_req, |http_req, query| http_req.query(query))
    }

    http_req = contract
        .headers()?
        .into_iter()
        .fold(http_req, |http_req, header| http_req.header(header));

    if let Some(body) = method.into_body() {
        let body = serde_json::to_vec(&body).map_err(|e| LatticeError::SerdeJSON(e, None))?;

        http_req = http_req.body(body).header(ContentType::JSON);
    }

    Ok(http_req)
}
