# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.217.4/containers/rust/.devcontainer/base.Dockerfile

# [Choice] Debian OS version (use bullseye on local arm64/Apple Silicon): buster, bullseye
ARG VARIANT="buster"
FROM mcr.microsoft.com/vscode/devcontainers/rust:0-${VARIANT}

# [Optional] Uncomment this section to install additional packages.
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends \
        fish \
        ffmpeg \
        libavutil-dev \
        libavformat-dev \
        libavdevice-dev \
        libavcodec-dev \
        libclang-dev \
        clang \ 
        wget \
        build-essential \
        cmake

RUN wget https://github.com/opencv/opencv/archive/refs/tags/4.2.0.zip
RUN unzip 4.2.0.zip -d opencv
RUN cd opencv/opencv-4.2.0 && \
    mkdir -p build/ && \
    cd build && \
    cmake \
        -D CMAKE_BUILD_TYPE=RELEASE \
        -D CMAKE_INSTALL_PREFIX=/usr/local \
        -D BUILD_DOCS=OFF \
        -D BUILD_EXAMPLES=OFF \
        -D BUILD_opencv_apps=OFF \
        -D BUILD_opencv_python2=OFF \
        -D BUILD_opencv_python3=OFF \
        -D BUILD_PERF_TESTS=OFF \
#        We want to statically link opencv into our program
        -D BUILD_SHARED_LIBS=OFF \ 
        -D OPENCV_GENERATE_PKGCONFIG=ON \
        -D BUILD_TESTS=OFF \
        -D ENABLE_PRECOMPILED_HEADERS=OFF\
        -D FORCE_VTK=OFF \
        -D WITH_FFMPEG=ON \
        -D WITH_GDAL=OFF \ 
        -D WITH_IPP=OFF \
        -D WITH_OPENEXR=OFF \
        -D WITH_OPENGL=OFF \ 
        -D WITH_QT=OFF\
        -D WITH_TBB=OFF \ 
        -D WITH_XINE=OFF \ 
        -D BUILD_JPEG=ON  \
        -D BUILD_TIFF=ON \
        -D BUILD_PNG=ON \
    ../ && \
    make && \
    make install

RUN wget https://github.com/rui314/mold/releases/download/v1.2.1/mold-1.2.1-x86_64-linux.tar.gz && \
    tar xf mold-1.2.1-x86_64-linux.tar.gz && \
    cp -r mold-1.2.1-x86_64-linux/* /usr/local && \
    rm /usr/bin/ld && \
    ln -s /usr/local/bin/mold /usr/bin/ld

