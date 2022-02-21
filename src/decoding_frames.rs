use std::{
    sync::mpsc::{channel, Receiver},
    thread,
};

use bytes::Bytes;
use ffmpeg::{
    codec::{self, decoder},
    frame,
    software::scaling,
    util::format,
};
use ffmpeg_next as ffmpeg;

use crate::{
    custom_ffmpeg_io::{read_from_custom_input, MPSCReader},
    flv_file::{BufferedSenderWriter, FLVWriterWrapper},
};

pub struct FrameExtractor {
    /// Put stream bytes into here. They will get passed to ffmpeg.
    rtmp_stream_input: FLVWriterWrapper<BufferedSenderWriter<1024>>,
}

impl FrameExtractor {
    pub fn new() -> (Self, Receiver<frame::Video>) {
        let (flv_tx, flv_rx) = channel();
        let (frame_tx, frame_rx) = channel();
        let mut rtmp_stream_input = FLVWriterWrapper::new(BufferedSenderWriter::new(flv_tx));
        rtmp_stream_input.write_header().unwrap();

        // This thread reads from the rtmp_stream_input and dumps the frames out on the frame_sink
        thread::spawn(move || {
            let custom_io = MPSCReader::new(flv_rx);
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

                let mut process_decoded_frames =
                    |decoder: &mut decoder::Video| -> Result<(), ffmpeg::Error> {
                        let mut decoded = frame::Video::empty();
                        while let Ok(()) = decoder.receive_frame(&mut decoded) {
                            let mut rgb_frame = frame::Video::empty();
                            scaler.run(&decoded, &mut rgb_frame)?;

                            // if the reciever stops listening, its completely fine for
                            // this thread to die
                            frame_tx.send(rgb_frame).unwrap();
                        }

                        Ok(())
                    };

                for (stream, packet) in ictx.packets() {
                    if stream.index() == video_stream_index {
                        decoder.send_packet(&packet).unwrap();
                        process_decoded_frames(&mut decoder).unwrap();
                    }
                }

                decoder.send_eof().unwrap();
                process_decoded_frames(&mut decoder).unwrap();
            }
        });

        (
            Self {
                rtmp_stream_input,
            },
            frame_rx,
        )
    }

    pub fn send_bytes(&mut self, timestamp: u32, bytes: &Bytes) {
        self.rtmp_stream_input
            .write_video_bytes(timestamp, bytes)
            .unwrap();
    }
}
