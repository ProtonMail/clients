use mail_api_shared::{ApiErrorInfo, ApiServiceError};
use mail_muon::{ContentType, Method as MuonMethod, Status, common::Sender, http};
use mail_muon::{ProtonRequest, ProtonResponse};

use ::mail_muon::http::{HttpReq, HttpRes};
use lattice::{
    LatticeError, LtApiResponseError, LtContract, LtRequestBody, LtRequestQueryParams,
    LtResponseBody, Method,
};

fn lattice_method_to_muon_method<T: LtRequestBody>(method: &Method<T>) -> MuonMethod {
    match method {
        Method::Get => MuonMethod::GET,
        Method::Delete => MuonMethod::DELETE,
        Method::Post(_) => MuonMethod::POST,
        Method::Put(_) => MuonMethod::PUT,
    }
}

fn lattice_request_to_muon_request<T: LtContract>(contract: T) -> Result<HttpReq, LatticeError> {
    let method = contract.method()?;
    let path = contract.path()?;
    let mut http_req = HttpReq::new(lattice_method_to_muon_method(&method), path);

    if let Some(query) = contract.query() {
        http_req = query
            .to_query_params()?
            .into_iter()
            .fold(http_req, |http_req, (k, v)| {
                http_req.query((k.into_owned(), v.into_inner()))
            })
    }

    http_req = contract
        .headers()?
        .into_iter()
        .fold(http_req, |http_req, header| http_req.header(header));

    if let Some(body) = method.into_body() {
        let body = body.to_body()?;

        http_req = http_req.body(body).header(ContentType::JSON);
    }

    Ok(http_req)
}

fn from_muon_res_to_lattice_error<T: LtContract>(
    response: HttpRes,
) -> Result<T::Response, LatticeError> {
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

fn lattice_error_to_api_error(value: LatticeError) -> ApiServiceError {
    match value {
        LatticeError::SerdeJSON(error, _) => ApiServiceError::UnknownError(error.to_string()),
        LatticeError::UnexpectedResponse(e) => ApiServiceError::ResponseError(e),
        LatticeError::UnexpectedStatusCode(code, _) => {
            ApiServiceError::UnknownError(format!("UnexpectedStatusCode {code}"))
        }
        LatticeError::ApiError(status_code, lt_api_response_error) => {
            let Ok(status) = http::Status::from_u16(status_code) else {
                return ApiServiceError::UnknownError("Invalid status code {status_code}".into());
            };

            // Easier to regenerate the json string, than to handle the conversion ourselves.
            let json =
                serde_json::to_string(lt_api_response_error.as_ref()).expect("Should not fail");
            let api_error =
                Some(serde_json::from_str::<ApiErrorInfo>(&json).expect("Should not fail"));

            match status {
                Status::BAD_REQUEST => ApiServiceError::BadRequest(json, api_error),
                Status::UNAUTHORIZED => ApiServiceError::Unauthorized(json, api_error),
                Status::FORBIDDEN => ApiServiceError::Forbidden(json, api_error),
                Status::NOT_FOUND => ApiServiceError::NotFound(json, api_error),
                Status::UNPROCESSABLE_ENTITY => {
                    ApiServiceError::UnprocessableEntity(json, api_error)
                }
                Status::TOO_MANY_REQUESTS => ApiServiceError::TooManyRequests(json, api_error),
                Status::INTERNAL_SERVER_ERROR => {
                    ApiServiceError::InternalServerError(json, api_error)
                }
                Status::NOT_IMPLEMENTED => ApiServiceError::NotImplemented(json, api_error),
                Status::BAD_GATEWAY => ApiServiceError::BadGateway(json, api_error),
                Status::SERVICE_UNAVAILABLE => ApiServiceError::ServiceUnavailable(json, api_error),
                code => ApiServiceError::OtherHttpError(code, json, api_error),
            }
        }
        LatticeError::SerdeQs(error) => ApiServiceError::QueryStringError(error),
        e => ApiServiceError::UnknownError(e.to_string()),
    }
}

#[allow(async_fn_in_trait)]
pub trait RunLatticeContractExt {
    async fn run_lattice_contract<T: LtContract>(
        &self,
        contract: T,
    ) -> Result<T::Response, LatticeError>;

    async fn run_lattice_contract_compat<T: LtContract>(
        &self,
        contract: T,
    ) -> Result<T::Response, ApiServiceError>;
}

impl<S: Sender<ProtonRequest, ProtonResponse>> RunLatticeContractExt for S {
    async fn run_lattice_contract<T: LtContract>(
        &self,
        contract: T,
    ) -> Result<T::Response, LatticeError> {
        let http_req = lattice_request_to_muon_request(contract)?;
        let resp = self.send(http_req).await.map_err(LatticeError::MailMuon)?;
        from_muon_res_to_lattice_error::<T>(resp)
    }

    async fn run_lattice_contract_compat<T: LtContract>(
        &self,
        contract: T,
    ) -> Result<T::Response, ApiServiceError> {
        let http_req =
            lattice_request_to_muon_request(contract).map_err(lattice_error_to_api_error)?;
        let resp = self.send(http_req).await?;
        from_muon_res_to_lattice_error::<T>(resp).map_err(lattice_error_to_api_error)
    }
}
