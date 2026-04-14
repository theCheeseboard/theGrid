use std::io::{BufReader, Cursor, Read, Seek};
use std::thread;

pub enum SoundEffect {
    Notification,
    CallJoin,
    CallLeave,
    MuteOff,
    MuteOn
}

impl SoundEffect {
    pub fn play(&self) {
        match self {
            SoundEffect::Notification => play_sound_effect(include_bytes!("../assets/sfx/notification.wav")),
            SoundEffect::CallJoin => play_sound_effect(include_bytes!("../assets/sfx/call_join.ogg")),
            SoundEffect::CallLeave => play_sound_effect(include_bytes!("../assets/sfx/call_leave.ogg")),
            SoundEffect::MuteOff => play_sound_effect(include_bytes!("../assets/sfx/mute_off.ogg")),
            SoundEffect::MuteOn => play_sound_effect(include_bytes!("../assets/sfx/mute_on.ogg"))
        }
    }
}

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
