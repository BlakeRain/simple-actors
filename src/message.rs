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

/// Define a load of `Message` traits.
///
/// This macro makes it easier to deal with lots of implementations of the [`Message`] trait. It is
/// quite common that you will end up with a lot of message types that need to implement the
/// `Message` trait. Even though the `Message` trait is very simple, it still takes a lot of code
/// to implement every time. This macro helps save time by providing a convenient short-hand for
/// defining these trait implementations.
///
/// ```ignore
/// messages! {
///   Ping => usize,
///   Bar => ()
/// }
/// ```
#[macro_export]
macro_rules! messages {
    ($($msg:ty => $reply:ty),*) => {
        $(impl simple_actors::message::Message for $msg {
            type Reply = $reply;
        })*
    }
}
