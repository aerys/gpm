use std::io;
use std::fmt;

use git2;

#[derive(Debug)]
pub enum CommandError {
    IO(io::Error),
    Git(git2::Error),
    String(String),
}

impl From<io::Error> for CommandError {
    fn from(err : io::Error) -> CommandError {
        CommandError::IO(err)
    }
}

impl From<git2::Error> for CommandError {
    fn from(err : git2::Error) -> CommandError {
        CommandError::Git(err)
    }
}

impl From<String> for CommandError {
    fn from(err : String) -> CommandError {
        CommandError::String(err)
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CommandError::IO(e) => write!(f, "{}", e),
            CommandError::Git(s) => write!(f, "{}", s),
            CommandError::String(s) => write!(f, "{}", s),
        }
    }
}
