use ffmpeg_next::format::{ Pixel, Sample, sample::Type };
use sdl3::{ audio::AudioFormat, pixels::PixelFormat };

pub trait ConvFormat<To> {
    fn convert(&self) -> To;
}

impl ConvFormat<AudioFormat> for Sample {
    fn convert(&self) -> AudioFormat {
        match self {
            Sample::None => AudioFormat::UNKNOWN,
            Sample::U8(_) => AudioFormat::U8,
            Sample::I16(_) => AudioFormat::S16LE,
            Sample::I32(_) => AudioFormat::S32LE,
            Sample::I64(Type::Planar) => AudioFormat::UNKNOWN,
            Sample::F32(_) => AudioFormat::F32LE,
            Sample::F64(_) => AudioFormat::UNKNOWN,
            _ => AudioFormat::UNKNOWN,
        }
    }
}

impl ConvFormat<PixelFormat> for Pixel {
    fn convert(&self) -> PixelFormat {
        match self {
            Pixel::RGB24 => PixelFormat::RGB24,
            Pixel::BGR24 => PixelFormat::BGR24,
            Pixel::ARGB => PixelFormat::ARGB8888,
            Pixel::RGBA => PixelFormat::RGBA8888,
            Pixel::ABGR => PixelFormat::ABGR8888,
            Pixel::BGRA => PixelFormat::BGRA8888,
            Pixel::RGB565LE => PixelFormat::RGB565,
            Pixel::BGR565LE => PixelFormat::BGR565,
            Pixel::NV12 => PixelFormat::NV12,
            Pixel::NV21 => PixelFormat::NV21,
            Pixel::YUYV422 => PixelFormat::YUY2,
            Pixel::UYVY422 => PixelFormat::UYVY,
            Pixel::YVYU422 => PixelFormat::YVYU,
            Pixel::RGB32 => PixelFormat::RGB332,
            Pixel::YUV420P => PixelFormat::UYVY,
            Pixel::YUV422P => PixelFormat::YUY2,
            _ => PixelFormat::UNKNOWN,
        }
    }
}
