//! A simple actor framework for Rust.
//!
//! This library provides a few simple mechanisms for creating actors based on sending messages to
//! each others mailboxes and processing those messages. The framework is designed to be as easy to
//! use as possible, with a simple API.
//!
//! **Note**: This library makes use of [async traits](https://crates.io/crates/async-trait) to
//! provide a simple way to define async functions in traits. If you don't like this, or need to
//! avoid the minor performance penalty from dynamic dispatch, this might not be the correct actor
//! library for you.
//!
//! There are three types that play key roles in the framework:
//!
//! 1. The [`Actor`] trait, which provides some key functions for an actor.
//! 2. The [`Handler`] trait, by which actors implement their [`Message`] handling.
//! 3. The [`Handle`] type, which is used to send messages to actors.
//!
//! Defining an actor is as simple as implementing the [`Actor`] trait for some type.
//!
//! ```ignore
//! #[derive(Default)]
//! struct Pinger {
//!   // How many pings I got.
//!   count: usize,
//! }
//!
//! impl Actor for Pinger {
//!   type Error = String;
//! }
//! ```
//!
//! To make actors do anything they need to be able to process _messages_. Each message that can be
//! sent to an actor is a type that implements the [`Message`] trait. This trait has an associated
//! type `Reply` that is used to indicate the type of reply that should be returned from an actor
//! that handled the message.
//!
//! ```ignore
//! struct Ping;
//!
//! impl Message for Ping {
//!   type Reply = usize;
//! }
//! ```
//!
//! For an actor to handle a message it must implement the [`Handler`] trait for that message type.
//!
//! ```ignore
//! #[async_trait]
//! impl Handler<Ping> for Pinger {
//!   async fn handle(&mut self, _: Ping) -> Result<usize, Self::Error> {
//!     self.count += 1;
//!     Ok(self.count)
//!   }
//! }
//! ```
//!
//! Spawning an actor is done with a [`Context`] instance, which is used to spawn actors and manage
//! their lifetimes. The [`Context`] type provides a number of functions for spawning actors and
//! waiting for them to finish (or terminating them all).
//!
//! When a new actor is spawned, a [`Handle`] instance is returned. This handle can be used to send
//! messages to the actor. The handle can also be cloned and passed in messages to other actors.
//! The actor will stay alive whilst there is at least one remaining `Handle`. Once all handles
//! have been dropped, the actor will terminate.
//!
//! ```ignore
//! #[tokio::main]
//! async fn main() {
//!   // Create a new context and spawn a new Pinger actor.
//!   let ctx = Context::default();
//!   let hdl = ctx.spawn_default::<Pinger>().await;
//!
//!   // Send a Ping message to the actor and make sure we get our expected response.
//!   let res = hdl.send(Ping).await.unwrap();
//!   assert_eq!(res, 1);
//!
//!   // Drop the handle, which will cause the actor to terminate.
//!   drop(hdl);
//!
//!   // Wait for all actors to finish.
//!   ctx.wait_all().await;
//! }
//! ```
//!
//! ## Some Notes
//!
//! 1. The [`Handle`] type has a number of functions for sending messages and waiting for the actor
//!    to reply, or just posting a message without waiting for the reply.
//! 1. Sometimes you have a lot of type that you need to implement the [`Message`] trait for. Even
//!    though the `Message` trait is really simple, it can get tedious. There is a [`messages!`]
//!    macro that can make this a lot easier.
//! 1. Actors can return errors when handling messages. By default this will cause the actor to
//!    die. However, sometimes this is not desirable. You can override behaviours like this in the
//!    implementation of the [`Actor`] trait.
//! 1. Sometimes you might want to be able to keep hold of a `Handle` without contributing to the
//!    actors lifetime. In this case you will want to downgrade a `Handle` to a [`WeakHandle`] and
//!    keep that instead. Later, you can try to upgrade the `WeakHandle` back into a `Handle`,
//!    which will succeed if the actor is still alive.
//! 1. The `Handle` type is parameterized by the type of the actor. This might not be useful to
//!    you, where you just want to think about _messages_, and be generic over all actors that can
//!    handle a specific message. In which case you can convert a `Handle` into a [`Recipient`].
//!    The `Recipient` type is parameterized by the message type instead of the actor. It can be
//!    passed around and keeps the actor alive, just like a `Handle`.
//! 1. Sometimes you might want to be able to keep a `Recipient` without contributing to the actors
//!    lifetime. Just the like downgrading of a `Handle` to a `WeakHandle`, a `Recipient` can be
//!    downgraded to a [`WeakRecipient`]. Likewise, if the actor is still alive, you can upgrade a
//!    `WeakRecipient` back to a `Recipient`.

pub mod actor;
pub mod context;
pub mod handle;
pub mod message;

mod envelope;

pub use self::actor::Actor;
pub use self::context::Context;
pub use self::handle::{Handle, Recipient, SendError, WeakHandle, WeakRecipient};
pub use self::message::{Handler, Message};
