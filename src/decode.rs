use std::time::Instant;

use ffmpeg_next::{self as ffmpeg, Format, format};
use ffmpeg_next::{
    format::input,
    media::{self},
    software::{self},
    util::frame::video::Video,
};

pub struct MDecode {
    input_ctx: ffmpeg::format::context::Input,
    scaling_ctx: ffmpeg::software::scaling::Context,
    decoder: ffmpeg_next::codec::decoder::video::Video,
    pub options: MDecodeOptions,
    pub video_stream_index: usize,
    pub decoder_stats: MDecoderStats,
}
#[derive(Debug)]
pub struct MDecodeOptions {
    pub scaling_flag: software::scaling::Flags,
    pub output_w: u32,
    pub output_h: u32,
}

pub struct MDecodeFrame {
    pub frame: Video,
}

// The iterator ends when the video is over;
impl Iterator for &mut MDecode {
    type Item = MDecodeFrame;

    fn next(&mut self) -> Option<Self::Item> {
        let start = Instant::now();
        let mut frame_buffer = Video::empty();
        while !self.decoder.receive_frame(&mut frame_buffer).is_ok() {
            if let Some((stream, packet)) = self.input_ctx.packets().next() {
                if stream.index() == self.video_stream_index {
                    if let Ok(_) = self.decoder.send_packet(&packet) {
                        continue;
                    }
                }
            } else {
                return None;
            }
        }
        let mut output_buffer = Video::empty();
        if let Err(_) = self.scaling_ctx.run(&frame_buffer, &mut output_buffer) {
            return None;
        }
        self.decoder_stats.time_to_frame = start.elapsed().as_secs_f32();
        return Some(MDecodeFrame {
            frame: output_buffer,
        });
    }
}

impl MDecode {
    pub fn open_video(
        path: &str,
        decode_options: Option<MDecodeOptions>,
    ) -> Result<MDecode, MDecodeError> {
        if let Ok(input_ctx) = input(path) {
            if let Some(video_stream) = input_ctx.streams().best(media::Type::Video)
                && let Ok(decoder_ctx) =
                    ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
                && let Ok(decoder) = decoder_ctx.decoder().video()
            {
                let decode_options = decode_options.unwrap_or(MDecodeOptions {
                    output_w: decoder.width(),
                    output_h: decoder.height(),
                    scaling_flag: software::scaling::Flags::BILINEAR,
                });

                if let Ok(scaling_ctx) = software::scaling::Context::get(
                    decoder.format(),
                    decoder.width(),
                    decoder.height(),
                    format::Pixel::RGB24,
                    decode_options.output_w,
                    decode_options.output_h,
                    decode_options.scaling_flag,
                ) {
                    let stream_index = video_stream.index().clone();
                    return Ok(MDecode {
                        input_ctx,
                        scaling_ctx,
                        decoder,
                        options: decode_options,
                        video_stream_index: stream_index,
                        decoder_stats: MDecoderStats {
                            time_to_frame: -1.0,
                        },
                    });
                } else {
                    return Err(MDecodeError::ContextCantBeInitialized);
                }
            } else {
                return Err(MDecodeError::VideoStreamNotFound);
            }
        } else {
            return Err(MDecodeError::FileNotFound);
        }
    }
}

pub enum MDecodeError {
    FileNotFound,
    VideoStreamNotFound,
    ContextCantBeInitialized,
}
pub enum StreamKind {
    Video,
    Audio,
    Subtitle,
}

pub struct MDecoderStats {
    pub time_to_frame: f32,
}
