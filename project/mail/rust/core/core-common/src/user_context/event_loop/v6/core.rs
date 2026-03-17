mod event_provider;
mod event_source;
mod event_store;
mod event_subscriber;

pub use event_source::*;
pub use event_subscriber::*;

#[derive(Clone)]
pub struct CoreEventLoopV6Context;

impl CoreEventLoopV6Context {
    #[must_use]
    pub fn boxed(&self) -> Box<Self> {
        Box::new(self.clone())
    }
}
