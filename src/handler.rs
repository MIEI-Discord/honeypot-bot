use serenity::{
    all::{Context, EventHandler, Message},
    async_trait,
};

pub(crate) struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        todo!()
    }
}
