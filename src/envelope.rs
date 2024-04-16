use std::pin::Pin;

use futures::Future;
use thiserror::Error;
use tokio::sync::oneshot;

use crate::{Actor, Handler, Message};

pub trait ToEnvelope<A, M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn envelope(message: M) -> Envelope<A>;
}

#[derive(Debug, Error)]
pub enum EnvelopeHandlerError<A>
where
    A: Actor,
{
    #[error("Ignored message: other end has probably dropped")]
    Ignored,
    #[error("Failed to send reply: other end has probably dropper")]
    SenderError,
    #[error("Error in actor: {0:?}")]
    ActorError(A::Error),
}

pub trait EnvelopeHandler<A: Actor> {
    fn handle<'a>(
        &'a mut self,
        actor: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = Result<(), EnvelopeHandlerError<A>>> + Send + 'a>>;
}

pub struct Envelope<A: Actor + ?Sized> {
    type_name: &'static str,
    handler: Box<dyn EnvelopeHandler<A> + Send>,
}

impl<A: Actor> Envelope<A> {
    pub fn new<M>(message: M, sender: Option<oneshot::Sender<M::Reply>>) -> Self
    where
        M: Message + 'static,
        A: Handler<M>,
    {
        Envelope {
            type_name: std::any::type_name::<M>(),
            handler: Box::new(EnvelopeHandlerProxy::new(message, sender)),
        }
    }

    pub fn type_name(&self) -> &str {
        self.type_name
    }
}

impl<A: Actor> EnvelopeHandler<A> for Envelope<A> {
    fn handle<'a>(
        &'a mut self,
        actor: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = Result<(), EnvelopeHandlerError<A>>> + Send + 'a>> {
        Box::pin(async { self.handler.handle(actor).await })
    }
}

pub struct EnvelopeHandlerProxy<M>
where
    M: Message,
{
    message: Option<M>,
    sender: Option<oneshot::Sender<M::Reply>>,
}

impl<M> EnvelopeHandlerProxy<M>
where
    M: Message + 'static,
{
    fn new(message: M, sender: Option<oneshot::Sender<M::Reply>>) -> Self {
        Self {
            message: Some(message),
            sender,
        }
    }
}

impl<A, M> EnvelopeHandler<A> for EnvelopeHandlerProxy<M>
where
    M: Message + 'static,
    A: Actor + Handler<M>,
{
    fn handle<'a>(
        &'a mut self,
        actor: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = Result<(), EnvelopeHandlerError<A>>> + Send + 'a>> {
        Box::pin(async move {
            let sender = self.sender.take();
            if sender.is_some() && sender.as_ref().unwrap().is_closed() {
                return Err(EnvelopeHandlerError::Ignored);
            }

            if let Some(message) = self.message.take() {
                let reply = <A as Handler<M>>::handle(actor, message)
                    .await
                    .map_err(|err| EnvelopeHandlerError::ActorError(err))?;

                if let Some(sender) = sender {
                    sender
                        .send(reply)
                        .map_err(|_| EnvelopeHandlerError::SenderError)?;
                }
            }

            Ok(())
        })
    }
}
