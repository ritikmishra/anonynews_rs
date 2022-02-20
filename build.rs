
fn main() {

    // copy pasted from pkg-config
    // disgusting, i know
    println!("cargo:rustc-link-search=/usr/local/lib");
    println!("cargo:rustc-link-search=/usr/local/lib/opencv4/3rdparty");
    println!("cargo:rustc-link-search=/opencv/opencv-4.2.0/build/lib");

    println!("cargo:rustc-link-lib=static=opencv_dnn");
    println!("cargo:rustc-link-lib=static=opencv_gapi");
    println!("cargo:rustc-link-lib=static=opencv_highgui");
    println!("cargo:rustc-link-lib=static=opencv_ml");
    println!("cargo:rustc-link-lib=static=opencv_objdetect");
    println!("cargo:rustc-link-lib=static=opencv_photo");
    println!("cargo:rustc-link-lib=static=opencv_stitching");
    println!("cargo:rustc-link-lib=static=opencv_video");
    println!("cargo:rustc-link-lib=static=opencv_calib3d");
    println!("cargo:rustc-link-lib=static=opencv_features2d");
    println!("cargo:rustc-link-lib=static=opencv_flann");
    println!("cargo:rustc-link-lib=static=opencv_videoio");
    println!("cargo:rustc-link-lib=static=opencv_imgcodecs");
    println!("cargo:rustc-link-lib=static=opencv_imgproc");
    println!("cargo:rustc-link-lib=static=opencv_core");

    println!("cargo:rustc-link-lib=ittnotify");
    println!("cargo:rustc-link-lib=libprotobuf");
    println!("cargo:rustc-link-lib=libjpeg-turbo");
    println!("cargo:rustc-link-lib=libpng");
    println!("cargo:rustc-link-lib=libtiff");
    println!("cargo:rustc-link-lib=libjasper");
    println!("cargo:rustc-link-lib=quirc");
    println!("cargo:rustc-link-lib=ade");
    println!("cargo:rustc-link-lib=gthread-2.0");
    println!("cargo:rustc-link-lib=glib-2.0");
    println!("cargo:rustc-link-lib=z");
    println!("cargo:rustc-link-lib=webp");
    println!("cargo:rustc-link-lib=dl");
    println!("cargo:rustc-link-lib=m");
    println!("cargo:rustc-link-lib=pthread");
    println!("cargo:rustc-link-lib=rt");
    



    cxx_build::bridge("src/image_processing.rs")
        .file("src/cv_face_blurring.cc")
        .include("/usr/local/include/opencv4/opencv")
        .include("/usr/local/include/opencv4")
        .flag("-Wall")
        .flag("-Wpedantic")
        .flag_if_supported("-std=c++14")
        .compile("anonynews_rs");
    
    println!("cargo:rerun-if-changed=src/image_processing.rs");
    println!("cargo:rerun-if-changed=src/cv_face_blurring.cc");
    println!("cargo:rerun-if-changed=include/cv_face_blurring.h");
}