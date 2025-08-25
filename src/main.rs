use std::{
    io::{self, BufRead},
    process::exit,
    sync::mpsc,
    thread,
};

use crate::mplayer::MPlayer;

mod decode;
mod mplayer;
mod utils;

fn main() {
    let (tx, rx) = mpsc::channel::<Command>();
    let player_thread = thread::spawn(move || {
        let player = MPlayer::setup();
        match player {
            Ok(mut player) => loop {
                if player.should_exit {
                    exit(0);
                }
                if player.decoder.is_none() {
                    if let Ok(command) = rx.recv() {
                        player.tick(Some(command));
                    }
                } else {
                    let mut cli_command = None;
                    if let Ok(command) = rx.try_recv() {
                        cli_command = Some(command);
                    }
                    player.tick(cli_command);
                }
            },
            Err(err) => {
                println!("{:?}", err);
            },
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
                    val if val.contains("open") => {
                        if let Some(dir) = val.split("open").nth(1) {
                            let _ =
                                tx.send(Command::Play(String::from(dir.replace("\"", "").trim())));
                        }
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
