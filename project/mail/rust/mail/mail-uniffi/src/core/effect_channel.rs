//! A request→response channel for bridging a Rust caller to a foreign answerer.
//!
//! `#[uniffi::export(with_foreign)]` async traits crash intermittently on Kotlin while
//! lifting the foreign return value back into Rust. The working alternative — already used
//! for notifications via `WatchPrivacyInfoStream` — is a `#[derive(uniffi::Object)]` stream
//! the foreign side polls with `next_async`. That shape only covers fire-and-forget signals.
//!
//! Some foreign traits are *questions*: Rust asks for a value and must wait for it (device
//! info for request headers, a human-verification token, a DNS resolution). This channel
//! turns such a `(Req) -> Resp` call into "enqueue a [`Pending`] carrying a private one-shot
//! back-channel, then await the answer". The foreign side drives a loop that pulls each
//! [`Pending`], computes the answer, and sends it back through the one-shot — so the value
//! crosses the boundary as a method *argument* (the safe direction) instead of a foreign
//! async-trait return (the crashing direction).
//!
//! It is deliberately domain-free so every converted trait shares one implementation.

use flume::{Receiver, Sender};
use tokio::sync::oneshot;

/// Why a request or its answer could not be delivered.
///
/// All variants mean the same thing in practice — the loop on the other end is gone — but
/// each names which end disappeared so callers can log the right cause when they translate
/// the failure into their trait's own "no answer" value.
#[derive(Debug, thiserror::Error)]
pub enum EffectChannelError {
    /// The foreign loop dropped its [`EffectChannelHandler`]; no one will ever take requests.
    #[error("request stream is gone (handler dropped)")]
    HandlerGone,
    /// The requester dropped before its answer was sent (only the answerer observes this).
    #[error("requester is gone")]
    RequesterGone,
    /// The [`Pending`] was dropped without anyone calling `respond`.
    #[error("responder dropped before answering")]
    ResponderDropped,
}

/// One in-flight request paired with the private one-shot that carries its answer.
///
/// Handed to the answering side via [`EffectChannelHandler::next`]. Split it into the request
/// payload (safe to expose to the foreign side) and a [`Responder`] that owns just the
/// answer channel, so a `uniffi::Object` can hold the latter without the former.
pub struct Pending<Req, Resp> {
    request: Req,
    response: oneshot::Sender<Resp>,
}

impl<Req, Resp> Pending<Req, Resp> {
    /// Separate the request payload from its one-shot answer channel.
    pub fn split(self) -> (Req, Responder<Resp>) {
        (self.request, Responder { tx: self.response })
    }
}

/// The answering end of a single request: send exactly one [`respond`](Responder::respond).
pub struct Responder<Resp> {
    tx: oneshot::Sender<Resp>,
}

impl<Resp> Responder<Resp> {
    /// Deliver the answer to the waiting requester.
    ///
    /// Errors only if the requester already gave up and dropped its receiver.
    pub fn respond(self, resp: Resp) -> Result<(), EffectChannelError> {
        self.tx
            .send(resp)
            .map_err(|_| EffectChannelError::RequesterGone)
    }
}

/// The answering end of the channel, driven by the foreign loop.
///
/// Cloneable, though in practice exactly one loop calls [`next`](Self::next).
pub struct EffectChannelHandler<Req, Resp> {
    rx: Receiver<Pending<Req, Resp>>,
}

impl<Req, Resp> Clone for EffectChannelHandler<Req, Resp> {
    fn clone(&self) -> Self {
        Self {
            rx: self.rx.clone(),
        }
    }
}

impl<Req, Resp> EffectChannelHandler<Req, Resp> {
    /// Wait for the next request. Errors once the requester is dropped, which is the foreign
    /// loop's cue to stop.
    pub async fn next(&self) -> Result<Pending<Req, Resp>, EffectChannelError> {
        self.rx
            .recv_async()
            .await
            .map_err(|_| EffectChannelError::RequesterGone)
    }
}

/// The asking end of the channel, held by the Rust-side trait implementation.
pub struct EffectChannel<Req, Resp> {
    tx: Sender<Pending<Req, Resp>>,
}

impl<Req, Resp> EffectChannel<Req, Resp> {
    /// Create a connected requester/handler pair.
    ///
    /// The request queue is bounded to one: only the freshest unanswered request is worth
    /// holding, and the bound keeps a stalled foreign loop from letting requests pile up.
    pub fn new() -> (Self, EffectChannelHandler<Req, Resp>) {
        let (tx, rx) = flume::bounded(1);
        (Self { tx }, EffectChannelHandler { rx })
    }

    /// Ask for an answer and wait for it.
    ///
    /// There is no timeout: a slow foreign loop blocks the caller exactly as a slow foreign
    /// trait implementation did before, keeping the channel invisible to callers upstream.
    pub async fn request(&self, req: Req) -> Result<Resp, EffectChannelError> {
        let (rtx, rrx) = oneshot::channel();
        self.tx
            .send_async(Pending {
                request: req,
                response: rtx,
            })
            .await
            .map_err(|_| EffectChannelError::HandlerGone)?;

        rrx.await.map_err(|_| EffectChannelError::ResponderDropped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn request_resolves_to_the_responded_value() {
        let (channel, handler) = EffectChannel::<i32, String>::new();

        let caller = tokio::spawn(async move { channel.request(7).await });

        let (input, responder) = handler.next().await.unwrap().split();
        assert_eq!(input, 7);
        responder.respond(format!("answer:{input}")).unwrap();

        assert_eq!(caller.await.unwrap().unwrap(), "answer:7");
    }

    #[tokio::test]
    async fn concurrent_requests_each_get_their_own_answer_out_of_order() {
        let (channel, handler) = EffectChannel::<i32, i32>::new();
        let channel = Arc::new(channel);

        let a = tokio::spawn({
            let channel = Arc::clone(&channel);
            async move { channel.request(1).await }
        });
        let b = tokio::spawn(async move { channel.request(2).await });

        // Taking the first pending frees the bounded(1) slot for the second.
        let (in1, r1) = handler.next().await.unwrap().split();
        let (in2, r2) = handler.next().await.unwrap().split();

        // Respond in the opposite order they were received; each caller still gets its own.
        r2.respond(in2 * 10).unwrap();
        r1.respond(in1 * 10).unwrap();

        assert_eq!(a.await.unwrap().unwrap(), 10);
        assert_eq!(b.await.unwrap().unwrap(), 20);
    }

    #[tokio::test]
    async fn request_errors_when_handler_dropped() {
        let (channel, handler) = EffectChannel::<(), ()>::new();
        drop(handler);

        assert!(matches!(
            channel.request(()).await,
            Err(EffectChannelError::HandlerGone)
        ));
    }

    #[tokio::test]
    async fn request_errors_when_responder_dropped_without_answering() {
        let (channel, handler) = EffectChannel::<(), i32>::new();

        let caller = tokio::spawn(async move { channel.request(()).await });

        // Drop the pending (and thus the responder) without answering.
        drop(handler.next().await.unwrap());

        assert!(matches!(
            caller.await.unwrap(),
            Err(EffectChannelError::ResponderDropped)
        ));
    }

    #[tokio::test]
    async fn next_errors_when_requester_dropped() {
        let (channel, handler) = EffectChannel::<(), i32>::new();
        drop(channel);

        assert!(matches!(
            handler.next().await,
            Err(EffectChannelError::RequesterGone)
        ));
    }

    #[tokio::test]
    async fn respond_errors_when_requester_gave_up() {
        let (channel, handler) = EffectChannel::<(), i32>::new();

        let mut caller = Box::pin(channel.request(()));
        // One poll enqueues the request and parks on the response; then abandon the caller.
        assert!(futures::poll!(caller.as_mut()).is_pending());
        let (_, responder) = handler.next().await.unwrap().split();
        drop(caller);

        assert!(matches!(
            responder.respond(1),
            Err(EffectChannelError::RequesterGone)
        ));
    }
}
