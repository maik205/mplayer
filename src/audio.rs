use std::sync::mpsc::{self, Receiver, Sender};

use ffmpeg_next::frame::Audio;
use sdl3::{
    Error, Sdl,
    audio::{AudioCallback, AudioSpec, AudioStreamWithCallback},
};

pub fn init_audio_subsystem<'a>(sdl: &Sdl, spec: AudioSpec) -> Result<MPlayerAudio, Error> {
    let audio = sdl.audio()?;
    let (tx, rx) = mpsc::channel();
    let ctx = MPlayerAudioCallbackCtx::new(rx);
    let device: sdl3::audio::AudioStreamWithCallback<MPlayerAudioCallbackCtx> =
        audio.open_playback_stream(&spec, ctx).expect("");
    let _ = device.resume();

    let audio_sys = MPlayerAudio { tx, device: device };

    Ok(audio_sys)
}

pub struct MPlayerAudio {
    pub tx: Sender<Audio>,
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

impl AudioCallback<f32> for MPlayerAudioCallbackCtx {
    fn callback(&mut self, stream: &mut sdl3::audio::AudioStream, _: i32) {
        match self.recv.try_recv() {
            Ok(frame) => {
                let _ = stream.put_data(frame.data(0));
            }
            Err(_) => {}
        }
    }
}
