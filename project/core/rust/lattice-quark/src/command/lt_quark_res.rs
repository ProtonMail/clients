use lattice::LatticeError;

/// Parses a successful Quark HTTP body into a typed response.
///
/// See [`super`] for the available adapters (`LtQuarkJSONRes`, `LtQuarkResTryFrom`, …) and when to
/// use each wire format.
pub trait LtQuarkRes: Sized {
    fn from_quark_body(body: &[u8]) -> Result<Self, LatticeError>;
}
