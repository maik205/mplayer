use std::sync::LazyLock;

use ffmpeg_next::{
    self as ffmpeg, Rational, Stream,
    codec::Context,
    decoder::Audio,
    format::{Pixel, context::Input},
    media,
    software::{self, scaling::Flags},
};
use sdl3::audio::{AudioFormat, AudioSpec};

use crate::{constants::ConvFormat, mplayer::OPTS};

#[derive(Clone)]
pub struct Range {
    pub min: u32,
    pub max: u32,
}

impl std::fmt::Debug for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Range")
            .field("min", &self.min)
            .field("max", &self.max)
            .finish()
    }
}
impl Range {
    pub fn new(min: u32, max: u32) -> Range {
        Range { min, max }
    }

    pub fn range_check_inclusive(&self, num: u32) -> RangeCheck {
        if num > self.max {
            return RangeCheck::Higher;
        }
        if num < self.min {
            return RangeCheck::Lower;
        }
        RangeCheck::InRange
    }

    pub fn range_check(&self, num: u32) -> RangeCheck {
        if num >= self.max {
            return RangeCheck::Higher;
        }
        if num <= self.min {
            return RangeCheck::Lower;
        }
        RangeCheck::InRange
    }
}

// Copied from https://github.com/zmwangx/rust-ffmpeg/blob/master/examples/metadata.rs
pub fn print_context_data(context: &Input) -> Result<(), ffmpeg::Error> {
    for (k, v) in context.metadata().iter() {
        println!("{}: {}", k, v);
    }

    if let Some(stream) = context.streams().best(ffmpeg::media::Type::Video) {
        println!("Best video stream index: {}", stream.index());
    }

    if let Some(stream) = context.streams().best(ffmpeg::media::Type::Audio) {
        println!("Best audio stream index: {}", stream.index());
    }

    if let Some(stream) = context.streams().best(ffmpeg::media::Type::Subtitle) {
        println!("Best subtitle stream index: {}", stream.index());
    }

    println!(
        "duration (seconds): {:.2}",
        context.duration() as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE)
    );
    for stream in context.streams() {
        println!("stream index {}:", stream.index());
        println!("\ttime_base: {}", stream.time_base());
        println!("\tstart_time: {}", stream.start_time());
        println!("\tduration (stream timebase): {}", stream.duration());
        println!(
            "\tduration (seconds): {:.2}",
            stream.duration() as f64 * f64::from(stream.time_base())
        );
        println!("\tframes: {}", stream.frames());
        println!("\tdisposition: {:?}", stream.disposition());
        println!("\tdiscard: {:?}", stream.discard());
        println!("\trate: {}", stream.rate());

        let codec = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
        println!("\tmedium: {:?}", codec.medium());
        println!("\tid: {:?}", codec.id());

        if codec.medium() == ffmpeg::media::Type::Video {
            if let Ok(video) = codec.decoder().video() {
                println!("\tbit_rate: {}", video.bit_rate());
                println!("\tmax_rate: {}", video.max_bit_rate());
                println!("\tdelay: {}", video.delay());
                println!("\tvideo.width: {}", video.width());
                println!("\tvideo.height: {}", video.height());
                println!("\tvideo.format: {:?}", video.format());
                println!("\tvideo.has_b_frames: {}", video.has_b_frames());
                println!("\tvideo.aspect_ratio: {}", video.aspect_ratio());
                println!("\tvideo.color_space: {:?}", video.color_space());
                println!("\tvideo.color_range: {:?}", video.color_range());
                println!("\tvideo.color_primaries: {:?}", video.color_primaries());
                println!(
                    "\tvideo.color_transfer_characteristic: {:?}",
                    video.color_transfer_characteristic()
                );
                println!("\tvideo.chroma_location: {:?}", video.chroma_location());
                println!("\tvideo.references: {}", video.references());
                println!("\tvideo.intra_dc_precision: {}", video.intra_dc_precision());
            }
        } else if codec.medium() == ffmpeg::media::Type::Audio {
            if let Ok(audio) = codec.decoder().audio() {
                println!("\tbit_rate: {}", audio.bit_rate());
                println!("\tmax_rate: {}", audio.max_bit_rate());
                println!("\tdelay: {}", audio.delay());
                println!("\taudio.rate: {}", audio.rate());
                println!("\taudio.channels: {}", audio.channels());
                println!("\taudio.format: {:?}", audio.format());
                println!("\taudio.frames: {}", audio.frames());
                println!("\taudio.align: {}", audio.align());
                println!("\taudio.channel_layout: {:?}", audio.channel_layout());
            }
        }
    }
    Ok(())
}
pub fn height_from_ar(aspect_ratio: Rational, width: u32) -> u32 {
    return width * aspect_ratio.1 as u32 / aspect_ratio.0 as u32;
}

pub fn width_from_ar(aspect_ratio: Rational, height: u32) -> u32 {
    return height * aspect_ratio.1 as u32 / aspect_ratio.0 as u32;
}

pub fn frame_time_ms(rate: Rational) -> i32 {
    return (1000 * rate.1 / rate.0) as i32;
}

pub fn frame_time_ns(rate: Rational) -> i32 {
    return (1_000_000_000 as u64 * rate.1 as u64 / rate.0 as u64) as i32;
}

pub enum RangeCheck {
    Lower,
    InRange,
    Higher,
}

pub fn calculate_tpf_from_time_base(time_base: Rational, frame_rate: Rational) -> i64 {
    if time_base.0 == 0 || frame_rate.0 == 0 {
        return 0;
    }
    let res = ((frame_rate.0 as i64 * time_base.1 as i64)
        / (frame_rate.1 as i64 * time_base.0 as i64))
        / (frame_rate.0 / frame_rate.1) as i64;
    return res;
}

#[derive(Debug, Clone)]
pub struct MDecodeOptions {
    pub scaling_flag: software::scaling::Flags,
    // The look range provides an upper limit to the decoder so that it wouldnt fetch more and also a lower bound to check the decoder health.
    pub look_range: Range,
    pub window_default_size: (u32, u32),
    pub pixel_format: Pixel,
}
impl Default for MDecodeOptions {
    fn default() -> Self {
        MDecodeOptions {
            look_range: Range::new(5, 15),
            scaling_flag: Flags::BILINEAR,
            window_default_size: (1920, 1080),
            pixel_format: Pixel::RGB24,
        }
    }
}

impl Default for &MDecodeOptions {
    fn default() -> Self {
        &OPTS
    }
}
#[derive(Debug, Clone, Copy)]
pub struct MediaInfo {
    pub v_width: u32,
    pub v_height: u32,
    pub video_rate: Rational,
    pub aspect_ratio: Rational,
    pub frame_time_ns: i32,
    pub frame_time_ms: i32,
    pub audio_spec: AudioSpec,
    pub time_base: Rational,
}

impl MediaInfo {
    pub fn get_media_info_from_input(input: Input) -> MediaInfo {
        let v_stream = input
            .streams()
            .best(media::Type::Video)
            .expect("No video stream found");
        let a_stream = input
            .streams()
            .best(media::Type::Audio)
            .expect("No audio stream found");

        let video_decoder = Context::from_parameters(v_stream.parameters())
            .expect("Failed to get video decoder context")
            .decoder()
            .video()
            .expect("Failed to get video decoder");

        let audio_decoder = Context::from_parameters(a_stream.parameters())
            .expect("Failed to get audio decoder context")
            .decoder()
            .audio()
            .expect("Failed to get audio decoder");

        MediaInfo {
            v_width: video_decoder.width(),
            v_height: video_decoder.height(),
            video_rate: v_stream.rate(),
            aspect_ratio: video_decoder.aspect_ratio(),
            frame_time_ms: frame_time_ms(v_stream.rate()),
            frame_time_ns: frame_time_ns(v_stream.rate()),
            audio_spec: AudioSpec {
                freq: Some(audio_decoder.rate() as i32),
                channels: Some(audio_decoder.channels().into()),
                format: Some(audio_decoder.format().convert()),
            },
            time_base: video_decoder.time_base(),
        }
    }
}

impl ConvFormat<AudioSpec> for Audio {
    fn convert(&self) -> AudioSpec {
        AudioSpec {
            freq: Some((self.rate() as i32) / 2),
            channels: Some(self.channel_layout().channels()),
            format: Some(self.format().convert()),
        }
    }
}
trait Distance {
    fn distance(&self, other: Self) -> Self;
}
impl Distance for i64 {
    fn distance(&self, other: Self) -> Self {
        (other - self).abs()
    }
}
pub enum TimeScale {
    Nano = 1_000_000,
    Mili = 1_000,
}
pub fn calculate_wait_from_rational(time_base: Rational, scale: TimeScale) -> u64 {
    // time_base = x/y seconds per unit (usually frame or tick)
    // To get the duration of one unit in the desired scale:
    // duration = (x / y) seconds * scale (e.g., 1_000_000_000 for ns)
    // = (x * scale) / y
    let scale_val = match scale {
        TimeScale::Nano => 1_000_000_000,
        TimeScale::Mili => 1_000,
    };
    if time_base.0 == 0 {
        return 0;
    }
    ((time_base.0 as f64 * scale_val as f64) / time_base.1 as f64) as u64
}

pub fn time_base_to_ns(time_base: Rational) -> u32 {
    ((1_000_000_000 * time_base.0) / time_base.1).abs() as u32
}
