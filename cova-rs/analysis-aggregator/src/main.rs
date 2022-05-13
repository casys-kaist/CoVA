#![feature(drain_filter, hash_drain_filter)]

use clap::Parser;
use std::path::Path;
use tokio::task;
mod server;
use log::info;
use std::sync::Arc;
use tokio::sync::{mpsc, Barrier};

use crate::server::assoc::assoc_server;
use server::dnn::dnn_server;
use server::track::track_server;

#[derive(Debug)]
enum Recieved {
    Dnn(Vec<bbox::Bbox>),
    Track(bbox::Frame),
    First(u64),
}

#[derive(Parser, Debug)]
struct Args {
    #[clap()]
    output_dir: String,
    /// Port number for the tracker
    #[clap()]
    track_port: String,
    /// Port number for the DNN
    #[clap()]
    dnn_port: String,
    #[clap(long, default_value = "1")]
    num_tracker: usize,
    #[clap(long, default_value = "0.15")]
    moving_iou: f32,
    #[clap(long, default_value = "0.3")]
    stationary_iou: f32,
    #[clap(long, default_value = "120")]
    stationary_maxage: usize,
    #[clap(long, default_value = "1.3")]
    scale_factor: f32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse Arguments
    let args = Args::parse();
    env_logger::init();

    // Create channel for association
    let (tx, rx) = mpsc::channel(10 * 1024 * 1024);

    let barrier = Arc::new(Barrier::new(args.num_tracker * 2 + 1));

    // Run Tracker Server
    info!(
        "[Track] Starting track server with {} workers",
        args.num_tracker
    );
    let port = args.track_port.clone();
    let tx_tmp = tx.clone();
    let barrier_tmp = barrier.clone();
    let track_handle = task::spawn(async move {
        track_server(port, args.num_tracker, tx_tmp, barrier_tmp)
            .await
            .expect("Error occured in track server.");
    });

    // Run DNN Server
    info!(
        "[DNN] Starting DNN server with {} workers",
        args.num_tracker
    );
    let port = args.dnn_port.clone();
    let barrier_tmp = barrier.clone();
    let dnn_handle = task::spawn(async move {
        dnn_server(port, args.num_tracker, tx, barrier_tmp)
            .await
            .expect("Error occured in DNN server.");
    });

    // Run association Server
    info!("[ASSOC] Starting association server",);
    let output_dir = args.output_dir.clone();
    let assoc_handle = task::spawn(async move {
        assoc_server(
            args.num_tracker,
            Path::new(&output_dir).join(Path::new("track.csv")),
            Path::new(&output_dir).join(Path::new("dnn.csv")),
            Path::new(&output_dir).join(Path::new("assoc.csv")),
            Path::new(&output_dir).join(Path::new("stationary.csv")),
            rx,
            barrier,
            args.moving_iou,
            args.stationary_iou,
            args.stationary_maxage,
            args.scale_factor,
        )
        .await
        .expect("Error occured in assoc server.");
    });

    dnn_handle.await?;
    track_handle.await?;
    assoc_handle.await?;

    Ok(())
}
