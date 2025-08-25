use std::collections::{VecDeque, vec_deque};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use ffmpeg_next::{self as ffmpeg, Rational, format};
use ffmpeg_next::{
    format::input,
    media::{self},
    software::{self},
    util::frame,
};

use crate::utils::Range;

#[derive(Debug, Clone)]
pub struct MDecodeOptions {
    pub scaling_flag: software::scaling::Flags,
    pub output_w: u32,
    pub output_h: u32,
    pub aspect_ratio: Rational,
    // The look range provides an upper limit to the decoder so that it wouldnt fetch more and also a lower bound to check the decoder health.
    pub look_range: Range,
}
pub struct MDecode {
    // pub inner: MDecodeInner,
    pub decoder_stats: MDecoderStats,
    pub decoder_output: Receiver<Option<MDecodeFrame>>,
    pub decoder_commander: Sender<DecoderCommand>,
    pub is_active: bool,
    pub thread_reference: JoinHandle<()>,
}

pub struct MDecodeInner {
    pub input_ctx: ffmpeg::format::context::Input,
    pub scaling_ctx: ffmpeg::software::scaling::Context,
    pub decoder: ffmpeg_next::codec::decoder::video::Video,
    pub options: MDecodeOptions,
    pub current_video_stream_index: usize,
    // pub current_audio_stream_index: usize,
}

impl Default for MDecodeOptions {
    fn default() -> Self {
        Self {
            scaling_flag: software::scaling::Flags::BILINEAR,
            output_w: 1920,
            output_h: 1080,
            look_range: Range::new(5, 20),
            aspect_ratio: Rational(16, 9),
        }
    }
}

pub struct MDecodeFrame {
    pub frame: frame::video::Video,
}
// The iterator ends when the video is over;
impl Iterator for &mut MDecode {
    type Item = MDecodeFrame;

    fn next(&mut self) -> Option<Self::Item> {
        return Some(MDecodeFrame {
            frame: output_buffer,
        });
    }
}

enum DecoderCommand {
    Open(String),
    Goto(u32),
    Option(MDecodeOptions),
    Take,
    Clean,
}
pub fn init(decode_options: Option<MDecodeOptions>) -> MDecode {
    let (decoder_tx, context_rx) = mpsc::channel::<Option<MDecodeFrame>>();
    let (context_tx, decoder_rx) = mpsc::channel::<DecoderCommand>();

    let decoder_thread = thread::spawn(move || {
        let decoder_tx = decoder_tx;
        let decoder_rx = decoder_rx;
        let mut buffer_size = 0;
        let mut inner = None;
        loop {
            match decoder_rx.try_recv() {
                Ok(command) => match command {
                    DecoderCommand::Open(path) => {
                        if let Ok(input_ctx) = input(&path) {
                            if let Some(video_stream) = input_ctx.streams().best(media::Type::Video)
                                && let Ok(decoder_ctx) =
                                    ffmpeg::codec::context::Context::from_parameters(
                                        video_stream.parameters(),
                                    )
                                && let Ok(decoder) = decoder_ctx.decoder().video()
                            {
                                let mut decode_options =
                                    decode_options.clone().unwrap_or(MDecodeOptions::default());
                                decode_options.aspect_ratio = decoder.aspect_ratio();
                                if let Ok(scaling_ctx) = software::scaling::Context::get(
                                    decoder.format(),
                                    decoder.width(),
                                    decoder.height(),
                                    format::Pixel::RGB24,
                                    decode_options.output_w,
                                    (decode_options.output_h
                                        * decode_options.aspect_ratio.1 as u32)
                                        / decode_options.aspect_ratio.0 as u32,
                                    decode_options.scaling_flag,
                                ) {
                                    let stream_index = video_stream.index().clone();

                                    inner.insert(MDecodeInner {
                                        input_ctx,
                                        scaling_ctx,
                                        decoder,
                                        options: decode_options,
                                        current_video_stream_index: stream_index,
                                    });

                                    let mut frame_buffer = frame::video::Video::empty();
                                    if let Some(mut m_decode) = inner {
                                        while !m_decode
                                            .decoder
                                            .receive_frame(&mut frame_buffer)
                                            .is_ok()
                                        {
                                            if let Some((stream, packet)) =
                                                m_decode.input_ctx.packets().next()
                                            {
                                                if m_decode.current_video_stream_index
                                                    == stream.index()
                                                {
                                                    if let Ok(_) =
                                                        m_decode.decoder.send_packet(&packet)
                                                    {
                                                        continue;
                                                    }
                                                }
                                            } else {
                                                decoder_tx.send(None);
                                            }
                                        }
                                        let mut output_buffer = frame::video::Video::empty();
                                        if let Err(_) = m_decode
                                            .scaling_ctx
                                            .run(&frame_buffer, &mut output_buffer)
                                        {
                                            decoder_tx.send(None);
                                        }

                                        decoder_tx.send(Some(MDecodeFrame {
                                            frame: output_buffer,
                                        }));
                                    }
                                }
                            }
                        }
                    }
                    DecoderCommand::Goto(_) => todo!(),
                    DecoderCommand::Option(decode_options) => todo!(),
                    DecoderCommand::Take => todo!(),
                    DecoderCommand::Clean => todo!(),
                },
                _ => {}
            }
        }
    });
    todo!();
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
                let decode_options = decode_options.unwrap_or(MDecodeOptions::default());

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
                        frame_buffer: Arc::new(Mutex::new(vec_deque::VecDeque::new())),
                        is_active: false,
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

    // pub fn feed_audio(&mut self, frame: frame::audio::Audio) {
    //     self.audio_buffer.push_back(frame);
    // }
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

pub struct MDecodeOptionBuilder {
    options: MDecodeOptions,
}

impl MDecodeOptionBuilder {
    pub fn new() -> MDecodeOptionBuilder {
        return MDecodeOptionBuilder {
            options: MDecodeOptions::default(),
        };
    }
    pub fn width(mut self, width: u32) -> MDecodeOptionBuilder {
        self.options.output_w = width;
        return self;
    }
    pub fn height(mut self, height: u32) -> MDecodeOptionBuilder {
        self.options.output_h = height;
        return self;
    }
}
