use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use std::io;

async fn send_on_tcp_stream(mut tcp_stream: TcpStream) -> io::Result<()> {
    println!("connected!");
    tcp_stream.readable().await?;

    let mut str = String::new();

    
    loop {
        let mut buf: Vec<u8> = vec![0; 20];
        let num_bytes = tcp_stream.try_read(&mut buf[..])?;
        match std::str::from_utf8(&mut buf[..num_bytes]) {
            Ok(a) => str.push_str(a),
            Err(_) => {
                println!("what? invalid utf8?");
                break;
            },
        }
        if num_bytes < 20 { 
            break;
        }
    }

    println!("{}", str);

    tcp_stream.write_all(b"hi!").await?;
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8899").await?;

    loop {
        let (mut tcp_stream, _addr) = listener.accept().await?;


        tokio::spawn(send_on_tcp_stream(tcp_stream));
    }

    Ok(())
}
