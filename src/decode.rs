use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ffmpeg_next::{self as ffmpeg, Rational, Stream, format};
use ffmpeg_next::{
    format::input,
    media::{self},
    software::{self},
    util::frame,
};

use crate::utils::{Range, RangeCheck, print_context_data};

#[derive(Debug, Clone)]
pub struct MDecodeOptions {
    pub scaling_flag: software::scaling::Flags,
    // The look range provides an upper limit to the decoder so that it wouldnt fetch more and also a lower bound to check the decoder health.
    pub look_range: Range,
    pub window_default_size: (u32, u32),
}
pub struct MDecode {
    // pub inner: MDecodeInner,
    pub decoder_stats: MDecoderStats,
    pub decoder_output: Receiver<Option<DecoderOutput>>,
    pub decoder_commander: Sender<Option<DecoderCommand>>,
    pub is_active: bool,
    pub thread_reference: JoinHandle<()>,
}

pub struct MDecodeInner {
    pub input_ctx: ffmpeg::format::context::Input,
    pub scaling_ctx: ffmpeg::software::scaling::Context,
    pub decoder: ffmpeg_next::codec::decoder::video::Video,
    pub current_video_stream_index: usize,
    pub options: MDecodeOptions,
    pub media_info: MediaInfo, // pub current_audio_stream_index: usize,
}
#[derive(Debug, Clone, Copy)]
pub struct MediaInfo {
    pub v_width: u32,
    pub v_height: u32,
    pub video_rate: Rational,
    pub aspect_ratio: Rational,
}

impl Default for MDecodeOptions {
    fn default() -> Self {
        Self {
            scaling_flag: software::scaling::Flags::BILINEAR,
            look_range: Range::new(5, 20),
            window_default_size: (1920, 1080),
        }
    }
}

pub struct MDecodeFrame {
    pub frame: frame::video::Video,
    pub cur_stats: MDecoderStats,
}
// The iterator ends when the video is over;
impl Iterator for &mut MDecode {
    type Item = DecoderOutput;

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

pub enum DecoderOutput {
    Frame(MDecodeFrame),
    MediaInfo(MediaInfo),
    Status(MDecoderStats),
}

pub fn init(decode_options: Option<MDecodeOptions>) -> MDecode {
    let (decoder_tx, context_rx) = mpsc::channel::<Option<DecoderOutput>>();
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
                                let decode_inner = get_decoder(path, decode_options.clone())
                                    .expect("The decoder can't be initialized");

                                // Tell the renderer about the media to be displayed.
                                let _ = decoder_tx.send(Some(DecoderOutput::MediaInfo(
                                    decode_inner.media_info.clone(),
                                )));
                                // Put the decoder in the container!
                                let _ = inner.insert(decode_inner);

                                let mut frame_buffer = frame::video::Video::empty();
                                // Decode the stream frame by frame (thank you ffmpeg, you're magical)
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
                                            }
                                        }
                                    }
                                    if let RangeCheck::Higher = decode_options
                                        .clone()
                                        .unwrap_or_default()
                                        .look_range
                                        .range_check_inclusive(to_take)
                                    {
                                        // The decoder thread can sleep while waiting for the renderer to finish eating the rest of the frames.
                                        // thread::sleep(Duration::from_millis(16));
                                        continue;
                                    }
                                    if let Some(ref mut m_decode) = inner {
                                        let timer = Instant::now();
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
                                        if let Ok(_) = decoder_tx.send(Some(DecoderOutput::Frame(
                                            MDecodeFrame {
                                                frame: output_buffer,
                                                cur_stats: MDecoderStats {
                                                    decode_latency: timer.elapsed().as_secs_f32(),
                                                },
                                            },
                                        ))) {
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
                    } else {
                        return;
                    }
                }
                Err(_) => {}
            }
        }
    });
    println!("[DEBUG] Decoder thread spawned");
    MDecode {
        decoder_stats: MDecoderStats {
            decode_latency: -1.0,
        },
        decoder_output: context_rx,
        decoder_commander: context_tx,
        is_active: false,
        thread_reference: decoder_thread,
    }
}

impl MDecodeInner {
    pub fn get_media_info(&self) -> &MediaInfo {
        return &self.media_info;
    }
}

pub fn get_decoder(
    path: String,
    decode_options: Option<MDecodeOptions>,
) -> Result<MDecodeInner, MDecodeError> {
    if let Ok(input_ctx) = input(&path) {
        let _ = print_context_data(&input_ctx);

        if let Some(video_stream) = input_ctx.streams().best(media::Type::Video)
            && let Ok(decoder_ctx) =
                ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
            && let Ok(decoder) = decoder_ctx.decoder().video()
        {
            let decode_options = decode_options.clone().unwrap_or(MDecodeOptions::default());

            //Extract the media's information and store it in the decoder inner information.
            let media_info = MediaInfo {
                v_width: decoder.width(),
                v_height: decoder.height(),
                video_rate: video_stream.rate(),
                aspect_ratio: decoder.aspect_ratio(),
            };

            let ar = Rational::new(decoder.width() as i32, decoder.height() as i32);
            println!(
                "{}",
                crate::utils::height_from_ar(ar, decode_options.window_default_size.0,)
            );
            if let Ok(scaling_ctx) = software::scaling::Context::get(
                decoder.format(),
                decoder.width(),
                decoder.height(),
                format::Pixel::RGB24,
                decode_options.window_default_size.0,
                crate::utils::height_from_ar(ar, decode_options.window_default_size.0),
                decode_options.scaling_flag,
            ) {
                let stream_index = video_stream.index().clone();
                return Ok(MDecodeInner {
                    input_ctx,
                    scaling_ctx,
                    decoder,
                    options: decode_options,
                    current_video_stream_index: stream_index,
                    media_info,
                });
            }
        }
    }
    return Err(MDecodeError::ContextCantBeInitialized);
}

impl MDecode {
    pub fn open_video(
        &mut self,
        path: String,
        decode_options: Option<MDecodeOptions>,
    ) -> Result<(), MDecodeError> {
        self.decoder_commander
            .send(Some(DecoderCommand::Open(path)))
            .unwrap();
        self.is_active = true;
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

#[derive(Debug)]
pub struct MDecoderStats {
    pub decode_latency: f32,
}
