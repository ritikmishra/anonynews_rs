use std::{
    io::{ErrorKind, Read, Seek, SeekFrom, Write},
    mem::MaybeUninit,
    sync::{
        mpsc::{sync_channel, RecvError, TryRecvError},
        Arc,
    },
    thread,
};

use ffmpeg::{
    codec, decoder,
    format::{self, input},
    frame,
    software::scaling,
};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::sys as ffmpeg_c;

use arrayvec::ArrayVec;
use std::sync::mpsc::{channel, Receiver};

/*


static int readFunction(void* opaque, uint8_t* buf, int buf_size) {
    auto& me = *reinterpret_cast<std::istream*>(opaque);
    me.read(reinterpret_cast<char*>(buf), buf_size);
    return me.gcount();
}

std::ifstream stream("file.avi", std::ios::binary);

const std::shared_ptr<unsigned char> buffer(reinterpret_cast<unsigned char*>(av_malloc(8192)), &av_free);
const std::shared_ptr<AVIOContext> avioContext(avio_alloc_context(buffer.get(), 8192, 0, reinterpret_cast<void*>(static_cast<std::istream*>(&stream)), &readFunction, nullptr, nullptr), &av_free);

const auto avFormat = std::shared_ptr<AVFormatContext>(avformat_alloc_context(), &avformat_free_context);
auto avFormatPtr = avFormat.get();
avFormat->pb = avioContext.get();
avformat_open_input(&avFormatPtr, "dummyFilename", nullptr, nullptr);

*/

trait CustomFFMPEGIO {
    /// Read from self, writing the bytes read into the buffer
    fn read(&mut self, buf: &mut [MaybeUninit<u8>]) -> Result<u32, ffmpeg::Error>;

    /// Write into self, reading from the bytes in the buffer
    fn write(&mut self, buf: &[u8]) -> Result<u32, ffmpeg::Error>;

    fn get_format_info(&mut self) -> Option<&mut ffmpeg_c::AVInputFormat> {
        None
    }
}

const CHUNK_SIZE: usize = 1024;
type ChunkType = ArrayVec<u8, CHUNK_SIZE>;

struct MPSCReader {
    recv: Receiver<ChunkType>,
}

impl MPSCReader {
    fn new(recv: Receiver<ChunkType>) -> Self {
        Self {
            recv
        }
    }
}

impl CustomFFMPEGIO for MPSCReader {
    fn read(&mut self, buf: &mut [MaybeUninit<u8>]) -> Result<u32, ffmpeg::Error> {
        assert!(
            buf.len() > CHUNK_SIZE,
            "what the hell? ffmpeg gave us a small buffer to write into?"
        );

        let mut bytes_written = 0;
        for (chunk_idx, chunk) in buf.chunks_exact_mut(CHUNK_SIZE).enumerate() {
            let bytes = if chunk_idx == 0 {
                self.recv.recv().map_err(|RecvError| ffmpeg::Error::Eof)?
            } else {
                match self.recv.try_recv() {
                    Ok(bytes) => bytes,
                    Err(TryRecvError::Disconnected | TryRecvError::Empty) => break,
                }
            };

            let (left, _) = chunk.split_at_mut(bytes.len());
            for (i, j) in Iterator::zip(left.iter_mut(), bytes.iter()) {
                i.write(*j);
            }
            bytes_written += bytes.len();
        }

        Ok(bytes_written as u32)
    }

    fn write(&mut self, buf: &[u8]) -> Result<u32, ffmpeg::Error> {
        todo!()
    }
}

struct FileReader {
    file_bytes: std::io::Bytes<std::fs::File>,
}

impl FileReader {
    fn new<P: std::convert::AsRef<std::path::Path>>(path: P) -> Self {
        let file = std::fs::File::open(path).unwrap();
        Self {
            file_bytes: file.bytes(),
        }
    }
}

impl CustomFFMPEGIO for FileReader {
    fn read(&mut self, buf: &mut [MaybeUninit<u8>]) -> Result<u32, ffmpeg::Error> {
        let mut maxi: usize = 0;
        for (i, byte) in self
            .file_bytes
            .by_ref()
            .flat_map(Result::ok)
            .enumerate()
            .take(buf.len())
        {
            buf[i].write(byte);
            maxi = i;
        }

        if maxi == 0 {
            Err(ffmpeg::Error::Eof)
        } else {
            Ok((maxi + 1) as u32)
        }
    }

    fn write(&mut self, buf: &[u8]) -> Result<u32, ffmpeg::Error> {
        todo!()
    }
}

unsafe extern "C" fn epic_read_func<T: CustomFFMPEGIO>(
    opaque: *mut libc::c_void,
    buf: *mut u8,
    buf_size: i32,
) -> i32 {
    let it = &mut *(opaque as *mut T);

    let buf_safer = std::slice::from_raw_parts_mut(buf as *mut MaybeUninit<u8>, buf_size as usize);
    let result = it.read(buf_safer);

    match result {
        Ok(bytes_read) => bytes_read as i32,
        Err(e) => libc::c_int::from(e),
    }
}

fn read_from_custom_input<T: CustomFFMPEGIO>(
    custom_ffmpegio_reader: T,
) -> Result<ffmpeg::format::context::Input, ffmpeg::Error> {
    let mut custom_ffmpegio_reader = Box::new(custom_ffmpegio_reader);
    unsafe {
        // init AVFormatContext
        let mut avformat_context = ffmpeg_c::avformat_alloc_context();
        {
            let buf_size: i32 = 8192;
            let buf = ffmpeg_c::av_malloc(buf_size as usize) as *mut u8;

            (*avformat_context).pb = ffmpeg_c::avio_alloc_context(
                buf,
                buf_size,
                0, // 0 for read, 1 for write,
                &mut *custom_ffmpegio_reader as *mut T as *mut libc::c_void,
                Some(epic_read_func::<T>),
                None,
                None,
            );

            (*avformat_context).flags |= ffmpeg_c::AVFMT_FLAG_CUSTOM_IO;

            (*avformat_context).iformat =
                ffmpeg_c::av_find_input_format(b"flv\0" as *const u8 as *const i8);
            // custom_ffmpegio_reader.get_format_info();
            // (*avformat_context).probesize = 10_000_000;
            // (*avformat_context).max_analyze_duration = 10_000_000;
        }

        // open the input
        match ffmpeg_c::avformat_open_input(
            &mut avformat_context,
            b"\0" as *const u8 as *const i8,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) {
            0 => {
                match ffmpeg_c::avformat_find_stream_info(avformat_context, std::ptr::null_mut()) {
                    r @ 0.. => {
                        Box::leak(custom_ffmpegio_reader);
                        Ok(ffmpeg::format::context::Input::wrap(avformat_context))
                    }
                    e => {
                        ffmpeg_c::avformat_close_input(&mut avformat_context);
                        Err(ffmpeg::Error::from(e))
                    }
                }
            }
            e => Err(ffmpeg::Error::from(e)),
        }
    }
}


fn main() {
    let (tx, rx) = sync_channel::<ChunkType>(1_000);

    thread::spawn(move || {
        let tx = tx;
        std::fs::read("hi.flv")
            .unwrap()
            .chunks(1024)
            .map(|chunk| ChunkType::try_from(chunk).unwrap())
            .for_each(|x| tx.send(x).unwrap());
    });

    let custom_io = MPSCReader::new(rx);
    // let custom_io = FileReader::new("hi.flv");

    if let Ok(mut ictx) = read_from_custom_input(custom_io)
    {
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
            file.write_all(format!("P6\n{} {}\n255\n", frame.width(), frame.height()).as_bytes())?;
            file.write_all(frame.data(0))?;
            Ok(())
        }

        fn save_packet(packet: &ffmpeg::Packet, index: usize) -> std::io::Result<()> {
            let mut file = std::fs::File::create(format!("temp_bytes/frame{}.bytes", index))?;
            file.write_all(packet.data().unwrap())?;
            Ok(())
        }

        let mut frame_index = 0;
        let mut frame_index2 = 0;
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
}
