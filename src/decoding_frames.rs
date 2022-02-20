use std::io::Write;

use bytes::Bytes;
use ffmpeg_next::{
    codec::{context::Context, decoder, id::Id},
    frame,
    util::format,
    Frame, Packet,
};

use tokio::{fs, io::AsyncWriteExt};
use crate::flv_file::FLVWriterWrapper;

pub async fn debug_save_to_png(frame: &frame::Video, filename: &str) -> std::io::Result<()> {
    let mut file = fs::File::open(filename).await?;

    let mut png_encoder = Context::new()
        .encoder()
        .video()
        .unwrap()
        .open_as(Id::PNG)
        .unwrap();

    png_encoder.send_frame(&*frame).unwrap();
    png_encoder.send_eof();

    let mut packet = Packet::new(10_000_000_000);
    png_encoder.receive_packet(&mut packet).unwrap();

    file.write_all(packet.data().expect("why isn't there data in my packet??"))
        .await?;

    Ok(())
}

pub struct FrameExtractor {
    flv_file: FLVWriterWrapper<std::fs::File>
    // accumulated_bytes: Vec<u8>,
    // video_decoder: decoder::Video,
    // file: std::fs::File
}

impl FrameExtractor {
    pub fn new() -> Self {
        let file = std::fs::File::create("hi2.flv").unwrap();
        let mut flv_file = FLVWriterWrapper::new(file);
        flv_file.write_header().unwrap();
        Self { flv_file }
    }

    pub fn decode_bytes(&mut self, timestamp: u32, bytes: &Bytes) -> Option<frame::Video> {
        self.flv_file.write_video_bytes(timestamp, bytes).unwrap();
        None
    }
}
