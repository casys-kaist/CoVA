use crate::Recieved;
use bbox::Bbox;
use log::{debug, info};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Barrier;
use tokio::task;

/// Open port and connect to fixed number of DNN
pub(crate) async fn dnn_server(
    port: String,
    num_dnn: usize,
    tx: tokio::sync::mpsc::Sender<Recieved>,
    barrier: Arc<Barrier>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

    let mut handles = vec![];
    for _ in 0..num_dnn {
        let (socket, _) = listener.accept().await?;
        let tx_tmp = tx.clone();
        let barrier_tmp = barrier.clone();
        let handle = task::spawn(async move {
            dnn_worker(socket, tx_tmp, barrier_tmp)
                .await
                .expect("Failed to recieve detection");
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await?;
    }
    info!("[DNN] Exit");
    Ok(())
}

async fn dnn_worker(
    mut socket: TcpStream,
    tx: tokio::sync::mpsc::Sender<Recieved>,
    barrier: Arc<Barrier>,
) -> Result<(), Box<dyn std::error::Error>> {
    debug!("[DNN] Waiting first barrier");
    barrier.wait().await;

    let mut dnn_buf = [0 as u8; 10000];
    let mut dnn_remain_str: String = String::new();

    loop {
        // Read data from socket
        let n = socket.read(&mut dnn_buf).await?;

        // Disconnected with client
        if n == 0 {
            return Ok(());
        }

        let mut bboxes = vec![];
        // Recieve and deserialize bounding boxes
        // FIXME: use track_worker like scheme to parse
        dnn_remain_str.push_str(&String::from_utf8_lossy(&dnn_buf[0..n]));
        for splited in dnn_remain_str.split("\n") {
            let list: Vec<&str> = splited.split(",").collect();
            if (list.len() != 6) || (list[5].len() == 0) {
                dnn_remain_str = splited.to_string();
                break;
            }
            let timestamp = list[0];
            let left = list[1];
            let top = list[2];
            let width = list[3];
            let height = list[4];
            let class_id = list[5];

            let timestamp: u64 = timestamp.parse()?;
            let left: f32 = left.parse()?;
            let top: f32 = top.parse()?;
            let width: f32 = width.parse()?;
            let height: f32 = height.parse()?;
            let class_id: i32 = class_id.parse()?;
            let class_id: u32 = u32::try_from(class_id).expect("Casting failed");

            let mut bbox = Bbox::new(left, top, width, height);
            bbox.class_id = Some(class_id);
            bbox.timestamp = Some(timestamp);

            bboxes.push(bbox);
        }
        // Send bounding box to associator
        tx.send(Recieved::Dnn(bboxes)).await?;
    }
}
