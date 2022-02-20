use cxx::let_cxx_string;

use self::ffi::blurFFMpegFrame;

// these paths are relative to the current file
const CAFFE_PROTOTXT: &[u8] = include_bytes!("../models/deploy.prototxt");
const CAFFE_MODEL: &[u8] = include_bytes!("../models/res10_300x300_ssd_iter_140000_fp16.caffemodel");
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

        fn blurFFMpegFrame(pngBuffer: &[u8], filename: &CxxString) -> UniquePtr<CxxVector<u8>>;
    }
}

/// MUST be the first thing that is called before anything else in here
/// Does not lead to UB, just wrong behavior because otherwise the C++ will use
/// default-constructed `cv::dnn::Net`s
pub fn init_models() {
    ffi::loadFaceDetectorNet(CAFFE_PROTOTXT, CAFFE_MODEL);

    // this path is relative to cargo.toml FIXME: it should not be absolute
    let_cxx_string!(pathname = "/workspaces/anonynews_rs/models/openface_nn4.small2.v1.t7");
    ffi::loadFaceEmbedderNet(&pathname);
}

pub fn save_png_buffer_to_file(pngbuffer: Vec<u8>, filename: &str) {
    let_cxx_string!(filename = filename);
    blurFFMpegFrame(pngbuffer.as_slice(), &filename);
    // let mat = cvMatrixFromPNGBuffer(pngbuffer.as_slice());
    // findFaces(mat);
    // saveCvImageToFile(mat, &filename);
}

pub fn print_hello_from_cxx() {
    ffi::printHelloFromCxx();
}
