use simple_actors::{Actor, Context, Handle, Handler, Message, Recipient, SendError};
use test_log::test;

struct OrderShipped(usize);

impl Message for OrderShipped {
    type Reply = ();
}

struct Ship(usize);

impl Message for Ship {
    type Reply = ();
}

struct Subscribe(pub Recipient<OrderShipped>);

impl Message for Subscribe {
    type Reply = ();
}

#[derive(Default)]
struct OrderEvents {
    subscribers: Vec<Recipient<OrderShipped>>,
}

impl Actor for OrderEvents {
    type Error = String;
    type Args = ();

    async fn create(_: Context, _: Handle<Self>, _: Self::Args) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}

impl OrderEvents {
    pub async fn notify(&mut self, order_id: usize) -> Result<(), SendError> {
        for subscriber in self.subscribers.clone().into_iter() {
            let res = subscriber.post(OrderShipped(order_id));
            res.await?;
        }

        Ok(())
    }
}

impl Handler<Subscribe> for OrderEvents {
    async fn handle(&mut self, message: Subscribe) -> Result<(), Self::Error> {
        self.subscribers.push(message.0);
        Ok(())
    }
}

impl Handler<Ship> for OrderEvents {
    async fn handle(&mut self, message: Ship) -> Result<(), Self::Error> {
        let _ = self.notify(message.0).await;
        Ok(())
    }
}

#[derive(Default)]
struct EmailSubscriber;
impl Actor for EmailSubscriber {
    type Error = String;
    type Args = ();

    async fn create(_: Context, _: Handle<Self>, _: Self::Args) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl Handler<OrderShipped> for EmailSubscriber {
    async fn handle(&mut self, message: OrderShipped) -> Result<(), Self::Error> {
        tracing::info!("Email sent for order {}", message.0);
        Ok(())
    }
}

#[derive(Default)]
struct SmsSubscriber;
impl Actor for SmsSubscriber {
    type Error = String;
    type Args = ();

    async fn create(_: Context, _: Handle<Self>, _: Self::Args) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl Handler<OrderShipped> for SmsSubscriber {
    async fn handle(&mut self, message: OrderShipped) -> Result<(), Self::Error> {
        tracing::info!("SMS sent for order {}", message.0);
        Ok(())
    }
}

#[test(tokio::test)]
async fn test_actor_recipient() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = Context::default();
    let sub1 = Subscribe(ctx.spawn_default::<EmailSubscriber>().await.recipient());
    let sub2 = Subscribe(ctx.spawn_default::<SmsSubscriber>().await.recipient());

    let order_events = ctx.spawn_default::<OrderEvents>().await;
    order_events.post(sub1).await?;
    order_events.post(sub2).await?;

    order_events.post(Ship(1000)).await?;

    drop(order_events);
    ctx.wait_all().await;

    Ok(())
}
