use std::{collections::VecDeque, thread};

use rml_rtmp::{
    handshake::{Handshake, HandshakeProcessResult, PeerType},
    sessions::{ServerSession, ServerSessionConfig, ServerSessionEvent, ServerSessionResult},
};
use tokio::{io::AsyncWriteExt, net::TcpStream};

use crate::image_processing;

pub struct ConnectionManager {
    socket: TcpStream,
    session: ServerSession,
    server_session_results: VecDeque<ServerSessionResult>,
    frame_decoder: crate::decoding_frames::FrameExtractor,
}

impl ConnectionManager {
    pub async fn connect(mut socket: TcpStream) -> anyhow::Result<Self> {
        println!("Handshaking . .. ");
        // We are a server trying to receive frames
        let mut handshake_manager = Handshake::new(PeerType::Server);

        // Drive the handshake process
        let remaining_bytes = loop {
            let response_bytes: Vec<u8>;
            let remaining_bytes: Option<Vec<u8>>;

            // they use yucky data representation
            socket.readable().await?;
            let vec = sock_read(&mut socket)?;

            // Their data repr is bad
            match handshake_manager.process_bytes(&vec) {
                Ok(HandshakeProcessResult::InProgress { response_bytes: r }) => {
                    println!("read {} bytes, handshake in progress!", vec.len());
                    response_bytes = r;
                    remaining_bytes = None;
                }
                Ok(HandshakeProcessResult::Completed {
                    response_bytes: r,
                    remaining_bytes: leftover,
                }) => {
                    println!("read {} bytes, handshake completed!", vec.len());
                    response_bytes = r;
                    remaining_bytes = Some(leftover);
                }
                Err(_) => {
                    todo!("error handling in handshake conducting")
                }
            }

            socket.write_all(&response_bytes).await?;

            if let Some(remaining_bytes) = remaining_bytes {
                break remaining_bytes;
            }
        };

        let (mut session, packets_to_send) = ServerSession::new(ServerSessionConfig::new())?;
        let packets_to_send2 = session.handle_input(&remaining_bytes)?;

        let (frame_decoder, frame_splitter_output) = crate::decoding_frames::FrameExtractor::new();
        let frame_blurrer_output = image_processing::start_blur_thread(frame_splitter_output);

        thread::spawn(move || {
            let mut frame_counter = 0;

            loop {
                let next_frame = frame_blurrer_output.recv().unwrap();
                let ppm = image_processing::frame_to_ppm_format(next_frame);

                std::fs::write(format!("./temp/blurred_frame_{}.ppm", frame_counter), &ppm).unwrap();
                frame_counter += 1;
            }
        });

        Ok(Self {
            socket,
            session,
            server_session_results: {
                let mut deque = VecDeque::from(packets_to_send);
                deque.extend(packets_to_send2);
                deque
            },
            frame_decoder,
        })
    }

    #[allow(unused)]
    fn convert_serversessionresult_to_bytes(&mut self) -> anyhow::Result<Vec<u8>> {
        let mut ret: Vec<u8> = Vec::new();
        while let Some(result) = self.server_session_results.pop_front() {
            match result {
                ServerSessionResult::OutboundResponse(packet) => {
                    ret.extend(packet.bytes);
                }
                ServerSessionResult::RaisedEvent(e) => match &e {
                    ServerSessionEvent::ConnectionRequested {
                        request_id,
                        app_name,
                    } => {
                        println!("\tsomeone wants to connect to app {}", app_name,);

                        let results = self.session.accept_request(*request_id)?;
                        self.server_session_results.extend(results);
                        ret.extend(self.convert_serversessionresult_to_bytes()?);
                    }
                    ServerSessionEvent::PublishStreamRequested {
                        request_id,
                        app_name,
                        stream_key,
                        mode,
                    } => {
                        println!(
                            "\tsomeone wants to publish ({:?}) on {}/{}",
                            mode, app_name, stream_key
                        );
                        let results = self.session.accept_request(*request_id)?;
                        self.server_session_results.extend(results);
                        ret.extend(self.convert_serversessionresult_to_bytes()?);
                    }
                    ServerSessionEvent::AudioDataReceived {
                        app_name,
                        stream_key,
                        data,
                        timestamp,
                    } => {}
                    ServerSessionEvent::VideoDataReceived {
                        app_name,
                        stream_key,
                        data,
                        timestamp,
                    } => {
                        self.frame_decoder.send_bytes(timestamp.value, data);
                    }
                    ServerSessionEvent::ClientChunkSizeChanged { new_chunk_size } => todo!(),
                    ServerSessionEvent::ReleaseStreamRequested {
                        request_id,
                        app_name,
                        stream_key,
                    } => {
                        println!("\t'Release stream requested'?");
                    }
                    ServerSessionEvent::PublishStreamFinished {
                        app_name,
                        stream_key,
                    } => {
                        println!("\tthey finished publishing a stream");
                    }
                    ServerSessionEvent::StreamMetadataChanged {
                        app_name,
                        stream_key,
                        metadata,
                    } => {
                        println!("\tthey changed the stream metadata: {:?}", metadata);
                    }
                    c @ ServerSessionEvent::UnhandleableAmf0Command {
                        command_name,
                        transaction_id,
                        command_object,
                        additional_values,
                    } => {
                        println!("\tgot an unhandleable amf0 command: {:?}", c);
                    }
                    ServerSessionEvent::PlayStreamRequested {
                        request_id,
                        app_name,
                        stream_key,
                        start_at,
                        duration,
                        reset,
                        stream_id,
                    } => {
                        println!("\tthey are requesting to play a stream");
                    }
                    ServerSessionEvent::PlayStreamFinished {
                        app_name,
                        stream_key,
                    } => {
                        println!("\tthey finished playing a stream");
                    }
                    ServerSessionEvent::AcknowledgementReceived { bytes_received } => {
                        println!("\tthey acknowledged they received some bytes")
                    }
                    ServerSessionEvent::PingResponseReceived { timestamp } => {
                        println!("\treceived a ping response")
                    }
                },
                ServerSessionResult::UnhandleableMessageReceived(_) => {
                    println!("\tyuck! we got an unhandleable message!")
                }
            }
        }

        // self.socket.flush().await?;
        Ok(ret)
    }

    pub async fn handle_connection(&mut self) -> anyhow::Result<()> {
        loop {
            let buf = self.convert_serversessionresult_to_bytes()?;
            self.socket.write_all(&buf).await?;
            self.socket.readable().await?;
            let read_bytes = sock_read(&mut self.socket)?;
            self.server_session_results
                .extend(self.session.handle_input(&read_bytes)?);
        }
    }
}

fn sock_read(socket: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    loop {
        match socket.try_read_buf(&mut buf) {
            Ok(0) => return Ok(buf),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(buf),
            Ok(_) => {}
            Err(e) => return Err(e),
        }
    }
}
