use tokio::net::TcpListener;

mod connection_manager;
mod decoding_frames;
mod custom_ffmpeg_io;
mod flv_file;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8899").await?;

    loop {
        println!("ready to accept connections");
        let (tcp_stream, _addr) = listener.accept().await?;
        println!("accepting connection from {:?}", _addr);
        tokio::spawn(async move {
            let mut conn = connection_manager::ConnectionManager::connect(tcp_stream)
                .await
                .unwrap();
            conn.handle_connection().await.unwrap();
        });
    }

    Ok(())
}
