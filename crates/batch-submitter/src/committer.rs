use bytes::Bytes;
use ethrex_common::types::batch::Batch;
use ethrex_p2p::{
    network::P2PContext,
    rlpx::{
        message::Message as RlpxMessage,
        mojave::messages::{MojaveBatch, MojaveMessage},
    },
};
use mojave_msgio::types::{self, Publisher};
use mojave_task::Service;
use mojave_utils::hash;
use tokio::sync::broadcast;

use crate::error::{Error, Result};

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

    fn commit_next_batch_to_l1(&self, _batch: Batch) -> Result<()> {
        // TODO: Implement the logic to commit the next batch to L1
        Ok(())
    }
}

impl<P> Service for Committer<P>
where
    P: Publisher,
{
    type Error = Error;

    async fn run(&mut self) -> Result<()> {
        tracing::info!("Committer started but doing nothing as of right now");

        let batch = self.rx.recv().await?;

        self.commit_next_batch_to_l1(batch.clone())?;

        // didn't check about dedup here
        let msg_id = hash::compute_keccak(&batch.number.to_ne_bytes());

        let msg = types::Message {
            header: types::MessageHeader {
                version: 1,
                kind: types::MessageKind::BatchSubmit,
                message_id: msg_id,
                seq: 1,
            },
            body: &batch,
        };

        let data = bincode::serialize(&msg)?;
        let data = Bytes::from(data);
        self.queue.publish(data).await?;

        self.p2p_context
            .broadcast_mojave_message(RlpxMessage::Mojave(MojaveMessage::Batch(
                MojaveBatch::new(batch),
            )))?;

        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down the committer...");
        Ok(())
    }
}