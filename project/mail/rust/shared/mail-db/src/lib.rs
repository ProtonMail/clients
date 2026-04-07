//! Database abstraction for hex componentens.
//!
//! See [RFC](../../../docs/rfcs/2026-03-27-storage.md) for more details.
//!

pub trait Transaction {
    type Error: std::error::Error + 'static;
}

pub trait ReadTx: Transaction {}
pub trait WriteTx: ReadTx {}

pub trait Database {
    type Error: std::error::Error + 'static;
    type ReadTx<'a>: ReadTx<Error = Self::Error> + 'a;
    type WriteTx<'a>: WriteTx<Error = Self::Error> + 'a;

    fn read<T, E: From<Self::Error>>(
        &self,
        closure: impl AsyncFnOnce(Self::ReadTx<'_>) -> Result<T, E>,
    ) -> impl Future<Output = Result<T, E>>;

    fn write<T, E: From<Self::Error>>(
        &self,
        closure: impl AsyncFnOnce(Self::WriteTx<'_>) -> Result<T, E>,
    ) -> impl Future<Output = Result<T, E>>;
}
