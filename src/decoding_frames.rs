use std::io::Write;

use bytes::Bytes;
use ffmpeg_next::{
    codec::{context::Context, decoder, id::Id},
    frame,
    util::format,
    Frame, Packet,
};

use tokio::{fs, io::AsyncWriteExt};

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
    video_decoder: decoder::Video,
    file: std::fs::File
}

impl FrameExtractor {
    pub fn new() -> Self {
        let mut x = Context::new()
            .decoder()
            .open_as(Id::FLV1)
            .unwrap()
            .video()
            .unwrap();
        let vv = &[
            b'F', b'L', b'V', 0x01,
            0x05, 0x00, 0x00, 0x00,
            0x09, 0x00, 0x00, 0x00,
            0x00,
        ];
        // x.send_packet(&Packet::borrow(vv))
        // .unwrap();
        let mut file = std::fs::File::create("hi.flv").unwrap();
        file.write_all(vv).unwrap();
        
        Self { video_decoder: x, file }
    }

    pub fn decode_bytes(&mut self, timestamp: u32, bytes: &Bytes) -> Option<frame::Video> {
        let prev_tag_size = (11 + bytes.len()) as u32;
        let frontmatter = {
            let mut ret: Vec<u8> = Vec::with_capacity(16);

            let payload_size: [u8; 4] = (bytes.len() as u32).to_be_bytes();
            let timestamp_bytes: [u8; 4] = timestamp.to_be_bytes();

            ret.push(9);
            ret.extend_from_slice(&payload_size[1..]);
            ret.extend_from_slice(&timestamp_bytes[1..]);
            ret.extend_from_slice(&timestamp_bytes[..1]);
            ret.extend_from_slice(&[0, 0, 0]); // stream id
            ret
        };
        self.file.write_all(frontmatter.as_slice()).unwrap();
        self.file.write_all(bytes.as_ref()).unwrap();
        self.file.write_all(&prev_tag_size.to_be_bytes()).unwrap();

        self.video_decoder
            .send_packet(&Packet::borrow(frontmatter.as_slice()))
            .ok()?;
        match self
            .video_decoder
            .send_packet(&Packet::borrow(bytes.as_ref()))
        {
            Ok(()) => {}
            e @ Err(_) => {
                println!("wot?? {:?}", e);
                return None;
            }
        };

        // let mut frame = frame::Video::new(ffmpeg_next::format::Pixel::ARGB, 1920, 1080);
        let mut frame = frame::Video::empty();
        match self.video_decoder.receive_frame(&mut *frame) {
            Ok(()) => Some(frame),
            e @ Err(_) => {
                println!("what? {:?}", e);
                None
            }
        }
    }
}
