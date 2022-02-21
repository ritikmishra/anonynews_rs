use tokio::net::TcpListener;

mod connection_manager;
mod decoding_frames;
mod custom_ffmpeg_io;
mod flv_file;
mod image_processing;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    image_processing::init_models();

    let listener = TcpListener::bind("0.0.0.0:8899").await?;

    image_processing::print_hello_from_cxx();
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
