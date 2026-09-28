//! Noticing when what someone writes sounds like real trouble, so a helpline
//! can sit quietly beside the sky. Nothing is sent anywhere or recorded.

use serde::Deserialize;

#[derive(Deserialize)]
struct Phrases {
    phrases: Vec<String>,
}

pub struct Care {
    phrases: Vec<String>,
}

pub const NOTE: &str = "You don't have to carry this alone tonight. Samaritans, free, any time: 116 123 (UK and Ireland). Anywhere else: findahelpline.com";

impl Care {
    pub fn bundled() -> Care {
        Care {
            phrases: toml::from_str::<Phrases>(include_str!("../data/care.toml"))
                .expect("care.toml")
                .phrases,
        }
    }

    pub fn concerning(&self, text: &str) -> bool {
        let lower = text.to_lowercase().replace('’', "'");
        self.phrases.iter().any(|p| lower.contains(p.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_trouble_is_noticed_and_ordinary_worry_isnt() {
        let care = Care::bundled();
        assert!(care.concerning("honestly I want to die"));
        assert!(care.concerning("I can’t go on like this"));
        assert!(!care.concerning("The boiler, and work"));
        assert!(!care.concerning("my dog died last year"));
    }
}
