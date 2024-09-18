use std::env;
use std::fmt;

pub struct Config {
    pub socket_addr: String,
    pub channel: String,
}

impl Config {
    pub fn new() -> Result<Self,ConfigError> {
        Ok(Config{
            socket_addr: env::var("SOCKET_ADDR")?,
            channel: env::var("CHANNEL")?,
        })
    }
}

pub enum ConfigError {
    VarError(std::env::VarError)
}

impl From<std::env::VarError> for ConfigError {
    fn from(e: std::env::VarError) -> Self {
        ConfigError::VarError(e)
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::VarError(e) =>
                write!(f, "{}", e),
        }
    }
}
