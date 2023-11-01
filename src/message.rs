use async_trait::async_trait;

use super::actor::Actor;

/// Trait for all actor messages
pub trait Message: Send {
    type Reply: Send;
}

/// Handler for a message.
///
/// When an actor wants to indicate that it can handle a particular message it implements this
/// trait, providing the functionality to handle the message.
#[async_trait]
pub trait Handler<M>
where
    M: Message,
    Self: Actor,
{
    async fn handle(&mut self, message: M) -> Result<M::Reply, Self::Error>;
}

#[macro_export]
macro_rules! messages {
    ($($msg:ty => $reply:ty),*) => {
        $(impl simple_actors::message::Message for $msg {
            type Reply = $reply;
        })*
    }
}
