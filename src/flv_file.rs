use std::io;
use std::io::Write;

use bytes::Bytes;

pub struct FLVWriterWrapper<W> {
    inner: W,
}

impl<W: Write> FLVWriterWrapper<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }
    pub fn write_header(&mut self) -> io::Result<()> {
        const HEADER_BYTES: &[u8] = &[
            b'F', b'L', b'V', 0x01, 0x05, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x00,
        ];
        self.inner.write_all(HEADER_BYTES)?;
        Ok(())
    }

    pub fn write_video_bytes(&mut self, timestamp: u32, video_bytes: &Bytes) -> io::Result<()> {
        // Step 1: Write the header for this block
        {
            let payload_size: [u8; 4] = (video_bytes.len() as u32).to_be_bytes();
            let timestamp_bytes: [u8; 4] = timestamp.to_be_bytes();

            self.inner.write_all(&[9])?; // indicates this is a video
            self.inner.write_all(&payload_size[1..])?;
            self.inner.write_all(&timestamp_bytes[1..])?;
            self.inner.write_all(&timestamp_bytes[..1])?;
            self.inner.write_all(&[0, 0, 0])?; // stream ID, TODO: maybe this should not be 0?
        }

        // Step 2: Write the actual video bytes
        self.inner.write_all(video_bytes.as_ref())?;

        // Step 3: Start writing the header for the next block
        {
            let prev_tag_size = (11 + video_bytes.len()) as u32;
            self.inner.write_all(&prev_tag_size.to_be_bytes())?;
        }

        Ok(())
    }

    // TODO:: implement writing for audio

    // TODO: implement writinhg for metadata

    pub fn flush_inner(&mut self) -> io::Result<()> {
        self.inner.flush()
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

//-----

use arrayvec::ArrayVec;
use std::sync::mpsc::{SendError, Sender};

pub struct BufferedSenderWriter<const BUF_SIZE: usize> {
    sender: Sender<ArrayVec<u8, BUF_SIZE>>,
    buffer: ArrayVec<u8, BUF_SIZE>,
}

impl<const BUF_SIZE: usize> BufferedSenderWriter<BUF_SIZE> {
    pub fn new(sender: Sender<ArrayVec<u8, BUF_SIZE>>) -> Self {
        Self {
            sender,
            buffer: ArrayVec::new(),
        }
    }
}

impl<const BUF_SIZE: usize> Write for BufferedSenderWriter<BUF_SIZE> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let remaining_buffer_space = self.buffer.remaining_capacity();
        let could_add = if buf.len() > remaining_buffer_space {
            &buf[..remaining_buffer_space]
        } else {
            buf
        };

        // safe bc we check that we don't write over beforehand
        self.buffer.try_extend_from_slice(could_add).unwrap();

        if self.buffer.remaining_capacity() == 0 {
            if let Err(SendError(_)) = self.sender.send(std::mem::take(&mut self.buffer)) {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
        }

        Ok(could_add.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.sender.send(std::mem::take(&mut self.buffer)) {
            Ok(()) => Ok(()),
            Err(SendError(_)) => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }
}
