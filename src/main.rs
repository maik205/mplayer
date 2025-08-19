use std::{
    io::{self, BufRead},
    process::exit,
    sync::mpsc,
    thread,
};

use crate::mplayer::MPlayer;

mod decode;
mod mplayer;

fn main() {
    let (tx, rx) = mpsc::channel::<Command>();
    let player_thread = thread::spawn(move || {
        if let Ok(mut player) = MPlayer::setup() {
            loop {
                let mut tick_cmd = None;
                if let Ok(command) = rx.try_recv() {
                    tick_cmd = Some(command);
                };
                
                player.tick(tick_cmd);

                if player.should_exit {
                    exit(0);
                }
            }
        } else {
            println!("Player initialization failed");
        }
    });

    'command: loop {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            if let Ok(line) = line {
                match line {
                    val if val == "exit".to_string() => {
                        if let Ok(_) = tx.send(Command::Shutdown) {
                            println!("shutting down mplayer");
                            if let Ok(_) = player_thread.join() {}
                        }
                        break 'command;
                    }
                    _ => {}
                }
            }
        }
    }
}

enum Command {
    Shutdown,
    Play(String),
    Pause,
    Goto(u32),
}
