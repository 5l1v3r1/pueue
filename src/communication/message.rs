use ::serde_derive::{Deserialize, Serialize};

use ::anyhow::Error;

/// The Message used to add a new command to the daemon.
#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    Add(AddMessage),
    Remove(RemoveMessage),
    Switch(SwitchMessage),

    Start(StartMessage),
    Pause(PauseMessage),
    Kill(KillMessage),

    Reset,
    Clear,

    Status,
    Success(SuccessMessage),
    Failure(FailureMessage),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddMessage {
    pub command: Vec<String>,
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoveMessage {
    pub indices: Vec<usize>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SwitchMessage {
    pub command: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StartMessage {
    pub command: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PauseMessage {
    pub command: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct KillMessage {
    pub command: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SuccessMessage {
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FailureMessage {
    pub text: String,
}

pub fn create_success_message(text: String) -> Result<Message, Error> {
    Ok(Message::Success(SuccessMessage { text: text }))
}

pub fn create_failure_message(text: String) -> Message {
    Message::Failure(FailureMessage { text: text })
}