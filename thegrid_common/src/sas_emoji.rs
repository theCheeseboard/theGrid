use cntp_i18n::I18N_MANAGER;
use imbl::HashMap;
use matrix_sdk::encryption::verification::Emoji;
use serde::Deserialize;
use std::sync::LazyLock;

static EMOJI_DATA: LazyLock<Vec<SasEmojiJsonEntry>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/sas-emoji/sas-emoji.json")).unwrap()
});

#[derive(Deserialize)]
struct SasEmojiJsonEntry {
    number: u32,
    emoji: String,
    description: String,
    unicode: String,
    translated_descriptions: HashMap<String, Option<String>>,
}

pub trait SasEmoji {
    fn translated_description(&self) -> String;
}

impl SasEmoji for Emoji {
    fn translated_description(&self) -> String {
        let current_locales = I18N_MANAGER.locale().messages;

        EMOJI_DATA
            .iter()
            .find(|entry| entry.emoji == self.symbol)
            .and_then(|entry| {
                for locale in current_locales {
                    if let Some(Some(description)) =
                        entry.translated_descriptions.get(&locale.replace("-", "_"))
                    {
                        return Some(description.to_string());
                    }
                }
                None
            })
            .unwrap_or_else(|| self.description.to_string())
    }
}
