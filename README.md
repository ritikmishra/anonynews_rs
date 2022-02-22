# rtmp-faceblur-proxy

This is a RTMP proxy that blurs all of the faces in a stream that are not 
on a whitelist

This project is a rewrite of the RTMP server in [anony.news](https://github.com/MiniHacks/anonygoose/).
For that project, I had written the RTMP server in Python, and was unsatisfied 
with how latent and memory-hungry it was.

## Usage

uh . . actually it doesn't quite work yet `¯\_(ツ)_/¯`

### TODOs

- [x] Accept RTMP connection
- [x] Split stream into video, audio
- [x] Split video into frames
- [x] Blur the frames
- [ ] Turn the frames back into video
- [ ] Combine video with audio
- [ ] Stream back to RTMP destination server
- [ ] Be fast
- [ ] Be memory efficient
- [ ] Be robust

## Building

The build system used is Cargo (even though some of it is written in C++). 
In order to build the project, you need ffmpeg and
OpenCV. Frankly, your best chance at building this project is to build it inside
a docker container. The `Dockerfile` inside `.devcontainer` installs both for you.
