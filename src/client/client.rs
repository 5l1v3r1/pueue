use ::anyhow::Result;
use ::byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use ::std::io::Cursor;
use ::tokio::net::TcpStream;
use ::tokio::prelude::*;

use crate::client::cli::handle_cli;
use crate::communication::local::get_socket_path;
use crate::communication::message::*;
use crate::settings::Settings;

/// The client
pub struct Client {
    settings: Settings,
    message: Message,
}

impl Client {
    pub fn new(settings: Settings) -> Result<Self> {
        let message = handle_cli()?;

        Ok(Client {
            settings: settings,
            message: message,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        // Connect to stream
        let mut stream = TcpStream::connect(get_socket_path(&self.settings)).await?;

        // Create the message payload and send it to the daemon.
        self.send_message(&mut stream).await?;

        // Check if we can receive the response from the daemon
        let response = self.receive_answer(&mut stream).await?;

        println!("{}", &response);
        Ok(())
    }

    /// Send a message to the daemon.
    /// The JSON payload is highly dependent on the commandline input parameters
    /// Some payloads are serialized `Add` or `Remove` messages.
    /// Before we send the actual payload, a header is sent with two u64.
    /// One signals the type of the message, whilst the other signals the length of the payload.
    async fn send_message(&mut self, stream: &mut TcpStream) -> Result<()> {
        // Prepare command for transfer and determine message byte size
        let payload = serde_json::to_string(&self.message)
            .expect("Failed to serialize message.")
            .into_bytes();
        let byte_size = payload.len() as u64;

        let mut header = vec![];
        header.write_u64::<BigEndian>(byte_size).unwrap();

        // Send the request size header first.
        // Afterwards send the request.
        stream.write_all(&header).await?;
        stream.write_all(&payload).await?;

        Ok(())
    }

    /// Receive the response of the daemon and handle it.
    async fn receive_answer(&mut self, stream: &mut TcpStream) -> Result<String> {
        // Extract the instruction size from the header bytes
        let mut header_buffer = vec![0; 8];
        stream.read_exact(&mut header_buffer).await?;
        let mut header = Cursor::new(header_buffer);
        let instruction_size = header.read_u64::<BigEndian>().unwrap() as usize;

        // Receive the instruction
        let mut buffer = vec![0; instruction_size];
        stream.read(&mut buffer).await?;

        Ok(String::from_utf8(buffer)?)
    }
}