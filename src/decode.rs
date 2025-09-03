use std::collections::VecDeque;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ffmpeg_next::codec::{Context, audio};
use ffmpeg_next::format::{Pixel, Sample};
use ffmpeg_next::frame::Audio;
use ffmpeg_next::{self as ffmpeg, format, Packet, Rational, Stream};
use ffmpeg_next::{
    format::input,
    media::{self},
    software::{self},
    util::frame,
};
use sdl3::Sdl;
use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec};
use sdl3::pixels::PixelFormatEnum;

use crate::audio::{MPlayerAudio, init_audio_subsystem};
use crate::constants::ConvFormat;
use crate::utils::{Range, RangeCheck, frame_time_ms, frame_time_ns, print_context_data};

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
    pub audio: MPlayerAudio,
}

pub struct MDecodeInner {
    pub input_ctx: ffmpeg::format::context::Input,
    pub scaling_ctx: ffmpeg::software::scaling::Context,
    pub video_decoder: ffmpeg_next::codec::decoder::video::Video,
    pub audio_decoder: ffmpeg::codec::decoder::audio::Audio,
    pub current_video_stream_index: usize,
    pub options: MDecodeOptions,
    pub media_info: MediaInfo,
    pub current_audio_stream_index: usize,
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
}

impl Default for MDecodeOptions {
    fn default() -> Self {
        Self {
            scaling_flag: software::scaling::Flags::BILINEAR,
            look_range: Range::new(2, 15),
            window_default_size: (1152, 648),
        }
    }
}

pub struct MDecodeVideoFrame {
    pub video_frame: frame::video::Video,
}
// The iterator ends when the video is over;
impl Iterator for &mut MDecode {
    type Item = DecoderOutput;

    fn next(&mut self) -> Option<Self::Item> {
        if let Ok(frame) = self.decoder_output.recv() {
            if let Some(_) = frame {
                let _ = self.decoder_commander.send(Some(DecoderCommand::Take));
            }
            return frame;
        }
        return None;
    }
}
pub struct MDecodeAudioFrame {
    pub audio_frame: frame::audio::Audio,
}

pub enum DecoderCommand {
    Open(String),
    Goto(u32),
    Option(MDecodeOptions),
    Take,
    Clean,
}

pub enum DecoderOutput {
    Video(MDecodeVideoFrame),
    Audio(MDecodeAudioFrame),
    MediaInfo(MediaInfo),
}

pub fn init(decode_options: Option<MDecodeOptions>, sdl: &Sdl) -> MDecode {
    let (decoder_tx, context_rx) = mpsc::channel::<Option<DecoderOutput>>();
    let (context_tx, decoder_rx) = mpsc::channel::<Option<DecoderCommand>>();
    let audio_spec: AudioSpec = AudioSpec {
        freq: Some(44100 / 2),
        channels: Some(2),
        format: Some(AudioFormat::F32LE),
    };
    let audio = init_audio_subsystem(sdl, &audio_spec).unwrap();
    let audio_tx_inner = audio.tx.clone();
    let decoder_thread = thread::spawn(move || {
        let decoder_tx = decoder_tx;
        let decoder_rx = decoder_rx;
        let audio_tx_inner = audio_tx_inner;
        let mut to_take = 2;
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

                                let mut video_frame_buffer = frame::video::Video::empty();
                                let mut audio_frame_buffer = frame::audio::Audio::empty();
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
                                    if let Some(ref mut inner) = inner {
                                        // For benchmarking purposes.
                                        let timer = Instant::now();

                                        // The frequency of audio frames is MUCH higher compared to video frames, doing this is lethally slow for the audio decoder.
                                        // Sending it over the same channel as the video would also create contention between audio and video ? right? lets test it out before microoptimizing to death
                                        while let Ok(()) = inner
                                            .audio_decoder
                                            .receive_frame(&mut audio_frame_buffer)
                                        {
                                            audio_tx_inner
                                                .send(audio_frame_buffer.clone())
                                                .unwrap();
                                        }

                                        while let Ok(()) = inner
                                            .video_decoder
                                            .receive_frame(&mut video_frame_buffer)
                                        {
                                            let mut video_frame = frame::video::Video::empty();
                                            if let Err(_) = inner
                                                .scaling_ctx
                                                .run(&video_frame_buffer, &mut video_frame)
                                            {
                                                let _ = decoder_tx.send(None);
                                            }
                                            if let Ok(_) =
                                                decoder_tx.send(Some(DecoderOutput::Video(
                                                    MDecodeVideoFrame { video_frame },
                                                )))
                                            {
                                                to_take += 1;
                                            }
                                        }

                                        if let Some((stream, packet)) =
                                            inner.input_ctx.packets().next()
                                        {
                                            match stream.index() {
                                                stream_indx
                                                    if stream_indx
                                                        == inner.current_audio_stream_index =>
                                                {
                                                    let _ =
                                                        inner.audio_decoder.send_packet(&packet);
                                                }
                                                stream_indx
                                                    if stream_indx
                                                        == inner.current_video_stream_index =>
                                                {
                                                    let _ =
                                                        inner.video_decoder.send_packet(&packet);
                                                }
                                                _ => {}
                                            }
                                        } else {
                                            let _ = decoder_tx.send(None);
                                        }
                                    }
                                }
                            }
                            DecoderCommand::Goto(_) => todo!(),
                            DecoderCommand::Option(mdecode_options) => todo!(),
                            DecoderCommand::Take => panic!(),
                            DecoderCommand::Clean => {
                                to_take = 0;
                            }
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
        audio,
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

        // ily ffmpeg!
        if let Some(video_stream) = input_ctx.streams().best(media::Type::Video)
            && let Some(audio_stream) = input_ctx.streams().best(media::Type::Audio)
            && let Ok(decoder_ctx) = Context::from_parameters(video_stream.parameters())
            && let Ok(a_decoder_context) = Context::from_parameters(audio_stream.parameters())
            && let Ok(video_decoder) = decoder_ctx.decoder().video()
            && let Ok(audio_decoder) = a_decoder_context.decoder().audio()
        {
            let decode_options = decode_options.clone().unwrap_or(MDecodeOptions::default());

            //Extract the media's information and store it in the decoder inner information.
            let media_info = MediaInfo {
                v_width: video_decoder.width(),
                v_height: video_decoder.height(),
                video_rate: video_stream.rate(),
                aspect_ratio: video_decoder.aspect_ratio(),
                frame_time_ms: frame_time_ms(video_stream.rate()),
                frame_time_ns: frame_time_ns(video_stream.rate()),
                audio_spec: AudioSpec {
                    freq: Some((audio_decoder.rate()).clone() as i32),
                    channels: Some(audio_decoder.channels().into()),
                    format: Some(audio_decoder.format().convert()),
                },
            };

            let ar = Rational::new(video_decoder.width() as i32, video_decoder.height() as i32);

            if let Ok(scaling_ctx) = software::scaling::Context::get(
                video_decoder.format(),
                video_decoder.width(),
                video_decoder.height(),
                Pixel::RGB24,
                decode_options.window_default_size.0,
                crate::utils::height_from_ar(ar, decode_options.window_default_size.0),
                decode_options.scaling_flag,
            ) {
                let video_stream_index = video_stream.index().clone();
                let audio_stream_index = audio_stream.index().clone();
                return Ok(MDecodeInner {
                    input_ctx,
                    scaling_ctx,
                    video_decoder,
                    audio_decoder,
                    options: decode_options,
                    current_video_stream_index: video_stream_index,
                    current_audio_stream_index: audio_stream_index,
                    media_info,
                });
            }
        }
        return Err(MDecodeError::ContextCantBeInitialized);
    } else {
        return Err(MDecodeError::FileNotFound);
    }
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

