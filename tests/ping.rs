use async_trait::async_trait;
use simple_actors::{
    message::{Handler, Message},
    Actor, Context, Handle,
};
use test_log::test;

#[derive(Default)]
struct PingActor {
    count: usize,
}

impl Actor for PingActor {
    type Error = String;
}

struct Ping;

impl Message for Ping {
    type Reply = usize;
}

#[async_trait]
impl Handler<Ping> for PingActor {
    async fn handle(&mut self, _: Ping) -> Result<usize, Self::Error> {
        self.count += 1;
        Ok(self.count)
    }
}

#[test(tokio::test)]
async fn test_actor_ping() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = Context::default();
    let hdl = Handle::<PingActor>::spawn_default(Some(ctx.clone())).await;

    let res = hdl.send(Ping).await?;
    assert_eq!(res, 1);

    // We have to drop the handle here, or 'ctx.wait_all' will not return.
    drop(hdl);

    ctx.wait_all().await;
    Ok(())
}
