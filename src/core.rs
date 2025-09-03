use ffmpeg_next::{
    self as ffmpeg, Frame, Rational,
    codec::{Context, Parameters, context, traits::Decoder},
    decoder,
    format::Pixel,
    frame::{Audio, Video},
    software::scaling::Flags,
};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender, SyncSender},
    },
    thread::{self, JoinHandle, Thread},
    time::Instant,
};

use ffmpeg::{Error, Packet, Stream, format::input};

use crate::{decode::MediaInfo, utils::height_from_ar};

pub struct MPlayerCore<'a> {
    packet_queue: VecDeque<(Packet, Stream<'a>)>,
    video: Option<DecodeThread<Video>>,
    audio: Option<DecodeThread<Audio>>,
    config: &'static MPlayerConfig,
    media_info: Option<MediaInfo>,
}

pub struct MPlayerConfig {}

impl Default for MPlayerConfig {
    fn default() -> Self {
        Self {}
    }
}

impl MPlayerCore<'_> {
    pub fn new(config: &'static MPlayerConfig) -> MPlayerCore<'static> {
        MPlayerCore {
            packet_queue: VecDeque::new(),
            video: None,
            audio: None,
            config: config,
            media_info: None,
        }
    }

    pub fn open_media(&mut self, path: String) -> Result<(), Error> {
        let input_ctx = input(&path)?;

        if let Some(video_stream) = input_ctx.streams().best(ffmpeg_next::media::Type::Video) {
            // DecodeThread::<Video>::spawn(
            //     context::Context::from_parameters(video_stream.parameters()).unwrap(),
            // );
        }
        Ok(())
    }
}

impl Default for ThreadConfig {
    fn default() -> Self {
        Self {
            buffer_capacity: 10,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ThreadConfig {
    buffer_capacity: u16,
}

pub enum ThreadData {
    Packet(Packet),
    Kill,
}

pub struct DecodeThread<OutputType> {
    pub handle: JoinHandle<()>,
    pub output_rx: Receiver<OutputType>,
}

pub struct ScalingConfig {
    pub scaling_flag: Flags,
    pub width: u32,
    pub aspect_ratio: Rational,
    pub pixel_format: Pixel,
}

impl DecodeThread<Video> {
    pub fn spawn(
        parameters: Parameters,
        packet_rx: Receiver<ThreadData>,
        thread_name: Option<String>,
        config: Option<ThreadConfig>,
        scaling_config: Mutex<ScalingConfig>,
    ) -> DecodeThread<Video> {
        let config = config.unwrap_or_default();
        let (output_tx, output_rx) = mpsc::sync_channel(config.buffer_capacity.into());

        let handle = thread::Builder::new()
            .name(thread_name.unwrap_or("media".to_string()))
            .spawn(move || {
                let mut video_decoder = Context::from_parameters(parameters)
                    .unwrap()
                    .decoder()
                    .video()
                    .unwrap();
                let scaling_config = scaling_config.lock().unwrap();
                let mut scaling_context = ffmpeg::software::scaling::Context::get(
                    video_decoder.format(),
                    video_decoder.width(),
                    video_decoder.height(),
                    scaling_config.pixel_format,
                    scaling_config.width,
                    height_from_ar(
                        Rational(video_decoder.width() as i32, video_decoder.height() as i32),
                        video_decoder.width(),
                    ),
                    scaling_config.scaling_flag,
                );
                let mut frame_buffer = Video::empty();
                while let Ok(ThreadData::Packet(packet)) = packet_rx.recv() {
                    video_decoder.send_packet(&packet).unwrap();
                    if let Ok(_) = video_decoder.receive_frame(&mut frame_buffer) {
                        if let Ok(ref mut scaler) = scaling_context {
                            let mut output_buffer = Video::empty();
                            if let Ok(()) = scaler.run(&frame_buffer, &mut output_buffer) {
                                let _ = output_tx.send(output_buffer);
                                continue;
                            }
                        }
                        let _ = output_tx.send(frame_buffer.clone());
                    }
                }
            })
            .expect("Unable to spawn video thread, you may not have enough memory available.");
        DecodeThread {
            handle,
            output_rx: output_rx,
        }
    }
}

impl DecodeThread<Audio> {
    pub fn spawn(
        parameters: Parameters,
        packet_rx: Receiver<ThreadData>,
        thread_name: Option<String>,
        config: Option<ThreadConfig>,
    ) -> DecodeThread<Video> {
        let config = config.unwrap_or_default();
        let (output_tx, output_rx) = mpsc::sync_channel(config.buffer_capacity.into());

        let handle = thread::Builder::new()
            .name(thread_name.unwrap_or("media".to_string()))
            .spawn(move || {
                let mut audio_decoder = Context::from_parameters(parameters)
                    .unwrap()
                    .decoder()
                    .audio()
                    .unwrap();
                let mut frame_buffer = Video::empty();
                while let Ok(ThreadData::Packet(packet)) = packet_rx.recv() {
                    audio_decoder.send_packet(&packet).unwrap();
                    if let Ok(_) = audio_decoder.receive_frame(&mut frame_buffer) {
                        output_tx.send(frame_buffer.clone()).unwrap();
                    }
                }
            })
            .expect("Unable to spawn media thread.");
        DecodeThread {
            handle,
            output_rx: output_rx,
        }
    }
}
