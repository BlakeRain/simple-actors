use async_trait::async_trait;

use crate::{context::Context, Handle};

#[async_trait]
pub trait Actor: Send + Sized + 'static {
    type Error: std::fmt::Debug + Send;

    /// Get the mailbox size for this actor (default: 8).
    fn mailbox_size(&self) -> usize {
        8
    }

    /// Get the name of the actor.
    ///
    /// By default this will return the actor's type name.
    fn name(&self) -> String {
        std::any::type_name::<Self>().into()
    }

    /// The actor is started.
    ///
    /// This method will be called on the actor before entering into the message loop. At this
    /// point, the actor can prefoerm any asynchronous actions required when it is started, such
    /// as sending messages to other actors and so on.
    ///
    /// It is also at this point that the actor receives a copy of a `Handle` to itself. This
    /// handle can be used to send messages to the actor. The actor is able to clone this handle
    /// and send it to other actors when it has started.
    ///
    /// Note: Do not retain the `Handle` to this actor (received by this function) within the actor
    /// itself. If you do, this will meant that the actor will never terminate as it retains a
    /// reference to itself. If you need to store a self-reference, downgrade the `Handle` into a
    /// `WeakHandle` and store that instead.
    ///
    /// If this method returns an error, the `is_recoverable` method is checked to see if the error
    /// is recoverable or not. If not, the actor will not be started: the message loop will not be
    /// entered into, and the `stopped()` method will be called.
    #[allow(unused_variables)]
    async fn started(
        &mut self,
        context: Option<Context>,
        handle: Handle<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Called when the actor has stopped.
    ///
    /// This method is invoked after an actor's message loop has terminated when there are no more
    /// active handles. That is to say, ince all `Handle` have been dropped, the `Actor` will
    /// terminate it's message loop.
    ///
    /// If this method returns an error it is logged but ignored; the actor will still terminate.
    ///
    /// Note that this method receives ownership of the actor, which usually simplifies actor
    /// clean-up: it is within this method that that the actor is dropped, and you know that your
    /// actor object is never going to be reused.
    async fn stopped(self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Check if an error is recoverable (default is no).
    ///
    /// This method is invoked whenever an error is returned from the actor, either in the `started`
    /// method ir by the `Handler::handle` method. If this method returns `true`, the actor will
    /// simply log the error message as a warning and then continue operation. If this method
    /// returns `false` (which is the default), the actor is terminated.
    #[allow(unused_variables)]
    fn is_recoverable(&mut self, error: &Self::Error) -> bool {
        false
    }
}
