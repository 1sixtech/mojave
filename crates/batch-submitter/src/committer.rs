use bytes::Bytes;
use ethrex_common::types::batch::Batch;
use ethrex_p2p::{
    network::P2PContext,
    rlpx::{
        Message,
        mojave::messages::{MojaveBatch, MojaveMessage},
    },
};
use mojave_msgio::types::Publisher;
use mojave_task::Task;
use tokio::sync::broadcast;

use crate::{
    error::{Error, Result},
    types::Request,
};

pub struct Committer<P: Publisher> {
    rx: broadcast::Receiver<Batch>,
    queue: P,
    p2p_context: P2PContext,
}

impl<P> Committer<P>
where
    P: Publisher,
{
    pub fn new(rx: broadcast::Receiver<Batch>, queue: P, p2p_context: P2PContext) -> Self {
        Self {
            rx,
            queue,
            p2p_context,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("Commiter started but doing nothing as of right now");

        loop {
            let batch = self.rx.recv().await?;

            self.commit_next_batch_to_l1(batch.clone())?;

            let data = bincode::serialize(&batch)?;
            let data = Bytes::from(data);
            self.queue.publish(data).await?;

            self.p2p_context
                .broadcast_mojave_message(Message::Mojave(MojaveMessage::Batch(
                    MojaveBatch::new(batch),
                )))?;
        }
    }

    #[allow(dead_code)]
    fn commit_next_batch_to_l1(&self, _batch: Batch) -> Result<()> {
        // TODO: Implement the logic to commit the next batch to L1
        Ok(())
    }
}

// impl<P> Task for Committer<P>
// where
//     P: Publisher,
// {
//     type Request = Request;
//
//     type Response = ();
//
//     type Error = Error;
//
//     async fn handle_request(&mut self, _request: Self::Request) -> Result<Self::Response> {
//         // for this case we don't care about request as it will alway be listening for the next tx
//     }
//
//     async fn on_shutdown(&mut self) -> Result<()> {
//         todo!()
//     }
// }
