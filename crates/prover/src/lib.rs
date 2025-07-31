use ethrex_prover_lib::{prove, to_batch_proof};
use mojave_common::{Message, ProverData};
use tokio::net::{TcpListener, TcpStream};

use tracing::error;

#[allow(unused)]
const QUEUE_SIZE: usize = 100;

#[allow(unused)]
pub struct Prover {
    aligned_mode: bool,
    tcp_listener: TcpListener,
}

impl Prover {
    /// Creates a new instance of the Prover.
    ///
    /// ```rust,ignore
    /// use mojave_prover::Prover;
    ///
    /// let (mut prover, _, _) = Prover::new(true);
    /// tokio::spawn(async move {
    ///     prover.start().await;
    /// });
    /// ```
    #[allow(unused)]
    pub async fn new(aligned_mode: bool, bind_addr: &str) -> Self {
        let tcp_listener = TcpListener::bind(bind_addr)
            .await
            .expect("TcpListener bind error");
        Prover {
            aligned_mode,
            tcp_listener,
        }
    }

    #[allow(unused)]
    pub async fn start(&mut self) {
        loop {
            match self.tcp_listener.accept().await {
                Ok((stream, addr)) => {
                    let aligned_mode = self.aligned_mode;
                    tokio::spawn(async move {
                        handle_connection(stream, aligned_mode).await;
                    });
                }
                Err(e) => {
                    error!("error accepting connection: {e}");
                }
            }
        }
    }
}

async fn handle_connection(mut stream: TcpStream, aligned_mode: bool) {
    match Message::receive::<ProverData>(&mut stream).await {
        Ok(data) => {
            let Ok(batch_proof) = prove(data.input, aligned_mode)
                .and_then(|output| to_batch_proof(output, aligned_mode))
                .inspect_err(|e| error!("{}", e.to_string()))
            else {
                error!("Error while generate proof");
                return;
            };

            if let Err(e) = Message::send(&mut stream, &(data.batch_number, batch_proof)).await {
                error!("Error while write stream: {e}");
            };
        }
        Err(e) => {
            error!("Error while receiving data from stream: {e}");
        }
    }
}
