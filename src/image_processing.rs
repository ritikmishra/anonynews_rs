use std::{
    env::temp_dir,
    io::Write,
    os::unix::prelude::OsStrExt,
    sync::mpsc::{channel, Receiver},
    thread,
};

use cxx::let_cxx_string;
use ffmpeg_next::{format::pixel, frame};

// these paths are relative to the current file
const CAFFE_PROTOTXT: &[u8] = include_bytes!("../models/deploy.prototxt");
const CAFFE_MODEL: &[u8] =
    include_bytes!("../models/res10_300x300_ssd_iter_140000_fp16.caffemodel");
const TORCH_MODEL: &[u8] = include_bytes!("../models/openface_nn4.small2.v1.t7");

#[cxx::bridge(namespace=anonynews_rs)]
mod ffi {
    extern "Rust" {}
    unsafe extern "C++" {
        include!("/usr/local/include/opencv4/opencv2/core.hpp");

        #[namespace = cv]
        type Mat;
    }

    unsafe extern "C++" {
        include!("anonynews_rs/include/cv_face_blurring.h");

        fn printHelloFromCxx() -> ();

        fn loadFaceDetectorNet(bufProto: &[u8], bufModel: &[u8]) -> ();

        fn loadFaceEmbedderNet(pathname: &CxxString) -> ();

        fn blurFFMpegFrame(pngBuffer: &[u8]) -> UniquePtr<CxxVector<u8>>;
    }
}

/// MUST be the first thing that is called before anything else in here
/// Does not lead to UB, just wrong behavior because otherwise the C++ will use
/// default-constructed `cv::dnn::Net`s
pub fn init_models() {
    ffi::loadFaceDetectorNet(CAFFE_PROTOTXT, CAFFE_MODEL);

    // this path is relative to cargo.toml
    let mut filename = temp_dir();
    filename.push("openface_model");

    std::fs::File::create(filename.as_path())
        .and_then(|mut f| {
            f.write_all(TORCH_MODEL)?;
            // FIXME -- using .as_bytes() is platform specific
            let_cxx_string!(pathname = filename.as_os_str().as_bytes());
            ffi::loadFaceEmbedderNet(&pathname);
            Ok(())
        })
        .expect("could not load openface model into opencv");
}

pub fn frame_to_ppm_format(frame: frame::Video) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();

    buf.extend_from_slice(format!("P6\n{} {}\n255\n", frame.width(), frame.height()).as_bytes());
    buf.extend_from_slice(frame.data(0));

    buf
}

pub fn blur_a_frame(frame: frame::Video) -> frame::Video {
    // remember how big the frame was
    let width = frame.width();
    let height = frame.height();

    let ppm_bytes = frame_to_ppm_format(frame);
    let blurred = ffi::blurFFMpegFrame(&ppm_bytes);

    let mut ret = frame::Video::new(pixel::Pixel::RGB24, width, height);
    // TODO: would be cooler if we didn't copy, i.e we got opencv to write into this directly
    ret.data_mut(0).copy_from_slice(blurred.as_slice());
    ret
}

pub fn print_hello_from_cxx() {
    ffi::printHelloFromCxx();
}

pub fn start_blur_thread(frame_receiver: Receiver<frame::Video>) -> Receiver<frame::Video> {
    let (blurred_tx, blurred_rx) = channel();

    thread::Builder::new()
        .name("frame blur thread".to_owned())
        .spawn(move || loop {
            // if they stop sending us frames, unwrap will trigger
            let to_blur = frame_receiver.recv().expect("frame splitter thread died");
            let blurred_buffer = blur_a_frame(to_blur);

            // if they stop listening to our frames, unwrap will trigger
            blurred_tx.send(blurred_buffer).expect("whoever was supposed to consume blurred frames died");
        })
        .expect("failed to spawn thread");

    blurred_rx
}
