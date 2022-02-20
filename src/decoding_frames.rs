use std::{io::Write, sync::mpsc::channel, thread};

use arrayvec::ArrayVec;
use bytes::Bytes;
use ffmpeg_next as ffmpeg;
use ffmpeg::{
    codec::{context::Context, decoder, id::Id, self},
    frame,
    util::format,
    Frame, Packet, software::scaling,
};

use crate::{flv_file::{BufferedSenderWriter, FLVWriterWrapper}, custom_ffmpeg_io::{read_from_custom_input, MPSCReader}};
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
    flv_file: FLVWriterWrapper<std::fs::File>,
    flv_stream: FLVWriterWrapper<BufferedSenderWriter<1024>>, // accumulated_bytes: Vec<u8>,
                                                              // video_decoder: decoder::Video,
                                                              // file: std::fs::File
}

impl FrameExtractor {
    pub fn new() -> Self {
        let file = std::fs::File::create("hi2.flv").unwrap();
        let mut flv_file = FLVWriterWrapper::new(file);
        flv_file.write_header().unwrap();

        let (tx, rx) = channel();
        let mut flv_stream = FLVWriterWrapper::new(BufferedSenderWriter::new(tx));
        flv_stream.write_header().unwrap();

        thread::spawn(move || {
            let custom_io = MPSCReader::new(rx);
            // let custom_io = FileReader::new("hi.flv");

            if let Ok(mut ictx) = read_from_custom_input(custom_io) {
                println!(
                    "{:?}",
                    ictx.streams()
                        .inspect(|x| println!("{:?}", x.metadata()))
                        .count()
                );
                let input = ictx
                    .streams()
                    .next()
                    // .best(ffmpeg::media::Type::Video)
                    .expect("pleases");

                let video_stream_index = input.index();
                let ctx = codec::Context::from_parameters(input.parameters()).unwrap();
                let mut decoder = ctx.decoder().video().unwrap();

                // Convert the frame to RGB, same width, height
                let mut scaler = scaling::Context::get(
                    decoder.format(),
                    decoder.width(),
                    decoder.height(),
                    format::Pixel::RGB24,
                    decoder.width(),
                    decoder.height(),
                    scaling::Flags::BILINEAR,
                )
                .unwrap();

                println!(
                    "width {:?} vs height {:?}",
                    decoder.width(),
                    decoder.height()
                );

                fn save_file(
                    frame: &frame::Video,
                    index: usize,
                ) -> std::result::Result<(), std::io::Error> {
                    let mut file = std::fs::File::create(format!("temp/frame{}.ppm", index))?;
                    file.write_all(
                        format!("P6\n{} {}\n255\n", frame.width(), frame.height()).as_bytes(),
                    )?;
                    file.write_all(frame.data(0))?;
                    Ok(())
                }

                fn save_packet(packet: &ffmpeg::Packet, index: usize) -> std::io::Result<()> {
                    let mut file =
                        std::fs::File::create(format!("temp_bytes/frame{}.bytes", index))?;
                    file.write_all(packet.data().unwrap())?;
                    Ok(())
                }

                let mut frame_index = 0;
                let mut process_decoded_frames =
                    |decoder: &mut decoder::Video| -> Result<(), ffmpeg::Error> {
                        let mut decoded = frame::Video::empty();
                        while let Ok(()) = decoder.receive_frame(&mut decoded) {
                            let mut rgb_frame = frame::Video::empty();
                            scaler.run(&decoded, &mut rgb_frame)?;
                            save_file(&rgb_frame, frame_index).unwrap();
                            frame_index += 1;
                        }

                        Ok(())
                    };

                for (stream, packet) in ictx.packets() {
                    if stream.index() == video_stream_index {
                        // save_packet(&packet, frame_index2);
                        // frame_index2 += 1;
                        decoder.send_packet(&packet).unwrap();
                        process_decoded_frames(&mut decoder).unwrap();
                    }
                }

                decoder.send_eof().unwrap();
                process_decoded_frames(&mut decoder).unwrap();
            }
        });

        Self {
            flv_file,
            flv_stream,
        }
    }

    pub fn decode_bytes(&mut self, timestamp: u32, bytes: &Bytes) -> Option<frame::Video> {
        self.flv_file.write_video_bytes(timestamp, bytes).unwrap();
        self.flv_stream.write_video_bytes(timestamp, bytes).unwrap();
        None
    }
}
