use crate::Recieved;
use bbox::Frame;
use futures::StreamExt;
use log::{debug, info};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Barrier;
use tokio::task;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

/// Open port and connect to fixed number of trackers
pub(crate) async fn track_server(
    port: String,
    num_tracker: usize,
    tx: tokio::sync::mpsc::Sender<Recieved>,
    barrier: Arc<Barrier>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    let mut handles = vec![];
    for _ in 0..num_tracker {
        let (socket, _) = listener.accept().await?;
        let tx_tmp = tx.clone();
        let barrier_tmp = barrier.clone();
        let handle = task::spawn(async move {
            track_worker(socket, tx_tmp, barrier_tmp).await.expect("");
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await?;
    }
    info!("[TRACK] Exit");
    Ok(())
}

async fn track_worker(
    socket: TcpStream,
    tx: tokio::sync::mpsc::Sender<Recieved>,
    barrier: Arc<Barrier>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut transport = Framed::new(socket, LengthDelimitedCodec::new());
    let mut first = true;
    loop {
        match transport.next().await {
            Some(ret) => {
                // Recieve and deserialize Frame { bboxes, oldest }
                let serialized = ret?;
                let mut frame = Frame::de(&serialized[..])?;

                // For only the first frame, send the range_start message
                if first {
                    tx.send(Recieved::First(frame.range_start)).await?;
                    debug!("[TRACK] Waiting first barrier");
                    barrier.wait().await;
                    first = false;
                }

                for bbox in frame.bboxes.iter_mut() {
                    // Scale from macroblock to pixel
                    bbox.scale_dim(16.);
                    // Modify track ID to differ across trackers
                    *bbox.track_id.as_mut().unwrap() += frame.range_start;
                }
                // Send Frame to associator
                tx.send(Recieved::Track(frame)).await?;
            }
            None => {
                debug!("[TRACK] EOF recieved");
                break;
            }
        };
    }
    transport.into_inner().shutdown().await?;
    Ok(())
}
