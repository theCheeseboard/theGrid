use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

pub struct TheGridError {
    message: String,
}

impl TheGridError {
    pub fn new(message: &str) -> Self {
        Self { message: message.to_string() }
    }
}

impl Debug for TheGridError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message.as_str())
    }
}

impl Display for TheGridError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message.as_str())
    }
}

impl Error for TheGridError {}
