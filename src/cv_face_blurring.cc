#include <iostream>
#include "anonynews_rs/include/cv_face_blurring.h"
#include <cassert>

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
            bufModel.size());
    }

    void loadFaceEmbedderNet(
        const std::string &pathname)
    {
        // std::string torchBytes(reinterpret_cast<const char *>(bufModel.data()), bufModel.size());
        faceEmbedderNet = cv::dnn::readNetFromTorch(pathname);
    }

    cv::Mat cvMatrixFromPNGBuffer(rust::Slice<const uint8_t> pngBuffer)
    {
        auto decodedMatrix = cv::imdecode(cv::_InputArray(pngBuffer.data(), pngBuffer.size()), cv::IMREAD_UNCHANGED);
        return decodedMatrix;
    }

    void saveCvImageToFile(std::unique_ptr<cv::Mat> img, const std::string &filename)
    {
        cv::imwrite(filename, *img);
    }

    std::vector<cv::Rect2i> findFaces(cv::Mat image)
    {
        const double CONFIDENCE_THRESHOLD = 0.2;

        double height = image.rows;
        double width = image.cols;

        // cv::Mat resized;
        // cv::resize(image, resized, cv::Size(300, 300));

        auto blob = cv::dnn::blobFromImage(image, 1.0, cv::Size(300, 300), cv::Scalar(104.0, 177.0, 123.0));

        faceDetectorNet.setInput(blob);
        cv::Mat detection = faceDetectorNet.forward();
        cv::Mat detectionMatrix(detection.size[2],
                                detection.size[3],
                                CV_32F,
                                detection.ptr<float>());

        std::vector<cv::Rect2i> regions;
        for (int i = 0; i < detectionMatrix.rows; i++)
        {
            auto confidence = detectionMatrix.at<float>(i, 2);
            double x1 = detectionMatrix.at<float>(i, 3) * width;
            double y1 = detectionMatrix.at<float>(i, 4) * height;
            double x2 = detectionMatrix.at<float>(i, 5) * width;
            double y2 = detectionMatrix.at<float>(i, 6) * height;

            if (confidence > CONFIDENCE_THRESHOLD)
            {
                bool valid =
                    0 <= x1 && x1 <= x2 && x2 <= width &&
                    0 <= y1 && y1 <= y2 && y2 <= height;

                if (valid)
                {
                    regions.push_back(cv::Rect2i(
                        cv::Point2i(x1, y1),
                        cv::Point2i(x2, y2)));
                }
            }
        }

        return regions;
    }

    struct EmbeddingResults
    {
        cv::Mat faceVec;
        cv::Rect2i region;

        EmbeddingResults(cv::Mat fv, cv::Rect2i r) : faceVec(std::move(fv)), region(r) {}
    };

    std::vector<EmbeddingResults> getEmbeddings(const cv::Mat &origImage)
    {
        const int MINIMUM_SIZE = 20;

        double width = origImage.cols;
        double height = origImage.rows;

        double new_width = 600;
        double width_scaling = width / new_width;

        double new_height = height / width_scaling;

        cv::Mat resizedImage;
        cv::resize(origImage, resizedImage, cv::Size2i(new_width, new_height));

        auto faceRegions = findFaces(resizedImage);

        std::vector<EmbeddingResults> embeddingResults;

        for (auto &region : faceRegions)
        {
            if (region.width < MINIMUM_SIZE || region.height < MINIMUM_SIZE)
            {
                continue;
            }
            cv::Mat face = resizedImage(region);
            cv::Mat faceBlob = cv::dnn::blobFromImage(face, 1.0 / 255.0, cv::Size2i(96, 96), cv::Scalar(0, 0, 0), true, false);
            faceEmbedderNet.setInput(faceBlob);

            cv::Mat faceVec;
            faceEmbedderNet.forward(faceVec);

            cv::Rect2i rescaledRegion(
                region.x * width_scaling,
                region.y * width_scaling,
                region.width * width_scaling,
                region.height * width_scaling);

            embeddingResults.push_back(EmbeddingResults(
                faceVec, rescaledRegion));
        }

        return embeddingResults;
    }

    std::vector<cv::Rect2i> findRegionsToBlur(std::vector<EmbeddingResults> embeddingResults, std::vector<cv::Mat> dontBlurTheseFaces)
    {
        std::vector<cv::Rect2i> ret;

        for (auto &er : embeddingResults)
        {
            ret.push_back(er.region);
        }

        return ret;
    }

    cv::Mat blurRegions(cv::Mat imageToBlur, std::vector<cv::Rect2i> regionsToBlur)
    {
        const auto GAUSSIAN_KERNEL = cv::Size(80, 80);

        for (auto &rec : regionsToBlur)
        {
            // FIXME: blur the margin as well
            cv::blur(imageToBlur(rec), imageToBlur(rec), GAUSSIAN_KERNEL);
        }

        return imageToBlur;
    }

    cv::Mat blur(cv::Mat toBlur)
    {
        auto embeddings = getEmbeddings(toBlur);
        // TODO: load whitelist somehow
        auto regionsToBlur = findRegionsToBlur(std::move(embeddings), std::vector<cv::Mat>());
        return blurRegions(toBlur, std::move(regionsToBlur));
    }

    std::unique_ptr<std::vector<uint8_t>> blurFFMpegFrame(rust::Slice<const uint8_t> pngBuffer)
    {
        auto cvMat = cvMatrixFromPNGBuffer(pngBuffer);
        auto blurredMat = blur(std::move(cvMat));

        std::unique_ptr<std::vector<uint8_t>> ret = std::make_unique<std::vector<uint8_t>>();

        cv::Mat flat = blurredMat.reshape(1, blurredMat.total() * blurredMat.channels());
        *ret = blurredMat.isContinuous() ? flat : flat.clone();

        return ret;
    }
}

//// This main function is handy for testing stuff out
// int main()
// {
//     anonynews_rs::faceDetectorNet = cv::dnn::readNetFromCaffe(
//         "/workspaces/anonynews_rs/models/deploy.prototxt",
//         "/workspaces/anonynews_rs/models/res10_300x300_ssd_iter_140000_fp16.caffemodel");

//     anonynews_rs::faceEmbedderNet = cv::dnn::readNetFromTorch(
//         "/workspaces/anonynews_rs/models/openface_nn4.small2.v1.t7");

//     cv::Mat people = cv::imread("/workspaces/anonynews_rs/face.jpeg");

//     // cv::resize(people, people, cv::Size(600, (600.0 / people.cols) * people.rows));

//     std::cout << people.channels() << std::endl;
//     auto blurred = anonynews_rs::blur(people);

//     cv::imwrite("/workspaces/anonynews_rs/temp/boxed.png", blurred);

//     return 0;
// }