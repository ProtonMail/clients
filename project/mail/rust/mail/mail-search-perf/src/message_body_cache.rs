//! Optional substitute message bodies on the fetch path: [`FixtureBodiesMessageBodyCache`] for harness builds, [`NoSubstituteMessageBodyCache`] otherwise.

use crate::{FixtureError, SubstituteBody, try_substitute_perf_body};

/// Optional substitute body for the message fetch path (perf fixtures).
pub trait MessageBodyCache: Send + Sync {
    fn try_substitute_body(&self, remote_id: &str) -> Result<Option<SubstituteBody>, FixtureError>;
}

/// Production / default: never substitute
#[derive(Clone, Copy, Debug, Default)]
pub struct NoSubstituteMessageBodyCache;

impl MessageBodyCache for NoSubstituteMessageBodyCache {
    fn try_substitute_body(
        &self,
        _remote_id: &str,
    ) -> Result<Option<SubstituteBody>, FixtureError> {
        Ok(None)
    }
}

/// Perf / historic-load simulation.
#[derive(Clone, Copy, Debug, Default)]
pub struct FixtureBodiesMessageBodyCache;

impl MessageBodyCache for FixtureBodiesMessageBodyCache {
    fn try_substitute_body(&self, remote_id: &str) -> Result<Option<SubstituteBody>, FixtureError> {
        try_substitute_perf_body(remote_id)
    }
}
