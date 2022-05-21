use tokio::net::{TcpListener, TcpStream, tcp};
use tracing::{info, error};

mod connection_manager;
mod decoding_frames;
mod custom_ffmpeg_io;
mod flv_file;
mod image_processing;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    image_processing::init_models();

    let listener = TcpListener::bind("0.0.0.0:8899").await?;

    image_processing::print_hello_from_cxx();
    loop {
        info!("ready to accept connections");
        let (tcp_stream, _addr) = listener.accept().await?;
        info!("accepting connection from {:?}", _addr);
        tokio::spawn(manage_connection(tcp_stream));
    }
}

#[tracing::instrument]
async fn manage_connection(socket: TcpStream) {
    let mut conn = connection_manager::ConnectionManager::connect(socket)
        .await
        .unwrap();
    if let Err(e) = conn.handle_connection().await {
        let err_dyn: &dyn std::error::Error = e.as_ref();
        error!(problem = err_dyn, "bruh what the hell?",);
    }
}
