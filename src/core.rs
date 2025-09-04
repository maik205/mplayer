use ffmpeg_next::{
    self as ffmpeg, Rational,
    codec::{Context, Parameters},
    format::Pixel,
    frame::{Audio, Video},
    media::Type,
    software::scaling::Flags,
};
use std::{
    collections::VecDeque,
    sync::{
        Mutex, RwLock,
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
};

use ffmpeg::{Error, Packet, Stream, format::input};

use crate::{
    constants::ConvFormat,
    decode::{MDecodeOptions, MediaInfo},
    utils::{Range, height_from_ar},
};

pub struct MPlayerCore {
    packet_queue: RwLock<VecDeque<(Packet, PacketMarker)>>,
    read_location: RwLock<u16>,
    look_range: Range,
    video: Option<DecodeThread<Video>>,
    audio: Option<DecodeThread<Audio>>,
    config: &'static MDecodeOptions,
    media_info: Option<MediaInfo>,
    officer_he_has_a_gun: Option<SyncSender<PacketDistributorCommand>>,
    what_gun: Option<JoinHandle<()>>,
    has_media: bool,
}

pub enum PacketDistributorCommand {
    Exit,
    MoveCursor(u32),
}

impl MPlayerCore {
    pub fn new(config: Option<&'static MDecodeOptions>) -> MPlayerCore {
        MPlayerCore {
            packet_queue: RwLock::new(VecDeque::new()),
            video: None,
            audio: None,
            config: config.unwrap_or_default(),
            media_info: None,
            has_media: false,
            officer_he_has_a_gun: None,
            what_gun: None,
            look_range: Range::new(10, 50),
            read_location: RwLock::new(0),
        }
    }

    pub fn open_media(
        &mut self,
        path: String,
        decode_options: Option<MDecodeOptions>,
    ) -> Result<(), Error> {
        let mut input_ctx = input(&path)?;
        let decode_options = decode_options.unwrap_or_default();

        let mut video_tx = None;
        let mut audio_tx = None;

        if let Some(video_stream) = input_ctx.streams().best(ffmpeg_next::media::Type::Video) {
            let (p_tx_video, p_rx_video) = mpsc::channel();
            video_tx = Some(p_tx_video);
            self.video = Some(DecodeThread::<Video>::spawn(
                video_stream.parameters(),
                p_rx_video,
                Some("video".to_string()),
                Some(ThreadConfig {
                    buffer_capacity: 10,
                }),
                Mutex::new(decode_options),
            ));
        }

        if let Some(video_stream) = input_ctx.streams().best(ffmpeg_next::media::Type::Audio) {
            let (p_tx_audio, p_rx_audio) = mpsc::channel();
            audio_tx = Some(p_tx_audio);
            self.audio = Some(DecodeThread::<Audio>::spawn(
                video_stream.parameters(),
                p_rx_audio,
                Some("audio".to_string()),
                Some(ThreadConfig {
                    buffer_capacity: 10,
                }),
            ));
        }
        let (tx_cmd_packets, rx_cmd_packets) = mpsc::channel::<PacketDistributorCommand>();
        let gun = thread::Builder::new()
            .name("packeteer".to_string())
            .spawn(|| {
                let rx_cmd_packets = rx_cmd_packets;
                let internal_counter = 0;
                loop {
                    if let Ok(command) = rx_cmd_packets.try_recv() {}
                }
            })
            .unwrap();
        if let Ok(mut lock) = self.packet_queue.write() {
            for (stream, packet) in input_ctx.packets() {
                lock.push_back((packet.clone(), stream.convert()));
            }
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

struct PacketMarker {
    stream_id: usize,
}

impl ConvFormat<PacketMarker> for Stream<'_> {
    fn convert(&self) -> PacketMarker {
        PacketMarker {
            stream_id: self.index(),
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
#[derive(Clone, Copy)]
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
        scaling_config: Mutex<MDecodeOptions>,
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

                let mut scaling_config_ = None;
                {
                    scaling_config_ = Some(scaling_config.lock().unwrap().clone()); 
                }
                let scaling_config = scaling_config_.unwrap();

                let mut scaling_context = ffmpeg::software::scaling::Context::get(
                    video_decoder.format(),
                    video_decoder.width(),
                    video_decoder.height(),
                    scaling_config.pixel_format,
                    scaling_config.window_default_size.0,
                    height_from_ar(
                        Rational(video_decoder.width() as i32, video_decoder.height() as i32),
                        scaling_config.window_default_size.0,
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
            .expect("Unable to spawn video thread, you may not have enough memory/cpu resource available.");
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
    ) -> DecodeThread<Audio> {
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
                let mut frame_buffer = Audio::empty();
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
