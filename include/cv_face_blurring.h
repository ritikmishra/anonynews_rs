#ifndef CV_FACE_BLURRING_H__
#define CV_FACE_BLURRING_H__

#include <opencv2/core.hpp>
#include <opencv2/dnn/dnn.hpp>
#include <rust/cxx.h>

namespace anonynews_rs
{

    void printHelloFromCxx();

    void loadFaceDetectorNet(
        rust::Slice<const uint8_t> bufProto,
        rust::Slice<const uint8_t> bufModel
    );

    void loadFaceEmbedderNet(
        rust::Slice<const uint8_t> bufModel
    );

}

#endif