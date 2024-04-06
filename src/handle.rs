use async_trait::async_trait;
use std::fmt::Debug;

use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinSet,
};

use crate::{
    context::Context,
    envelope::{Envelope, EnvelopeHandler, EnvelopeHandlerError},
    Actor, Handler, Message,
};

#[derive(Debug, Clone, Error, PartialEq)]
pub enum SendError {
    /// This error arises when the actors message channel has been closed.
    ///
    /// Typically this is because the actor has terminated in some way before all the `Handle` have
    /// been dropped. This could be due to an irrecoverable error arising in the actor.
    #[error("Actor inbox channel was closed")]
    ChannelClosed,

    /// This error arises when the channel used to receive the reply has been closed.
    ///
    /// Typically this is because there was a problem with the actor being able to send the reply.
    #[error("Message reply channel was closed")]
    ReplyChannelClosed,
}

/// A handle to an actor
///
/// Actor handles can be cloned and passed between threads. This allows actors to pass handles to
/// other actors in messages.
///
/// Once all handles for an actor have been dropped, the actor will terminate its message loop and
/// shut down.
///
/// To keep a handle around without contributing to the actors lifetime, you can downgrade a
/// `Handle` to a [`WeakHandle`].
///
/// The `Handle` type is parameterized by the actor. If you want to keep a handle to any actor that
/// can receive a particular [`Message`], you can convert the `Handle` to a [`Recipient`].
pub struct Handle<A: Actor> {
    sender: mpsc::Sender<Envelope<A>>,
}

impl<A: Actor> Debug for Handle<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Handle<{}>", std::any::type_name::<A>())
    }
}

impl<A> Clone for Handle<A>
where
    A: Actor,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<A> Handle<A>
where
    A: Actor + 'static,
{
    pub(crate) fn run(context: Context, join_set: &mut JoinSet<()>, mut actor: A) -> Self {
        let (sender, mut receiver) = mpsc::channel::<Envelope<A>>(actor.mailbox_size());
        let handle = Self { sender };
        let name = actor.name();

        let task_handle = handle.clone();
        join_set.spawn(async move {
            tracing::info!(actor_name = name, "Starting actor");
            if let Err(err) = actor.started(context.clone(), task_handle).await {
                if !actor.is_recoverable(&err) {
                    tracing::error!(error = ?err, actor_name = name,
                        "Received irrecoverable error starting actor");
                    return;
                }

                tracing::warn!(error = ?err, actor_name = name,
                    "Received recoverable error starting actor");
            }

            let mut termination = "no more handles";
            while let Some(mut message) = receiver.recv().await {
                match message.handle(&mut actor).await {
                    Ok(_) => {}

                    // Special case to handle actor-specific errors. We want to make sure that the
                    // error is recoverable, in which case we continue with processing this actor.
                    // On the other handle, if the error is not recoverable, we want to terminate.
                    Err(EnvelopeHandlerError::ActorError(error)) => {
                        if !actor.is_recoverable(&error) {
                            termination = "irrecoverable error";
                            tracing::error!(error = ?error, actor_name = name,
                                            message_type = message.type_name(),
                                            "Received irrecoverable error in message handler");
                            break;
                        } else {
                            tracing::warn!(error = ?error, actor_name = actor.name(),
                            message_type = message.type_name(),
                            "Received recoverable error in message handler");
                        }
                    }

                    Err(EnvelopeHandlerError::Ignored) => {
                        tracing::warn!(
                            actor_name = name,
                            message_type = message.type_name(),
                            "Actor ignored message"
                        );
                    }

                    Err(EnvelopeHandlerError::SenderError) => {
                        tracing::warn!(
                            actor_name = name,
                            message_type = message.type_name(),
                            "Actor failed to send reply to message; other end probably dropped"
                        );
                    }
                }
            }

            tracing::info!(actor_name = name, "Actor has terminated ({termination})");

            if let Err(err) = actor.stopped().await {
                tracing::warn!(actor_name = name, error = ?err,
                               "Received error during termination of actor");
            }
        });

        handle
    }

    /// Send a message to the actor and wait for a reply.
    ///
    /// This method is used to send a message to the actor attached to this `Handle`. Once the
    /// message has been processed by the actor, this method will return the reply. If there was a
    /// problem communicating with the actor, a `SendError` is returned.
    ///
    /// This method will return an error of:
    ///
    /// 1. The actor's message loop has terminated, or
    /// 2. If the actor terminate its message loop before a reply could be sent.
    ///
    /// The latter case can arise when the actor encounters an error whilst processing a message,
    /// and that error is not recoverable.
    ///
    /// The reply is communicated with the recipient actor by way of a `oneshot` single-value
    /// channel. There are some semantics to be aware of:
    ///
    /// 1. If the sender drops the receive end of the oneshot before the message has been
    ///    dispatched to the actor, the message will be dropped before it is processed by the
    ///    actor. In which case, a warning will be logged indicating that the actor ignored the
    ///    message. If you intend to drop the receiver of a reply, but still want the message to be
    ///    communicated to the actor, consider the `Handler::post` method instead of this one.
    ///
    /// 2. If the sender drops the receive end of the oneshot after the message has has been
    ///    dispatched to the actor, the actor will be unable to send the reply. This is not
    ///    considered an error from the actor's point-of-view, and only a wanrning will be logged
    ///    indicating the actor failed to send a reply.
    pub async fn send<M>(&self, message: M) -> Result<M::Reply, SendError>
    where
        M: Message + 'static,
        A: Handler<M>,
    {
        let (sender, receiver) = oneshot::channel();
        let envelope = Envelope::new(message, Some(sender));

        self.sender
            .send(envelope)
            .await
            .map_err(|_| SendError::ChannelClosed)?;

        receiver.await.map_err(|_| SendError::ReplyChannelClosed)
    }

    /// Post a message to the actor, but do not wait for a reply.
    ///
    /// This method is used to send a message to the actor and discard any reply. This method will
    /// return an error if the actors message loop has terminated, but will not wait for the actor
    /// to process the message.
    pub async fn post<M>(&self, message: M) -> Result<(), SendError>
    where
        M: Message + 'static,
        A: Handler<M>,
    {
        let envelope = Envelope::new(message, None);
        self.sender
            .send(envelope)
            .await
            .map_err(|_| SendError::ChannelClosed)
    }

    /// Post a message to the actor, but do not wait for a reply (blocking).
    ///
    /// This method can be used to send a message to an actor and ignore a reply. This method will
    /// return an error if the actor's message loop has terminated.
    #[tracing::instrument(skip_all)]
    pub fn blocking_post<M>(&self, message: M) -> Result<(), SendError>
    where
        M: Message + 'static,
        A: Handler<M>,
    {
        let envelope = Envelope::new(message, None);
        self.sender
            .blocking_send(envelope)
            .map_err(|_| SendError::ChannelClosed)
    }

    /// Return a `Recipient` for a given message type.
    ///
    /// So long as the actor type `A` is able to process message `M` (i.e. there exists some
    /// `Handle<M>` implementation for `A`), then a `Recipient<M>` can be created.
    ///
    /// This is very useful when you have multiple actor types that can process a given message, and
    /// you need to register any of those actors in some way to receive that message.
    pub fn recipient<M>(&self) -> Recipient<M>
    where
        M: Message + 'static,
        A: Handler<M>,
    {
        Recipient {
            sender: self.boxed(),
        }
    }

    /// Downgrade this handle to a `WeakHandle`.
    ///
    /// A `WeakHandle` can be used much like a normal `Handle`, except it does not keep the actor
    /// alive. Once all the normal `Handle` have been dropped, regardless of any `WeakHandle`, the
    /// actor will terminate.
    ///
    /// To use a `WeakHandle` to send messages, it must be upgraded to a `Handle`.
    pub fn downgrade(&self) -> WeakHandle<A> {
        WeakHandle {
            sender: Some(self.sender.downgrade()),
        }
    }

    /// Check if the actor's mailbox has been closed.
    ///
    /// This can arise when there was an irrecoverable error in the actor, in which case the actor
    /// will have terminated before all `Handle` have been dropped.
    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }
}

/// A weak handle to an actor.
///
/// This weaker form of `Handle` will not cause the actor to stay alive: once all the normal
/// `Handle` have been dropped, regardless of any remaining `WeakHandle`, the actor will terminate.
///
/// This type is useful in three scenarios:
///
/// 1. Breaking cycles between actors. if two actors hold `Handle` to each other, they will never
///    terminate. By using a `WeakHandle`, the cycle is broken and the actors can terminate.
/// 2. Hold a weak reference to an actor. This can be useful when you need to send a message to an
///    actor, but you do not want to keep that actor alive unnecessarily.
/// 3. Allow an actor to maintain a reference to itself, maybe to be passed out later to other
///    actors, without creating an unnecessary self-reference cycle.
///
/// As an example, in the `Actor::started` method an actor receives a `Handle` to itself. It can
/// pass this `Handle` in messages to other actors so they can communicate with it. However, once
/// the `started` method has returned, there is no longer any means by which an actor can retrieve
/// a `Handle` to itself. If an actor still requires a self-referent `Handle`, it should not
/// naiively store the `Handle` it received in the `started` method in a field, as this will create
/// a cycle that keeps the actor alive. A better approach is to downgrade the `Handle` it receives
/// in the `Actor::started` method into a `WeakHandle` and store that in a field. The actor will
/// then be able to safely upgrade the `WeakHandle` into a normal `Handle` to use later on.
///
/// # Example
///
/// In the following example, an actor type called `MyActor` maintains a `WeakHandle` to itself. It
/// uses the `Default` trait implementation on `WeakHandle`, which creates an empty handle that
/// will never upgrade. During the `started` method of the `Actor` trait implementation, the actor
/// downgrades the `Handle` it receives to a `WeakHandle` and assigns it to the `self_handle`
/// field.
///
/// The `MyActor` actor is also able to handle a `SendMeYou` message, which has a reply type of
/// `Handle<MyActor>`. The handler of this message for `MyActor` attempts to upgrade the
/// `WeakHandle` it stored in the `self_handle` field, returning an error if the upgrade did not
/// succeed. If the upgrade was successful, the new `Handle` it send in reply.
///
/// ```
/// # use simple_actors::*;
/// # use thiserror::Error;
///
/// #[derive(Default)]
/// struct MyActor {
///   // A handle to this actor, in weak form, which does not create cycles.
///   self_handle: WeakHandle<Self>,
/// }
///
/// #[derive(Debug, Error)]
/// pub enum MyActorError {
///   #[error("Failed to upgrade self handle")]
///   FailedToUpgradeSelfHandle,
/// }
///
/// #[async_trait::async_trait]
/// impl Actor for MyActor {
///   type Error = MyActorError;
///
///   async fn started(&mut self, _: Context, handle: Handle<Self>) -> Result<(), Self::Error> {
///     self.self_handle = handle.downgrade();
///     Ok(())
///   }
/// }
///
/// struct SendMeYou;
///
/// impl Message for SendMeYou {
///   type Reply = Handle<MyActor>;
/// }
///
/// #[async_trait::async_trait]
/// impl Handler<SendMeYou> for MyActor {
///   async fn handle(&mut self, message: SendMeYou) -> Result<Handle<Self>, Self::Error> {
///     self.self_handle.upgrade().ok_or(MyActorError::FailedToUpgradeSelfHandle)
///   }
/// }
/// ```
pub struct WeakHandle<A: Actor> {
    sender: Option<mpsc::WeakSender<Envelope<A>>>,
}

/// A default value for a `WeakHandle`.
///
/// Often we want to store a `WeakHandle` in a struct, but are unable to initialize it during
/// construction of that structure. For this reason, a `WeakHandle` has a default _uninitialized_
/// state (basically the `None` value of an `Option`). This can be used to assign a value to
/// `WeakHandle` later on.
///
/// Attempting to upgrade a default value for a `WeakHandle` will always fail.
impl<A> Default for WeakHandle<A>
where
    A: Actor,
{
    fn default() -> Self {
        Self { sender: None }
    }
}

impl<A> Clone for WeakHandle<A>
where
    A: Actor,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<A> WeakHandle<A>
where
    A: Actor + 'static,
{
    /// Clear a `WeakHandle` so it no longer can be upgraded.
    pub fn clear(&mut self) {
        self.sender = None;
    }

    /// Upgrade to a normal `Handle`.
    ///
    /// This will attempt to convert the `WeakHandle` into a normal `Handle` that can be used to
    /// send messages to the actor. This will return `Some` if there are other `Handle` alove and
    /// the actor was not terminated; otherwise this method will return `None`.
    ///
    /// An additional case exists where a `Default` weak handle was created, or the `WeakHandle`
    /// was cleared. In such cases the `WeakHandle` will be empty, and upgrading will not succeed.
    pub fn upgrade(&self) -> Option<Handle<A>> {
        let sender = self.sender.clone()?;
        let sender = sender.upgrade()?;
        Some(Handle { sender })
    }

    /// Create a `WeakRecipient` from this `WeakHandle`.
    ///
    /// For the same reason that we may want to create a `Recipient` from a `Handle`, we can also
    /// create a `WeakRecipient` from a `WeakHandle`. The `WeakRecipient` will need to be upgraded
    /// to a `Recipient` before it can be used. Much like a `WeakHandle`, a `WeakRecipient` does not
    /// count as a reference to an actor until it is upgraded to a `Recipient`.
    pub fn recipient<M>(&self) -> WeakRecipient<M>
    where
        M: Message + 'static,
        A: Handler<M>,
    {
        WeakRecipient {
            upgradable: Some(self.boxed()),
        }
    }
}

// A utility trait used internally to allow us to form a 'Recipient'.
#[async_trait]
trait Sender<M>: Send
where
    M: Message + Send,
    M::Reply: Send,
{
    async fn send(&self, message: M) -> Result<M::Reply, SendError>;
    async fn post(&self, message: M) -> Result<(), SendError>;
    fn blocking_post(&self, message: M) -> Result<(), SendError>;
    fn boxed(&self) -> Box<dyn Sender<M> + Sync>;
    fn downgrade_recipient(&self) -> Box<dyn UpgradableRecipient<M> + Sync>;
}

// We make 'Handle' implement the 'Sender' trait so it can be boxed into a 'Recipient'.
#[async_trait]
impl<A, M> Sender<M> for Handle<A>
where
    A: Actor,
    M: Message + 'static,
    A: Handler<M>,
{
    async fn send(&self, message: M) -> Result<M::Reply, SendError> {
        self.send(message).await
    }

    async fn post(&self, message: M) -> Result<(), SendError> {
        self.post(message).await
    }

    fn blocking_post(&self, message: M) -> Result<(), SendError> {
        self.blocking_post(message)
    }

    fn boxed(&self) -> Box<dyn Sender<M> + Sync> {
        Box::new(self.clone())
    }

    fn downgrade_recipient(&self) -> Box<dyn UpgradableRecipient<M> + Sync> {
        Box::new(self.downgrade())
    }
}

/// A recipient of a message
///
/// This structure is a useful way to maintain a handle to an actor that can receive a certain
/// message without also knowning the type of that actor. A `Recipient` can be obtained from a
/// `Handle` using the `Handle::recipient` method.
pub struct Recipient<M>
where
    M: Message + Send,
    M::Reply: Send,
{
    sender: Box<dyn Sender<M> + Sync>,
}

impl<M: Message> Debug for Recipient<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Recipient<{}>", std::any::type_name::<M>(),)
    }
}

impl<M> Clone for Recipient<M>
where
    M: Message + Send,
    M::Reply: Send,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.boxed(),
        }
    }
}

impl<M> Recipient<M>
where
    M: Message + Send,
    M::Reply: Send,
{
    /// Send a message to the recipient actor and wait for a reply.
    ///
    /// This method is used to send a message of this type to the actor attached to this
    /// `Recipient`. Once the message has been processed by the actor, this method will return the
    /// reply. If there was a problem communicating with the actor, a `SendError` is returned.
    pub async fn send(&self, message: M) -> Result<M::Reply, SendError> {
        self.sender.send(message).await
    }

    /// Post a message to the recipient actor, but do not wait for a reply.
    ///
    /// This method is used to send a message to the actor and discard any reply. This method will
    /// return an error if the actors message loop has terminated, but will not wait for the actor
    /// to process the message.
    pub async fn post(&self, message: M) -> Result<(), SendError> {
        self.sender.post(message).await
    }

    /// Post a message to the recipient actor, but do not wait for a reply (blocking).
    ///
    /// This method can be used to send a message of this type to an actor and ignore a reply. This
    /// method will return an error if the actor's message loop has terminated.
    pub fn blocking_post(&self, message: M) -> Result<(), SendError> {
        self.sender.blocking_post(message)
    }

    /// Downgrade this reciepient into a `WeakRecipient`.
    pub fn downgrade(&self) -> WeakRecipient<M> {
        WeakRecipient {
            upgradable: Some(self.sender.downgrade_recipient()),
        }
    }
}

trait UpgradableRecipient<M>: Send
where
    M: Message + Send,
    M::Reply: Send,
{
    fn boxed(&self) -> Box<dyn UpgradableRecipient<M> + Sync>;
    fn upgrade_recipient(&self) -> Option<Recipient<M>>;
}

impl<A, M> UpgradableRecipient<M> for WeakHandle<A>
where
    A: Actor,
    M: Message + 'static,
    A: Handler<M>,
{
    fn boxed(&self) -> Box<dyn UpgradableRecipient<M> + Sync> {
        Box::new(self.clone())
    }

    fn upgrade_recipient(&self) -> Option<Recipient<M>> {
        let handle = self.upgrade()?;
        Some(handle.recipient())
    }
}

/// A weak recipient of a message.
///
/// This weaker form of `Recipient` will not cause the actor to stay alive: once all the normal
/// `Handle` and `Recipient` have been dropped, regardless of any remaining `WeakRecipient`, the
/// actor will terminate.
///
/// You can upgrade a `WeakRecipient` back into a [`Recipient`] so long as the actor remains alive.
pub struct WeakRecipient<M>
where
    M: Message + Send,
    M::Reply: Send,
{
    upgradable: Option<Box<dyn UpgradableRecipient<M> + Sync>>,
}

impl<M> Default for WeakRecipient<M>
where
    M: Message + Send,
    M::Reply: Send,
{
    fn default() -> Self {
        Self { upgradable: None }
    }
}

impl<M> Clone for WeakRecipient<M>
where
    M: Message + Send,
    M::Reply: Send,
{
    fn clone(&self) -> Self {
        Self {
            upgradable: if let Some(upgradable) = &self.upgradable {
                Some(upgradable.boxed())
            } else {
                None
            },
        }
    }
}

impl<M> WeakRecipient<M>
where
    M: Message + Send,
    M::Reply: Send,
{
    /// Clear a `WeakRecipient` so it no longer can be upgraded.
    pub fn clear(&mut self) {
        self.upgradable = None;
    }

    /// Upgrade a weak recipient to a normal `Recipient`.
    ///
    /// This will attempt to convert the `WeakRecipient` into a normal `Recipient` that can be used
    /// to send messages to the actor. This will return `Some` if the actor was not terminated;
    /// otherwise this method will return `None`.
    ///
    /// An additional case exists where a `Default` weak recipient was created, or the
    /// `WeakRecipient` was cleared. In such cases the `WeakRecipient` will be empty, and upgrading
    /// will not succeed.
    pub fn upgrade(&self) -> Option<Recipient<M>> {
        let Some(ref upgradable) = self.upgradable else {
            return None;
        };

        upgradable.upgrade_recipient()
    }
}
