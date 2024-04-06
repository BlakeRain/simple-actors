use async_trait::async_trait;
use simple_actors::{Actor, Context, Handler, Message};
use test_log::test;

#[derive(Default)]
struct MyActor {}

impl Actor for MyActor {
    type Error = String;
}

struct MyMessage {}

impl Message for MyMessage {
    type Reply = ();
}

#[async_trait]
impl Handler<MyMessage> for MyActor {
    async fn handle(&mut self, _message: MyMessage) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test(tokio::test)]
async fn test_actor_weak_handle() -> Result<(), Box<dyn std::error::Error>> {
    // Create a context and spawn a new actor
    let ctx = Context::default();
    let hdl = ctx.spawn_default::<MyActor>().await;

    // Make sure that we have the actor in our context
    assert_eq!(ctx.remaining_actors().await, 1);

    // Downgrade our handle to a weak handle
    let weak = hdl.downgrade();

    // We should still have the actor in our context
    assert_eq!(ctx.remaining_actors().await, 1);

    // Upgrading the handle should work fine
    {
        assert!(weak.upgrade().is_some());
    }

    // Drop the handle, and yield execution to allow the actor to be dropped
    drop(hdl);
    tokio::task::yield_now().await;

    // We should no longer have the actor in our context
    assert_eq!(ctx.remaining_actors().await, 0);

    // The weak handle should no longer upgrade.
    assert!(weak.upgrade().is_none());

    Ok(())
}

#[test(tokio::test)]
async fn test_actor_weak_handle_upgrade() -> Result<(), Box<dyn std::error::Error>> {
    // Create a context and spawn a new actor
    let ctx = Context::default();
    let hdl = ctx.spawn_default::<MyActor>().await;

    // Make sure that we have the actor in our context
    assert_eq!(ctx.remaining_actors().await, 1);

    // Downgrade our handle to a weak handle
    let weak = hdl.downgrade();

    // We should still have the actor in our context
    assert_eq!(ctx.remaining_actors().await, 1);

    // Upgrading the handle should work fine, giving us a new handle
    let upgraded = {
        let upgraded = weak.upgrade();
        assert!(upgraded.is_some());
        upgraded.unwrap()
    };

    // Drop the handle, and yield execution to the runtime
    drop(hdl);
    tokio::task::yield_now().await;

    // We should still have the actor in our context, as 'upgraded' is still live.
    assert_eq!(ctx.remaining_actors().await, 1);

    // Now drop the 'upgraded' handle, and yield execution to the runtime to allow the actor to be
    // dropped
    drop(upgraded);
    tokio::task::yield_now().await;

    // We should no longer have the actor in our context
    assert_eq!(ctx.remaining_actors().await, 0);

    // The weak handle should no longer upgrade.
    assert!(weak.upgrade().is_none());

    Ok(())
}

#[test(tokio::test)]
async fn test_actor_weak_recipient() -> Result<(), Box<dyn std::error::Error>> {
    // Create a context and spawn a new actor
    let ctx = Context::default();
    let hdl = ctx.spawn_default::<MyActor>().await;

    // Make sure that we have the actor in our context
    assert_eq!(ctx.remaining_actors().await, 1);

    // Create a 'Recipient<MyMessage>' from our handle
    let rec = hdl.recipient::<MyMessage>();

    // Drop the handle, and yield execution
    drop(hdl);
    tokio::task::yield_now().await;

    // Even though we've dropped our handle, our `Recipient` should still keep our actor alive.
    assert_eq!(ctx.remaining_actors().await, 1);

    // Now downgrade our recipient to a `WeakRecipient`.
    let weak = rec.downgrade();

    // We should still have the actor in our context
    assert_eq!(ctx.remaining_actors().await, 1);

    // Upgrading the weak recipient should work fine
    {
        assert!(weak.upgrade().is_some());
    }

    // Drop the receiver, and yield execution to allow the actor to be dropped
    drop(rec);
    tokio::task::yield_now().await;

    // We should no longer have the actor in our context
    assert_eq!(ctx.remaining_actors().await, 0);

    // The weak recipient should no longer upgrade.
    assert!(weak.upgrade().is_none());

    Ok(())
}

#[test(tokio::test)]
async fn test_actor_weak_recipient_upgrade() -> Result<(), Box<dyn std::error::Error>> {
    // Create a context and spawn a new actor
    let ctx = Context::default();
    let hdl = ctx.spawn_default::<MyActor>().await;

    // Make sure that we have the actor in our context
    assert_eq!(ctx.remaining_actors().await, 1);

    // Create a 'Recipient<MyMessage>' from our handle
    let rec = hdl.recipient::<MyMessage>();

    // Drop the handle, and yield execution
    drop(hdl);
    tokio::task::yield_now().await;

    // Even though we've dropped our handle, our `Recipient` should still keep our actor alive.
    assert_eq!(ctx.remaining_actors().await, 1);

    // Now downgrade our recipient to a `WeakRecipient`.
    let weak = rec.downgrade();

    // We should still have the actor in our context
    assert_eq!(ctx.remaining_actors().await, 1);

    // Upgrading the weak recipient should give us a new recipient
    let upgraded = {
        let upgraded = weak.upgrade();
        assert!(upgraded.is_some());
        upgraded.unwrap()
    };

    // Drop the receiver, and yield execution to the runtime
    drop(rec);
    tokio::task::yield_now().await;

    // We should still have the actor in our context, as 'upgraded' is still live.
    assert_eq!(ctx.remaining_actors().await, 1);

    // Now drop the 'upgraded' recipient, and yield execution to the runtime to allow the actor to
    // be dropped
    drop(upgraded);
    tokio::task::yield_now().await;

    // We should no longer have the actor in our context
    assert_eq!(ctx.remaining_actors().await, 0);

    // The weak recipient should no longer upgrade.
    assert!(weak.upgrade().is_none());

    Ok(())
}
