use rml_rtmp::{
    handshake::{Handshake, HandshakeProcessResult, PeerType},
    sessions::{
        ServerSession, ServerSessionConfig, ServerSessionError, ServerSessionEvent,
        ServerSessionResult,
    },
};
use std::{collections::VecDeque, thread};
use tokio::{io::AsyncWriteExt, net::TcpStream};
use tracing::{debug, info, span, Level};

use crate::image_processing;

enum SessionResultAction {
    SendBytes(Vec<u8>),
    HandleMoreSessionResults(Vec<ServerSessionResult>),
    NoAction,
    CloseConnection,
}

pub struct ConnectionManager {
    socket: TcpStream,
    session: ServerSession,
    server_session_results: VecDeque<ServerSessionResult>,
    frame_decoder: crate::decoding_frames::FrameExtractor,
}

impl std::fmt::Debug for ConnectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionManager")
            .field("socket", &self.socket)
            .field("server_session_results", &self.server_session_results)
            .field("frame_decoder", &self.frame_decoder)
            .finish()
    }
}

impl ConnectionManager {
    pub async fn connect(mut socket: TcpStream) -> anyhow::Result<Self> {
        let remaining_bytes;
        {
            // We are a server trying to receive frames
            let span = span!(Level::TRACE, "rtmp_handshake");
            let _span_raii = span.enter();

            let mut handshake_manager = Handshake::new(PeerType::Server);

            // Drive the handshake process
            remaining_bytes = loop {
                let response_bytes: Vec<u8>;
                let remaining_bytes: Option<Vec<u8>>;

                // they use yucky data representation
                socket.readable().await?;
                let vec = sock_read(&mut socket)?;

                // Their data repr is bad
                match handshake_manager.process_bytes(&vec) {
                    Ok(HandshakeProcessResult::InProgress { response_bytes: r }) => {
                        info!("read {} bytes, handshake in progress!", vec.len());
                        response_bytes = r;
                        remaining_bytes = None;
                    }
                    Ok(HandshakeProcessResult::Completed {
                        response_bytes: r,
                        remaining_bytes: leftover,
                    }) => {
                        info!("read {} bytes, handshake completed!", vec.len());
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
        }

        {
            let _span = span!(Level::TRACE, "streaming_from_client").entered();

            let (mut session, packets_to_send) = ServerSession::new(ServerSessionConfig::new())?;
            let packets_to_send2 = session.handle_input(&remaining_bytes)?;

            let (frame_decoder, frame_splitter_output) =
                crate::decoding_frames::FrameExtractor::new();
            let frame_blurrer_output = image_processing::start_blur_thread(frame_splitter_output);

            thread::Builder::new()
                .name("write blurred frames to filesystem".to_owned())
                .spawn(move || {
                    let _span = span!(Level::TRACE, "writing_frames_to_fs").entered();
                    let mut frame_counter = 0;

                    match std::fs::create_dir("./temp") {
                        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => Err(e).unwrap(),
                        _ => (),
                    };

                    loop {
                        let next_frame = frame_blurrer_output.recv().expect("frame blurring thread died");
                        let ppm = image_processing::frame_to_ppm_format(next_frame);

                        std::fs::write(format!("./temp/blurred_frame_{}.ppm", frame_counter), &ppm)
                            .unwrap();
                        frame_counter += 1;
                    }
                })
                .expect("failed to spawn thread");

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
    }

    fn process_server_session_event(
        &mut self,
        e: ServerSessionEvent,
    ) -> Result<SessionResultAction, ServerSessionError> {
        Ok(match e {
            ServerSessionEvent::ConnectionRequested {
                request_id,
                app_name,
            } => {
                debug!("\tsomeone wants to connect to app {}", app_name,);
                SessionResultAction::HandleMoreSessionResults(
                    self.session.accept_request(request_id)?,
                )
            }
            ServerSessionEvent::PublishStreamRequested {
                request_id,
                app_name,
                stream_key,
                mode,
            } => {
                debug!(
                    "\tsomeone wants to publish ({:?}) on {}/{}",
                    mode, app_name, stream_key
                );
                SessionResultAction::HandleMoreSessionResults(
                    self.session.accept_request(request_id)?,
                )
            }
            ServerSessionEvent::AudioDataReceived {
                app_name,
                stream_key,
                data,
                timestamp,
            } => SessionResultAction::NoAction,
            ServerSessionEvent::VideoDataReceived {
                app_name,
                stream_key,
                data,
                timestamp,
            } => {
                self.frame_decoder.send_bytes(timestamp.value, &data);
                SessionResultAction::NoAction
            }
            ServerSessionEvent::ClientChunkSizeChanged { new_chunk_size } => todo!(),
            ServerSessionEvent::ReleaseStreamRequested {
                request_id,
                app_name,
                stream_key,
            } => {
                debug!("\t'Release stream requested'?");
                SessionResultAction::NoAction
            }
            ServerSessionEvent::PublishStreamFinished {
                app_name,
                stream_key,
            } => {
                debug!("\tthey finished publishing a stream");
                SessionResultAction::CloseConnection
            }
            ServerSessionEvent::StreamMetadataChanged {
                app_name,
                stream_key,
                metadata,
            } => {
                debug!("\tthey changed the stream metadata: {:?}", metadata);
                SessionResultAction::NoAction
            }
            c @ ServerSessionEvent::UnhandleableAmf0Command { .. } => {
                debug!("\tgot an unhandleable amf0 command: {:?}", c);
                SessionResultAction::NoAction
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
                debug!("\tthey are requesting to play a stream");
                SessionResultAction::NoAction
            }
            ServerSessionEvent::PlayStreamFinished {
                app_name,
                stream_key,
            } => {
                debug!("\tthey finished playing a stream");
                SessionResultAction::CloseConnection
            }
            ServerSessionEvent::AcknowledgementReceived { bytes_received } => {
                debug!("\tthey acknowledged they received some bytes");
                SessionResultAction::NoAction
            }
            ServerSessionEvent::PingResponseReceived { timestamp } => {
                debug!("\treceived a ping response");
                SessionResultAction::NoAction
            }
        })
    }

    fn process_server_session_result(
        &mut self,
        ssr: ServerSessionResult,
    ) -> Result<SessionResultAction, ServerSessionError> {
        match ssr {
            ServerSessionResult::RaisedEvent(e) => self.process_server_session_event(e),
            ServerSessionResult::OutboundResponse(packet) => {
                Ok(SessionResultAction::SendBytes(packet.bytes))
            }
            ServerSessionResult::UnhandleableMessageReceived(_) => {
                debug!("yuck! we got an unhandleable message :(");
                Ok(SessionResultAction::NoAction)
            }
        }
    }

    #[allow(unused)]
    #[tracing::instrument(level = "info")]
    pub fn process_message_buffer(&mut self) -> anyhow::Result<(Vec<u8>, bool)> {
        let mut connection_should_close = false;
        let mut bytes_to_send: Vec<u8> = Vec::new();
        let mut more_session_results: Vec<ServerSessionResult> = Vec::new();

        // Keep processing server events until there are none left
        // Most server events require us to send some bytes to the client,
        // but some kinds decompose into more events, so we may as well handle those too
        while !self.server_session_results.is_empty() {
            let x = std::mem::take(&mut self.server_session_results);
            x.into_iter()
                .map(|ssr| self.process_server_session_result(ssr))
                .try_for_each(|el| -> Result<(), ServerSessionError> {
                    match el? {
                        SessionResultAction::SendBytes(b) => {
                            bytes_to_send.extend(b);
                        }
                        SessionResultAction::HandleMoreSessionResults(srs) => {
                            more_session_results.extend(srs);
                        }
                        SessionResultAction::CloseConnection => {
                            connection_should_close = true;
                        }
                        SessionResultAction::NoAction => (),
                    };
                    Ok(())
                })?;

            self.server_session_results
                .extend(std::mem::take(&mut more_session_results));
        }

        Ok((bytes_to_send, connection_should_close))
    }

    #[tracing::instrument]
    pub async fn handle_connection(&mut self) -> anyhow::Result<()> {
        loop {
            let (bytes_to_send, should_close_connection) = self.process_message_buffer()?;
            self.socket.write_all(&bytes_to_send).await?;
            self.socket.readable().await?;
            let read_bytes = sock_read(&mut self.socket)?;
            self.server_session_results
                .extend(self.session.handle_input(&read_bytes)?);

            if should_close_connection {
                info!("closing connection");
                return Ok(());
            }
        }
    }
}

#[tracing::instrument]
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
