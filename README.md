<img width="120" alt="mplayer logo ig" src="https://github.com/user-attachments/assets/689cee92-33ec-4932-aaee-f524fd3a4d20" />

## mplayer
A media player written in Rust, leveraging [FFmpeg](https://ffmpeg.org/) for media decoding and [SDL3](https://github.com/libsdl-org/SDL) for window creation, audio playback and rendering.  
**mplayer** aims to provide a fast, cross-platform, and customizable media playback experience.

---


## Features

- [x] SDL3-based video and audio output
- [x] Play most common video and audio formats
- [ ] Simple OSD
- [ ] Video seeking and playback speed control
- [ ] Subtitle support for video stream
- [ ] External subtitle support
- [ ] Rescaling
- [ ] Keyboard shortcuts
- [ ] Hardware acceleration (wgpu)
- [ ] Settings/configuration file
- [ ] Drag-and-drop file loading

---

## Screenshots
<img width="560" height="569" alt="image" src="https://github.com/user-attachments/assets/dab28e09-290a-45b3-8d65-71f589e3fd2c" />


---

## Getting Started

### Prerequisites

- [Rust toolchain](https://www.rust-lang.org/tools/install)
- FFmpeg development libraries
- SDL3 development libraries

### Build

```sh
git clone https://github.com/maik205/mplayer.git
cd mplayer
cargo build --release
```

### Run

```sh
> cargo run

> open <path or URL to media>
```

---

## Contributing

Contributions and feedback are welcomed!  

---

## Acknowledgments

- [FFmpeg](https://ffmpeg.org/)
- [SDL3](https://github.com/libsdl-org/SDL)
