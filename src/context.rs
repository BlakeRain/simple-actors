use std::sync::Arc;

use tokio::{sync::Mutex, task::JoinHandle};

#[derive(Debug, Default)]
struct ContextInner {
    actors: Vec<(String, JoinHandle<()>)>,
}

impl ContextInner {
    fn filter_finished_tasks(&mut self) {
        self.actors.retain(|(_, task)| !task.is_finished());
    }
}

/// A context for retaining actors.
///
/// This structure can be passed to the `Handle::spawn()` and `Handle::spawn_default()` functions
/// to register the task for the actor message loop into the context. This is useful for keeping
/// track of a number of actors without actually retaining any references to them: once all the
/// `Handle` for each actor have been dropped, regardless of the actor's presence in a `Context`,
/// the actor task will terminate.
///
/// The `Context` provides some useful functions such as checking how many remaining actors are in
/// the `Context` and waiting for all the actors to finish.
#[derive(Debug, Default, Clone)]
pub struct Context {
    inner: Arc<Mutex<ContextInner>>,
}

impl Context {
    pub(crate) async fn register_actor(&self, name: String, task: JoinHandle<()>) {
        let mut inner = self.inner.lock().await;
        inner.actors.push((name, task))
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
    /// be delayed in it's repsonse to a task terminating (even if `Joinhandle::abort()` is
    /// called), it's possible for the count returned from this method to be imprecise during the
    /// period between one or more tasks terminating and their corresponding `JoinHandle` being
    /// notified.
    pub async fn remaining_actors(&self) -> usize {
        let mut inner = self.inner.lock().await;
        inner.filter_finished_tasks();
        inner.actors.len()
    }

    /// Check if there are any remaining actors.
    ///
    /// Note that this method operates identically to `Context::remaining_actors()`, and is
    /// therefore susceptible to the same caveats.
    pub async fn is_empty(&self) -> bool {
        self.remaining_actors().await == 0
    }

    /// Wait for all actors to complete.
    ///
    /// This method consumes the `Context` (takes ownership) and will not return until all tasks
    /// have completed. This can end up taking an indefinite amount of time whilst there is still a
    /// `Handle` retained for one or more actors registered with this context, or some adverse
    /// runtime nonsense has occurred.
    #[tracing::instrument(skip_all)]
    pub async fn wait_all(self) {
        let tasks = {
            let mut inner = self.inner.lock().await;
            inner.filter_finished_tasks();
            std::mem::take(&mut inner.actors)
        };

        if tasks.is_empty() {
            tracing::info!("No actors remaining in context");
            return;
        }

        tracing::info!("Politely waiting for {} actor(s) to terminate", tasks.len());
        let tasks = tasks.into_iter().map(|(name, task)| {
            tracing::info!("Remaining actor: {name}");
            task
        });

        futures::future::join_all(tasks).await;
    }

    /// Terminate all actors in the context.
    ///
    /// This method will go through all the remaining tasks in the `Context` and abort them using
    /// the corresponding `JoinHandle::abort()` method. It will then wait for each task to
    /// terminate and report if there was any deviation from the expected `JoinError`.
    ///
    /// Specifically, if the task termination caused a panic, this will be logged as an error.
    ///
    /// Note that this method should not be used unless all the tasks must be terminated
    /// forcefully. The ideal way to terminate a set of actors is to drop all their handles. You
    /// might be tempted to use this emthod if there are cycles in your actor graph. This is not
    /// the correct way to shutdown; you should use `WeakHandle` instead and break reference
    /// cycles.
    #[tracing::instrument(skip_all)]
    pub async fn terminate_all(self) {
        let tasks = {
            let mut inner = self.inner.lock().await;
            inner.filter_finished_tasks();
            std::mem::take(&mut inner.actors)
        };

        if tasks.is_empty() {
            tracing::info!("No actors remaining in context");
            return;
        }

        tracing::info!("Rudely terminating {} actor(s)", tasks.len());
        for (name, task) in &tasks {
            tracing::info!("Terminating actor: {name}");
            task.abort();
        }

        for (name, task) in tasks {
            match task.await {
                Ok(_) => tracing::info!("Actor {name} terminated successfully"),
                Err(err) => {
                    if err.is_panic() {
                        tracing::error!(
                            "Actor {name} panicked during termination: {:?}",
                            err.into_panic()
                        );
                    }
                }
            }
        }
    }
}
