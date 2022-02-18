use futures::Future;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use std::fmt::Debug;
use std::io;

mod connection_manager;
mod decoding_frames;



#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8899").await?;

    loop {
        let (tcp_stream, _addr) = listener.accept().await?;
        println!("accepting connection from {:?}", _addr);
        tokio::spawn(async move {
            let mut conn = connection_manager::ConnectionManager::connect(tcp_stream).await.unwrap();
            conn.handle_connection().await.unwrap();
        });
    }

    Ok(())
}
