use ffmpeg_next::{self as ffmpeg, Rational, format::context::Input};

#[derive(Clone)]
pub struct Range {
    min: u32,
    max: u32,
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
