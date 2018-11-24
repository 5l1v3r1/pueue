use byteorder::{BigEndian, ReadBytesExt};
use failure::Error;
use futures::prelude::*;
use futures::Future;
use std::collections::HashMap;
use std::io::Cursor;
use tokio::io as tokio_io;
use tokio_uds::{UnixListener, UnixStream};

use crate::communication::local::*;
use crate::communication::message::*;
use crate::daemon::queue::*;
use crate::daemon::task_handler::TaskHandler;
use crate::settings::Settings;

/// The daemon is center of all logic in pueue.
/// This is the single source of truth for all clients and workers.
pub struct Daemon {
    unix_listener: UnixListener,
    unix_incoming: Vec<Box<Future<Item = (MessageType, String, UnixStream), Error = Error> + Send>>,
    unix_response: Vec<Box<Future<Item = (UnixStream, Vec<u8>), Error = Error> + Send>>,
    queue: Queue,
    task_handler: TaskHandler,
}

impl Daemon {
    /// Create a new daemon.
    /// This function also handle the creation of other components,
    /// such as the queue, sockets and the process handler.
    pub fn new(settings: &Settings) -> Self {
        let task_handler = TaskHandler::new();

        Daemon {
            unix_listener: get_unix_listener(&settings),
            unix_incoming: Vec::new(),
            unix_response: Vec::new(),
            queue: Vec::new(),
            task_handler: task_handler,
        }
    }

    /// Poll the unix listener and accept new incoming connections
    /// Create a new future for receiving the instruction and add it to unix_incoming
    fn accept_incoming(&mut self) {
        loop {
            // Poll if we have a new incoming connection.
            // In case we don't, break the loop
            let accept_result = self.unix_listener.poll_accept();
            let accept_future = if let Ok(future) = accept_result {
                future
            } else {
                println!("Failed to accept incoming unix connection.");
                println!("{:?}", accept_result.err());
                continue;
            };

            // Check if we can accept an incoming connection or if we need to wait a little.
            match accept_future {
                Async::Ready((stream, _socket_addr)) => {
                    // First read the header to determine the size of the instruction
                    let incoming = tokio_io::read_exact(stream, vec![0; 16])
                        .then(|result| {
                            let (stream, header) = result?;

                            // Extract the instruction size from the header bytes
                            let mut header = Cursor::new(header);
                            let instruction_size = header.read_u64::<BigEndian>().unwrap() as usize;
                            let instruction_index =
                                header.read_u64::<BigEndian>().unwrap() as usize;

                            // Try to resolve the instruction index
                            // If we got an invalid instruction index, c
                            let instruction_type = get_message_type(instruction_index)?;

                            Ok(ReceiveInstruction {
                                instruction_type: instruction_type,
                                read_instruction_future: Box::new(
                                    tokio_io::read_exact(stream, vec![0; instruction_size])
                                        .map_err(|error| Error::from(error)),
                                ),
                            })
                        })
                        .and_then(|future| future);
                    self.unix_incoming.push(Box::new(incoming));
                }
                Async::NotReady => break,
            }
        }
    }

    /// Continuously poll the existing incoming futures.
    /// In case we received an instruction, handle it and create a response future.
    /// The response future is added to unix_response and handled in a separate function.
    fn handle_incoming(&mut self) -> HashMap<MessageType, String> {
        let mut instructions: HashMap<MessageType, String> = HashMap::new();
        let len = self.unix_incoming.len();

        for i in (0..len).rev() {
            let result = self.unix_incoming[i].poll();

            // Handle socket error
            if result.is_err() {
                println!("Socket errored during read");
                println!("{:?}", result.err());
                self.unix_incoming.remove(i);

                continue;
            }

            // We received an instruction from a client. Handle it
            match result.unwrap() {
                Async::Ready((instruction_type, instruction, stream)) => {
                    println!("{:?}", instruction_type);
                    println!("{}", instruction);

                    instructions.insert(instruction_type, instruction);

                    // Create a future for sending the response.
                    let response = String::from("Command added");
                    let response_future = tokio_io::write_all(stream, response.into_bytes());
                    self.unix_response.push(Box::new(
                        response_future.map_err(|error| Error::from(error)),
                    ));
                    self.unix_incoming.remove(i);
                }
                Async::NotReady => {}
            }
        }

        instructions
    }

    /// Send the response to the client.
    fn handle_responses(&mut self) {
        let len = self.unix_response.len();
        for i in (0..len).rev() {
            let result = self.unix_response[i].poll();

            // Handle socket error
            if result.is_err() {
                println!("Socket errored during send");
                println!("{:?}", result.err());
                self.unix_response.remove(i);

                continue;
            }

            // Check whether the response has been sent and remove the future and thereby the socket on success
            match result.unwrap() {
                Async::Ready((_, _)) => {
                    self.unix_response.remove(i);
                }
                Async::NotReady => {}
            }
        }
    }
}

impl Daemon {
    pub fn handle_instructions(&mut self, instructions: HashMap<MessageType, String>) {
        for (instruction_type, instruction) in instructions {
            let message = extract_message(instruction_type.clone(), instruction);

            match instruction_type {
                MessageType::Add => {
                    let add_message = if let Some(add_message) = message.add {
                        add_message
                    } else {
                        panic!("Error in add message unwrap.");
                    };
                    add_task(&mut self.queue, add_message);
                }
                MessageType::Invalid => panic!("Invalid message type"),
            };
        }
    }
}

impl Future for Daemon {
    type Item = ();
    type Error = Error;

    /// The poll function of the daemon.
    /// This is continuously called by the Tokio core.
    fn poll(&mut self) -> Result<Async<()>, Self::Error> {
        // Accept all new connections
        self.accept_incoming();

        // Poll all connection futures and handle the received instruction.
        let instructions = self.handle_incoming();

        self.handle_instructions(instructions);

        self.handle_responses();

        self.task_handler.check(&mut self.queue);

        // `NotReady` is returned here because the future never actually
        // completes. The server runs until it is dropped.
        Ok(Async::NotReady)
    }
}
