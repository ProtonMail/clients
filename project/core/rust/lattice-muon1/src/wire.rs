use lattice::LatticeError;
use lattice::Sensitive;
use lattice::{LtWireMethod, LtWireRequest, LtWireRequestProvider, LtWireResponse};
use mail_muon::{ContentType, Method as MuonMethod, http::HttpReq, http::HttpRes};

/// [`LtWireRequestProvider`] implementation for the mail-muon transport.
pub struct Muon1WireRequestProvider;

impl LtWireRequestProvider for Muon1WireRequestProvider {
    type Request = HttpReq;
    type Response = HttpRes;
    type Error = LatticeError;

    fn from_wire(wire: LtWireRequest) -> Result<Self::Request, Self::Error> {
        let LtWireRequest {
            headers,
            method,
            path,
            query,
        } = wire;

        let muon_method = match &method {
            LtWireMethod::Get => MuonMethod::GET,
            LtWireMethod::Delete => MuonMethod::DELETE,
            LtWireMethod::Post(_) => MuonMethod::POST,
            LtWireMethod::Put(_) => MuonMethod::PUT,
        };

        let mut http_req = HttpReq::new(muon_method, path);

        for (k, v) in query {
            http_req = http_req.query((k, v.into_inner()));
        }

        for (k, v) in headers {
            http_req = http_req.header((k, v.into_inner()));
        }

        match method {
            LtWireMethod::Get | LtWireMethod::Delete => Ok(http_req),
            LtWireMethod::Post(body) | LtWireMethod::Put(body) => {
                // TODO: Content type JSON is not always correct here
                // This is the current behavior of the transport layer, but we should fix it.
                Ok(http_req.body(body.into_inner()).header(ContentType::JSON))
            }
        }
    }

    fn to_wire(res: Self::Response) -> Result<LtWireResponse, Self::Error> {
        let status = res.status().as_u16();
        let headers = res
            .headers()
            .iter()
            .filter_map(|(k, v)| {
                Some((
                    k.as_str().to_owned(),
                    Sensitive::new(v.to_str().ok()?.to_owned()),
                ))
            })
            .collect();
        let body = Sensitive::new(res.into_body());
        Ok(LtWireResponse {
            status,
            headers,
            body,
        })
    }
}
