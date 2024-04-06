use std::sync::Arc;

use tokio::{sync::Mutex, task::JoinSet};

use crate::{Actor, Handle};

#[derive(Debug, Default)]
struct ContextInner {
    join_set: JoinSet<()>,
}

impl ContextInner {
    fn filter_finished_tasks(&mut self) {
        while let Some(result) = self.join_set.try_join_next() {
            if let Err(err) = result {
                tracing::error!(error = ?err, "Encountered error joining actor");
            }
        }
    }

    fn take_join_set(&mut self) -> JoinSet<()> {
        self.filter_finished_tasks();
        std::mem::take(&mut self.join_set)
    }
}

/// A context for retaining actors.
///
/// This structure is used to track the tasks associated with a set of actors. When you wish to
/// spawn a new actor, you call the `Context::spawn` method, which will return the `Handle` you can
/// use to communicate with the actor.
///
/// The context simplifies tracking multiple actors, especially when actors end up spawning other
/// actors. The traditional `Vec` of `JoinHandle` is not entirely useful when this happens. The
/// `Context` provides some useful functions such as checking how many remaining actors are in the
/// `Context` and waiting for all the actors to finish.
///
/// Access to the `Context` is brokered by a tokio `Mutex`, so the `Context` can be freely cloned
/// and passed around with abandon. Actors themselves will receive a clone of the `Context` into
/// which they are spawned via their `Actor::started` trait method. Actors can retain the `Context`
/// and use it for spawning additional actors.
#[derive(Debug, Default, Clone)]
pub struct Context {
    inner: Arc<Mutex<ContextInner>>,
}

impl Context {
    /// Spawn a new actor in this context.
    ///
    /// This will spawn a new task that executes the actor mailbox. The function returns a new
    /// `Handle` that can be used to send messages to the new actor.
    pub async fn spawn<A: Actor + 'static>(&self, actor: A) -> Handle<A> {
        let mut inner = self.inner.lock().await;
        Handle::run(self.clone(), &mut inner.join_set, actor)
    }

    /// Spawn a default actor.
    ///
    /// This is the same as the `spawn` method except it uses the `Default` value for the actor.
    pub async fn spawn_default<A: Actor + Default + 'static>(&self) -> Handle<A> {
        self.spawn(A::default()).await
    }

    /// Count the number of remaining actors.
    ///
    /// If there are actor tasks that were registered with this `Context`, and are still running,
    /// they are counted towards the total returned from this method. An actor task generally keeps
    /// running until all the `Handle` to the task have been dropped or an irrecoverable error has
    /// been encountered.
    ///
    /// Note that the result of this method is informative rather than precise: as we reply on
    /// `JoinHandle` to ascertain if a task is still running, and `JoinHandle::is_finished()` might
    /// be delayed in it's response to a task terminating (even if `Joinhandle::abort()` is
    /// called), it's possible for the count returned from this method to be imprecise during the
    /// period between one or more tasks terminating and their corresponding `JoinHandle` being
    /// notified.
    pub async fn remaining_actors(&self) -> usize {
        let mut inner = self.inner.lock().await;
        inner.filter_finished_tasks();
        inner.join_set.len()
    }

    /// Check if there are any remaining actors.
    ///
    /// Note that this method operates identically to `Context::remaining_actors()`, and is
    /// therefore susceptible to the same caveats.
    pub async fn is_empty(&self) -> bool {
        self.remaining_actors().await > 0
    }

    pub(crate) async fn take_join_set(&self) -> JoinSet<()> {
        self.inner.lock().await.take_join_set()
    }

    /// Wait for all actors to complete.
    ///
    /// This method takes the current set of pending actors out of the `Context` and waits for them
    /// all to finish. This can end up taking an indefinite amount of time whilst there is still a
    /// `Handle` retained for one or more actors that were in this context, so some adverse runtime
    /// nonsense has occurred.
    ///
    /// The `Context` can be freely used to spawn new actors either after this method has returned
    /// or during this methods execution: new actors will be added to the `Context` as normal, this
    /// method takes the current set of actors from the `Context` at time of invocation.
    #[tracing::instrument(skip_all)]
    pub async fn wait_all(&self) {
        let mut join_set = self.take_join_set().await;
        if join_set.is_empty() {
            tracing::info!("No actors remaining in context");
            return;
        }

        tracing::info!(
            "Politely waiting for {} actor(s) to terminate",
            join_set.len()
        );

        while let Some(result) = join_set.join_next().await {
            if let Err(err) = result {
                tracing::error!(error = ?err, "Encountered error waiting for actor");
            }
        }
    }

    /// Terminate all actors in the context.
    ///
    /// This method will take the current set of running actors out of the `Context`, abort all
    /// their tasks, and then wait for them to finish.
    ///
    /// Note that this method should not be used unless all the tasks must be terminated
    /// forcefully. The ideal way to terminate a set of actors is to drop all their handles. You
    /// might be tempted to use this emthod if there are cycles in your actor graph. This is not
    /// the correct way to shutdown: you should use `WeakHandle` instead and break reference
    /// cycles.
    #[tracing::instrument(skip_all)]
    pub async fn terminate_all(&self) {
        let mut join_set = self.take_join_set().await;
        if join_set.is_empty() {
            tracing::info!("No actors remaining in context");
            return;
        }

        tracing::info!("Rudely terminating {} actor(s)", join_set.len());
        join_set.abort_all();

        while let Some(result) = join_set.join_next().await {
            if let Err(err) = result {
                tracing::error!(error = ?err, "Encountered error waiting for actor");
            }
        }
    }
}
