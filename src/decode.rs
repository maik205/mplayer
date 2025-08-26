use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};

use ffmpeg_next::{self as ffmpeg, Rational, format};
use ffmpeg_next::{
    format::input,
    media::{self},
    software::{self},
    util::frame,
};

use crate::utils::{Range, RangeCheck};

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
    pub decoder_commander: Sender<Option<DecoderCommand>>,
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
        if let Ok(frame) = self.decoder_output.recv() {
            let _ = self.decoder_commander.send(Some(DecoderCommand::Take));

            return frame;
        }
        return None;
    }
}

pub enum DecoderCommand {
    Open(String),
    Goto(u32),
    Option(MDecodeOptions),
    Take,
    Clean,
}
pub fn init(decode_options: Option<MDecodeOptions>) -> MDecode {
    let (decoder_tx, context_rx) = mpsc::channel::<Option<MDecodeFrame>>();
    let (context_tx, decoder_rx) = mpsc::channel::<Option<DecoderCommand>>();

    let decoder_thread = thread::spawn(move || {
        let decoder_tx = decoder_tx;
        let decoder_rx = decoder_rx;
        let mut to_take = 0;
        let mut inner = None;
        loop {
            match decoder_rx.try_recv() {
                Ok(command) => {
                    if let Some(command) = command {
                        match command {
                            DecoderCommand::Open(path) => {
                                let _ = inner.insert(
                                    get_decoder(path, decode_options.clone())
                                        .expect("The decoder can't be initialized"),
                                );

                                let mut frame_buffer = frame::video::Video::empty();
                                loop {
                                    if let Ok(command) = decoder_rx.try_recv() {
                                        match command {
                                            Some(DecoderCommand::Open(path)) => {
                                                let _ = inner
                                                    .insert(get_decoder(path, None).expect(
                                                    "The decoding context can't be initialized.",
                                                ));
                                                to_take = 0;
                                            }
                                            Some(DecoderCommand::Goto(_)) => todo!(),
                                            Some(DecoderCommand::Option(mdecode_options)) => {
                                                // Find the differences and process as necessary
                                            }
                                            Some(DecoderCommand::Take) => {
                                                to_take -= 1;
                                            }
                                            Some(DecoderCommand::Clean) => {}
                                            None => {
                                                std::process::exit(1);
                                            },
                                        }
                                    }
                                    if let RangeCheck::Higher = decode_options
                                        .clone()
                                        .unwrap()
                                        .look_range
                                        .range_check_inclusive(to_take)
                                    {
                                        continue;
                                    }
                                    if let Some(ref mut m_decode) = inner {
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
                                                let _ = decoder_tx.send(None);
                                            }
                                        }
                                        let mut output_buffer = frame::video::Video::empty();
                                        if let Err(_) = m_decode
                                            .scaling_ctx
                                            .run(&frame_buffer, &mut output_buffer)
                                        {
                                            let _ = decoder_tx.send(None);
                                        }
                                        if let Ok(_) = decoder_tx.send(Some(MDecodeFrame {
                                            frame: output_buffer,
                                        })) {
                                            to_take += 1;
                                        }
                                    }
                                }
                            }
                            DecoderCommand::Goto(_) => todo!(),
                            DecoderCommand::Option(mdecode_options) => todo!(),
                            DecoderCommand::Take => panic!(),
                            DecoderCommand::Clean => todo!(),
                        }
                    }
                    else {
                        return;
                    }
                }
                Err(_) => {},
            }
        }
    });
    MDecode {
        decoder_stats: MDecoderStats {
            time_to_frame: -1.0,
        },
        decoder_output: context_rx,
        decoder_commander: context_tx,
        is_active: true,
        thread_reference: decoder_thread,
    }
}

pub fn get_decoder(
    path: String,
    decode_options: Option<MDecodeOptions>,
) -> Result<MDecodeInner, MDecodeError> {
    if let Ok(input_ctx) = input(&path) {
        if let Some(video_stream) = input_ctx.streams().best(media::Type::Video)
            && let Ok(decoder_ctx) =
                ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
            && let Ok(decoder) = decoder_ctx.decoder().video()
        {
            let mut decode_options = decode_options.clone().unwrap_or(MDecodeOptions::default());
            decode_options.aspect_ratio = decoder.aspect_ratio();
            if let Ok(scaling_ctx) = software::scaling::Context::get(
                decoder.format(),
                decoder.width(),
                decoder.height(),
                format::Pixel::RGB24,
                decode_options.output_w,
                (decode_options.output_h * decode_options.aspect_ratio.1 as u32)
                    / decode_options.aspect_ratio.0 as u32,
                decode_options.scaling_flag,
            ) {
                let stream_index = video_stream.index().clone();
                return Ok(MDecodeInner {
                    input_ctx,
                    scaling_ctx,
                    decoder,
                    options: decode_options,
                    current_video_stream_index: stream_index,
                });
            }
        }
    }
    return Err(MDecodeError::ContextCantBeInitialized);
}

impl MDecode {
    pub fn open_video(
        &mut self,
        path: &str,
        decode_options: Option<MDecodeOptions>,
    ) -> Result<(), MDecodeError> {
        self.decoder_commander.send(Some(DecoderCommand::Open(path.to_owned()))).unwrap();
        Ok(())
    }

    // pub fn feed_audio(&mut self, frame: frame::audio::Audio) {
    //     self.audio_buffer.push_back(frame);
    // }
}

#[derive(Debug)]
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
