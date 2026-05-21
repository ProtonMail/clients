use lattice::LtTransportProvider;
use lattice_muon1::Muon1Transport;
use mail_api_shared::{ApiErrorInfo, ApiServiceError};
use mail_muon::common::Sender;
use mail_muon::{ProtonRequest, ProtonResponse, Status, http};

use lattice::{LatticeError, LtContract};
pub use lattice_muon1::LtTransportError;

pub fn lattice_error_to_api_error(value: LatticeError) -> ApiServiceError {
    match value {
        LatticeError::SerdeJSON(error, _) => ApiServiceError::UnknownError(error.to_string()),
        LatticeError::UnexpectedResponse(e) => ApiServiceError::ResponseError(e),
        LatticeError::UnexpectedStatusCode(code, _) => {
            ApiServiceError::UnknownError(format!("UnexpectedStatusCode {code}"))
        }
        LatticeError::SerdeQs(e) => ApiServiceError::UnknownError(e.to_string()),
        LatticeError::ApiError(status_code, lt_api_response_error) => {
            let Ok(status) = http::Status::from_u16(status_code) else {
                return ApiServiceError::UnknownError("Invalid status code {status_code}".into());
            };

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
        LatticeError::Other(s) => ApiServiceError::UnknownError(s),
    }
}

fn lattice_muon1_error_to_api_error(value: LtTransportError) -> ApiServiceError {
    match value {
        LtTransportError::Lattice(e) => lattice_error_to_api_error(e),
        LtTransportError::Transport(e) => ApiServiceError::from(e),
    }
}

#[allow(async_fn_in_trait)]
pub trait RunLatticeContractExt {
    async fn run_lattice_contract<T: LtContract>(
        &self,
        contract: T,
    ) -> Result<T::Response, LtTransportError>;

    async fn run_lattice_contract_compat<T: LtContract>(
        &self,
        contract: T,
    ) -> Result<T::Response, ApiServiceError>;
}

impl<S: ?Sized + Sender<ProtonRequest, ProtonResponse> + Send + Sync> RunLatticeContractExt for S {
    async fn run_lattice_contract<T: LtContract>(
        &self,
        contract: T,
    ) -> Result<T::Response, LtTransportError> {
        Muon1Transport::new(self)
            .send_contract_request(&contract)
            .await
    }

    async fn run_lattice_contract_compat<T: LtContract>(
        &self,
        contract: T,
    ) -> Result<T::Response, ApiServiceError> {
        self.run_lattice_contract(contract)
            .await
            .map_err(lattice_muon1_error_to_api_error)
    }
}
