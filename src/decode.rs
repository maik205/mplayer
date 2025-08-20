use std::marker::PhantomData;

use ffmpeg_next as ffmpeg;
use ffmpeg_next::{
    format::input,
    media::{self},
    software::{self},
    util::frame::video::Video,
};

struct MDecode {
    input_ctx: ffmpeg::format::context::Input,
    scaling_ctx: ffmpeg::software::scaling::Context,
    decoder: ffmpeg_next::codec::decoder::video::Video,
    options: MDecodeOptions,
    pub video_stream_index: usize,
}

struct MDecodeOptions {
    scaling_flag: software::scaling::Flags,
    output_w: u32,
    output_h: u32,
}

struct MDecodeFrame {
    decoder_frame: Video,
}

// The iterator ends when the video is over;
impl Iterator for MDecode {
    type Item = MDecodeFrame;

    fn next(&mut self) -> Option<Self::Item> {
        let mut frame_buffer = Video::empty();
        while !self.decoder.receive_frame(&mut frame_buffer).is_ok() {
            if let Some((stream, packet)) = self.input_ctx.packets().next() {
                if stream.index() == self.video_stream_index {
                    if let Ok(_) = self.decoder.send_packet(&packet) {
                        continue;
                    }
                }
            }
        }
        let mut output_buffer = Video::empty();
        if let Err(_) = self.scaling_ctx.run(&frame_buffer, &mut output_buffer) {
            return None;
        }
        return Some(MDecodeFrame {
            decoder_frame: output_buffer,
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
                    decoder.format(),
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

enum MDecodeError {
    FileNotFound,
    VideoStreamNotFound,
    ContextCantBeInitialized,
}
enum StreamKind {
    Video,
    Audio,
    Subtitle,
}
