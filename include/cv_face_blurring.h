#ifndef CV_FACE_BLURRING_H__
#define CV_FACE_BLURRING_H__

#include <memory>

#include <opencv2/core.hpp>
#include <opencv2/dnn/dnn.hpp>
#include <opencv2/imgproc.hpp>
#include <opencv2/imgcodecs.hpp>
#include <rust/cxx.h>

namespace anonynews_rs
{

    void printHelloFromCxx();

    void loadFaceDetectorNet(
        rust::Slice<const uint8_t> bufProto,
        rust::Slice<const uint8_t> bufModel
    );

    void loadFaceEmbedderNet(
        const std::string& pathname
    );

    std::unique_ptr<std::vector<uint8_t>> blurFFMpegFrame(rust::Slice<const uint8_t> pngBuffer, const std::string& filename);
}

#endif