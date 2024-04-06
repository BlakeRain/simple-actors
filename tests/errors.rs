use async_trait::async_trait;
use simple_actors::{messages, Actor, Context, Handler, SendError};
use test_log::test;

#[derive(Default)]
struct ErrorActor {
    count: usize,
}

#[derive(Debug)]
enum Error {
    Recoverable,
    Irrecoverable,
}

impl Actor for ErrorActor {
    type Error = Error;

    fn is_recoverable(&mut self, error: &Self::Error) -> bool {
        matches!(error, Error::Recoverable)
    }
}

enum Ping {
    Regular,
    Error(Error),
}

messages! {
    Ping => usize
}

#[async_trait]
impl Handler<Ping> for ErrorActor {
    async fn handle(&mut self, ping: Ping) -> Result<usize, Self::Error> {
        match ping {
            Ping::Regular => {
                // Regular ping operation
                self.count += 1;
                Ok(self.count)
            }

            Ping::Error(err) => {
                // Irregular ping operation
                Err(err)
            }
        }
    }
}

#[test(tokio::test)]
async fn test_actor_error_recoverable() -> Result<(), Box<dyn std::error::Error>> {
    // Create the context and spawn our ping actor.
    let ctx = Context::default();
    let hdl = ctx.spawn_default::<ErrorActor>().await;

    // Send a ping message to the actor as normal
    let res = hdl.send(Ping::Regular).await?;
    assert_eq!(res, 1);

    // Now send a message to the actor that causes the actor to encounter an error. This is not an
    // irrecoverable error, so the actor should still be alive afterwards.
    //
    // This should result in an error indicating that the reply channel has closed. This will be
    // because the actor has handled the message, but then encountered an error. The error caused
    // the `EnvelopeHandler::handle` method to just drop the oneshot reply sender.

    let res = hdl.send(Ping::Error(Error::Recoverable)).await;
    assert_eq!(res, Err(SendError::ReplyChannelClosed));

    // Make sure that our actor is still alive and well
    let res = hdl.send(Ping::Regular).await?;
    assert_eq!(res, 2);

    // Make sure that we still have an actor left
    assert_eq!(ctx.remaining_actors().await, 1);

    // We have to drop the handle here, or `ctx.wait_all` will not return.
    drop(hdl);

    ctx.wait_all().await;
    Ok(())
}

#[test(tokio::test)]
async fn test_actor_error_irrecoverable() -> Result<(), Box<dyn std::error::Error>> {
    // Create the context and spawn our ping actor.
    let ctx = Context::default();
    let hdl = ctx.spawn_default::<ErrorActor>().await;

    // Send a ping message to the actor as normal
    let res = hdl.send(Ping::Regular).await?;
    assert_eq!(res, 1);

    // Now send a message that causes the actor to terminate it's event loop. This is done with a
    // ping message that tells the actor's handler for this message to return an error. The error
    // is irrecoverable, meaning the actor will end.
    //
    // This should result in an error indicating that the reply channel has closed. This will be
    // because the actor has handled the message, but then encountered an error. The error caused
    // the `EnvelopeHandler::handle` method to just drop the oneshot reply sender.

    let res = hdl.send(Ping::Error(Error::Irrecoverable)).await;
    assert_eq!(res, Err(SendError::ReplyChannelClosed));

    // Make sure that we have no more actors left
    assert_eq!(ctx.remaining_actors().await, 0);

    // Attempting to use the handle again should result in an error
    let res = hdl.send(Ping::Regular).await;
    assert_eq!(res, Err(SendError::ChannelClosed));

    // We have to drop the handle here, or `ctx.wait_all` will not return.
    drop(hdl);

    ctx.wait_all().await;
    Ok(())
}
