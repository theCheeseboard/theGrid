use std::io::{BufReader, Cursor, Read, Seek};
use std::thread;

pub fn play_sound_effect<R: Send + Sync + 'static + AsRef<[u8]>>(file: R) {
    thread::spawn(move || {
        let Ok(sink_handle) = rodio::DeviceSinkBuilder::open_default_sink() else {
            return;
        };

        let file = Cursor::new(file);
        let Ok(player) = rodio::play(sink_handle.mixer(), file) else {
            return;
        };

        player.sleep_until_end()
    });
}
