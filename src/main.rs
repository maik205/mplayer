use crate::mplayer::{MPlayer, MPlayerShouldExit};

mod mplayer;

fn main() {
    if let Ok(mut player) = MPlayer::setup() {
        'main: loop {
            if player.tick() {
                break 'main;
            }
        }
    } else {
        println!("Player initialization failed");
    }
}
