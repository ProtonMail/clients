use std::collections::HashMap;

use lattice::{
    LatticeError, Sensitive,
    transport::{LtWireMethod, LtWireRequest},
};

use crate::LtQuarkContract;

/// Encode a Quark contract as a lattice wire request.
pub trait LtQuarkWireExt: LtQuarkContract {
    fn to_wire_request(&self) -> Result<LtWireRequest, LatticeError> {
        let path = format!("/internal/quark/raw::{}", Self::COMMAND_PATH);
        let params = self.params()?.as_command();
        Ok(LtWireRequest {
            headers: HashMap::new(),
            method: LtWireMethod::Get,
            path,
            query: vec![("strInput".to_string(), Sensitive::new(params))],
        })
    }
}

impl<T: LtQuarkContract> LtQuarkWireExt for T {}
