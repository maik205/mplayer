use ffmpeg_next::format::{sample::Type, Pixel, Sample};
use sdl3::{audio::AudioFormat, pixels::PixelFormatEnum};

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
            Sample::F32(_) => AudioFormat::F32BE,
            Sample::F64(_) => AudioFormat::UNKNOWN,
            _ => AudioFormat::UNKNOWN
        }
    }
}

impl ConvFormat<PixelFormatEnum> for Pixel {
    fn convert(&self) -> PixelFormatEnum {
        match self {
            Pixel::RGB24 => PixelFormatEnum::RGB24,
            Pixel::BGR24 => PixelFormatEnum::BGR24,
            Pixel::ARGB => PixelFormatEnum::ARGB8888,
            Pixel::RGBA => PixelFormatEnum::RGBA8888,
            Pixel::ABGR => PixelFormatEnum::ABGR8888,
            Pixel::BGRA => PixelFormatEnum::BGRA8888,
            Pixel::RGB565LE => PixelFormatEnum::RGB565,
            Pixel::BGR565LE => PixelFormatEnum::BGR565,
            Pixel::NV12 => PixelFormatEnum::NV12,
            Pixel::NV21 => PixelFormatEnum::NV21,
            Pixel::YUYV422 => PixelFormatEnum::YUY2,
            Pixel::UYVY422 => PixelFormatEnum::UYVY,
            Pixel::YVYU422 => PixelFormatEnum::YVYU,
            Pixel::RGB32 => PixelFormatEnum::RGB332,
            Pixel::YUV420P => PixelFormatEnum::UYVY,
            Pixel::YUV422P => PixelFormatEnum::YUY2,
            _ => PixelFormatEnum::Unknown,
        }
    }
}
