

const CAFFE_PROTOTXT: &[u8] = include_bytes!("../models/deploy.prototxt");
const CAFFE_MODEL: &[u8] = include_bytes!("../models/res10_300x300_ssd_iter_140000.caffemodel");
const TORCH_MODEL: &[u8] = include_bytes!("../models/openface_nn4.small2.v1.t7");

#[cxx::bridge(namespace=anonynews_rs)]
pub mod ffi {

    extern "Rust" {
    }

    unsafe extern "C++" {
        include!("anonynews_rs/include/cv_face_blurring.h");


        fn printHelloFromCxx() -> ();

        fn loadFaceDetectorNet(
            bufProto: &[u8],
            bufModel: &[u8]
        ) -> ();
    
        fn loadFaceEmbedderNet(
            bufModel: &[u8]
        ) -> ();
    
    }
}

pub fn init_models() {
    
}