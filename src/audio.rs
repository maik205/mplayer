use std::{
    ops::Div,
    sync::mpsc::{self, Receiver, Sender, SyncSender},
};

use ffmpeg_next::frame::Audio;
use sdl3::{
    Error, Sdl,
    audio::{AudioCallback, AudioFormatNum, AudioSpec, AudioStreamWithCallback},
};

pub fn init_audio_subsystem(sdl: &Sdl, spec: AudioSpec) -> Result<MPlayerAudio, Error> {
    let audio = sdl.audio()?;
    let (tx, rx) = mpsc::sync_channel(100);
    let ctx = MPlayerAudioCallbackCtx::new(rx);
    let mut spec = spec;
    spec.channels = Some(2);

    let device = match spec.format {
        Some(sdl3::audio::AudioFormat::S16LE) => {
            audio.open_playback_stream::<MPlayerAudioCallbackCtx, i16>(&spec, ctx)?
        }
        Some(sdl3::audio::AudioFormat::S32LE) => {
            audio.open_playback_stream::<MPlayerAudioCallbackCtx, i32>(&spec, ctx)?
        }
        Some(sdl3::audio::AudioFormat::F32LE) => {
            let mut spec = spec;
            if let Some(freq) = spec.freq {
                let _ = spec.freq.insert(freq / spec.channels.unwrap());
            }
            audio.open_playback_stream::<MPlayerAudioCallbackCtx, f32>(&spec, ctx)?
        }
        Some(sdl3::audio::AudioFormat::U8) => {
            audio.open_playback_stream::<MPlayerAudioCallbackCtx, u8>(&spec, ctx)?
        }
        Some(sdl3::audio::AudioFormat::S8) => {
            audio.open_playback_stream::<MPlayerAudioCallbackCtx, i8>(&spec, ctx)?
        }
        _ => {
            panic!("Unsupported audio format.");
        }
    };
    let _ = device.resume();

    let audio_sys = MPlayerAudio { tx, device: device };

    Ok(audio_sys)
}

pub struct MPlayerAudio {
    pub tx: SyncSender<Audio>,
    pub device: AudioStreamWithCallback<MPlayerAudioCallbackCtx>,
}

pub struct MPlayerAudioCallbackCtx {
    recv: Receiver<Audio>,
}

impl MPlayerAudioCallbackCtx {
    pub fn new(audio_rx: Receiver<Audio>) -> MPlayerAudioCallbackCtx {
        MPlayerAudioCallbackCtx { recv: audio_rx }
    }
}

impl<T> AudioCallback<T> for MPlayerAudioCallbackCtx
where
    T: AudioFormatNum,
{
    fn callback(&mut self, stream: &mut sdl3::audio::AudioStream, _: i32) {
        match self.recv.try_recv() {
            Ok(frame) => {
                let _ = stream.put_data(frame.data(0));
            }
            Err(_) => {}
        }
    }
}
