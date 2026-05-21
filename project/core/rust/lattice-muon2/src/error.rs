use derive_more::{Display, Error, From};

/// Transport or Lattice-layer failure when using Muon v2 with Lattice contracts.
#[derive(Debug, Display, From, Error)]
pub enum LtTransportError {
    #[display("{_0}")]
    Transport(#[from] muon::Error),
    #[display("{_0}")]
    Lattice(#[from] lattice::LatticeError),
}
