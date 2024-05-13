use simple_actors::{Actor, Context, Handle, Handler, Message};
use test_log::test;

#[derive(Default)]
struct PingActor {
    count: usize,
}

impl Actor for PingActor {
    type Error = String;
    type Args = ();

    async fn create(_: Context, _: Handle<Self>, _: Self::Args) -> Result<Self, Self::Error> {
        Ok(Self { count: 0 })
    }
}

struct Ping;

impl Message for Ping {
    type Reply = usize;
}

impl Handler<Ping> for PingActor {
    async fn handle(&mut self, _: Ping) -> Result<usize, Self::Error> {
        self.count += 1;
        Ok(self.count)
    }
}

#[test(tokio::test)]
async fn test_actor_ping() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = Context::default();
    let hdl = ctx.spawn_default::<PingActor>().await;

    let res = hdl.send(Ping).await?;
    assert_eq!(res, 1);

    // We have to drop the handle here, or 'ctx.wait_all' will not return.
    drop(hdl);

    ctx.wait_all().await;
    Ok(())
}
