#include <iostream>
#include "anonynews_rs/include/cv_face_blurring.h"

namespace anonynews_rs
{

    void printHelloFromCxx()
    {
        std::cout << "Hello from the CXX part of the thing!" << std::endl;
    }

    static cv::dnn::Net faceDetectorNet;
    static cv::dnn::Net faceEmbedderNet;

    void loadFaceDetectorNet(
        rust::Slice<const uint8_t> bufProto,
        rust::Slice<const uint8_t> bufModel)
    {
        faceDetectorNet = cv::dnn::readNetFromCaffe(
            reinterpret_cast<const char *>(bufProto.data()),
            bufProto.size(),
            reinterpret_cast<const char *>(bufModel.data()), 
            bufModel.size()
        );
    }

    void loadFaceEmbedderNet(
        rust::Slice<const uint8_t> bufModel)
    {
        std::string torchBytes(reinterpret_cast<const char *>(bufModel.data()), bufModel.size());
        faceEmbedderNet = cv::dnn::readNetFromTorch(torchBytes);
    }

}
