use crate::{context::Context, Handle};

/// Trait for all actors.
///
/// Any type that needs to be an actor should implement this trait.
pub trait Actor: Send + Sized + 'static {
    /// The error type for the actor.
    ///
    /// When an actor handles a message, it can return this error type to indicate that the message
    /// could not be handled. Errors are specific to actors, as opposed to return values that are
    /// specific to messages.
    type Error: std::fmt::Debug + Send;

    /// The arguments to the actor creation.
    ///
    /// When the actor is being created, this type should enclose the arguments that are required
    /// to create the actor. These arguments, along with the actor [`Context`] and [`Handle`] will
    /// be passed to the [`Actor::create`] method.
    type Args: Send;

    /// Get the mailbox size for this actor (default: 8).
    ///
    /// This controls how many messages can be kept in the actors mailbox. Once the actors mailbox
    /// is full, any attempt to send a message to the actor will block until the actor processes
    /// some of the messages in the mailbox.
    fn mailbox_size() -> usize {
        8
    }

    /// Get the name of the actor.
    ///
    /// By default this will return the actor's type name. This is really only used in logging.
    fn name(&self) -> String {
        std::any::type_name::<Self>().into()
    }

    /// The actor is created.
    ///
    /// This method will be called on the actor before entering into the message loop. At this
    /// point, the actor can perform any asynchronous actions required when it is started, such
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
    /// The actor also receives the `Context` in which it will be running. This can be cloned and
    /// retained if the actor needs to spawn new actors at any time.
    ///
    /// If this method returns an error, the `is_recoverable` method is checked to see if the error
    /// is recoverable or not. If not, the actor will not be started: the message loop will not be
    /// entered into, and the `stopped()` method will be called.
    fn create(
        context: Context,
        handle: Handle<Self>,
        args: Self::Args,
    ) -> impl std::future::Future<Output = Result<Self, Self::Error>> + std::marker::Send;

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
    fn stopped(
        self,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + std::marker::Send {
        async { Ok(()) }
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
