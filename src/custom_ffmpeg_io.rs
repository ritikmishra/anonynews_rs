//! Once the RTMP server gets bytes, we need to pass those bytes to ffmpeg so that
//! they can be decoded into frames. This module provides an adapter to turn
//! a std::sync::mpsc::Receiver into an ffmpeg input
//! Once you have the ffmpeg input, you can subsequently extract frames from it
//! and blur them or whatever you want

use std::{
    mem::MaybeUninit,
    sync::mpsc::{Receiver, RecvError, TryRecvError},
};

use arrayvec::ArrayVec;
use ffmpeg::sys as ffmpeg_c;
use ffmpeg_next as ffmpeg;

// --- trait defns

/// A trait for something that `ffmpeg` can read bytes from
pub trait CustomFFMpegRead {
    /// Read from self, writing the bytes read into the buffer
    fn read(&mut self, buf: &mut [MaybeUninit<u8>]) -> Result<u32, ffmpeg::Error>;

    /// Guess what format this video file is in
    fn get_format_info(&mut self) -> Option<&mut ffmpeg_c::AVInputFormat> {
        None
    }
}

/// A trait for something that `ffmpeg` can write into
pub trait CustomFFMpegWrite {
    /// Write into self, reading from the bytes in the buffer
    fn write(&mut self, buf: &[u8]) -> Result<u32, ffmpeg::Error>;
}

// --- concrete impl defns

/// A wrapper around a `Receiver<ArrayVec<u8, CHUNK_SIZE>>` 
/// that `ffmpeg` can read bytes from in order to decode audio
pub struct MPSCReader<const CHUNK_SIZE: usize> {
    recv: Receiver<ArrayVec<u8, CHUNK_SIZE>>,
}

impl<const CHUNK_SIZE: usize> MPSCReader<CHUNK_SIZE> {
    pub fn new(recv: Receiver<ArrayVec<u8, CHUNK_SIZE>>) -> Self {
        Self { recv }
    }
}

impl<const CHUNK_SIZE: usize> CustomFFMpegRead for MPSCReader<CHUNK_SIZE> {
    fn read(&mut self, buf: &mut [MaybeUninit<u8>]) -> Result<u32, ffmpeg::Error> {
        assert!(
            buf.len() > CHUNK_SIZE,
            "what the hell? ffmpeg gave us a small buffer to write into?"
        );

        let mut bytes_written = 0;
        for (chunk_idx, chunk) in buf.chunks_exact_mut(CHUNK_SIZE).enumerate() {
            // block for the first chunk only
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
            // warning: correctness relies on chunks_exact_mut
            bytes_written += bytes.len();
        }

        Ok(bytes_written as u32)
    }
}

// --- unsafe code below

/// This function tells ffmpeg how to read from a CustomFFMpegRead implementor
///
/// # Contract
///
/// This function transmutes the opaque pointer to a mutable reference to `T`
/// This means that
/// 1. The pointer should point to a valid `T`
/// 2. The pointer should be unique
/// 3. The pointer should be non-null
unsafe extern "C" fn custom_ffmpeg_read<T: CustomFFMpegRead>(
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

/// Use this function to help ffmpeg read from custom rust sources
pub fn read_from_custom_input<T: CustomFFMpegRead>(
    custom_ffmpegio_reader: T,
) -> Result<ffmpeg::format::context::Input, ffmpeg::Error> {
    let mut custom_ffmpegio_reader = Box::new(custom_ffmpegio_reader);
    unsafe {
        // step 1: init AVFormatContext
        let mut avformat_context = ffmpeg_c::avformat_alloc_context();
        {
            let buf_size: i32 = 8192;
            let buf = ffmpeg_c::av_malloc(buf_size as usize) as *mut u8;

            // tell the av format context to use our custom IO functions
            (*avformat_context).pb = ffmpeg_c::avio_alloc_context(
                buf,
                buf_size,
                0, // 0 for read, 1 for write,
                &mut *custom_ffmpegio_reader as *mut T as *mut libc::c_void,
                Some(custom_ffmpeg_read::<T>),
                None,
                None,
            );

            (*avformat_context).flags |= ffmpeg_c::AVFMT_FLAG_CUSTOM_IO;

            // FIXME: assume we are reading FLV
            (*avformat_context).iformat = custom_ffmpegio_reader
                .get_format_info()
                .map(|mutref| mutref as *mut _)
                .unwrap_or_else(|| {
                    ffmpeg_c::av_find_input_format(b"flv\0" as *const u8 as *const i8)
                });
        }

        // open the input
        match ffmpeg_c::avformat_open_input(
            &mut avformat_context,
            b"\0" as *const u8 as *const i8, // empty filename
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) {
            0 => {
                match ffmpeg_c::avformat_find_stream_info(avformat_context, std::ptr::null_mut()) {
                    0.. => {
                        // FIXME: nobody ever frees the custom ffmpeg io reader??
                        Box::leak(custom_ffmpegio_reader);
                        Ok(ffmpeg::format::context::Input::wrap(avformat_context))
                    }
                    errno => {
                        ffmpeg_c::avformat_close_input(&mut avformat_context);
                        Err(ffmpeg::Error::from(errno))
                    }
                }
            }
            errno => Err(ffmpeg::Error::from(errno)),
        }
    }
}
