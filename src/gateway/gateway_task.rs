use std::{sync::Arc, time::Duration};

use chorus::types::{GatewayHeartbeat, GatewaySendPayload, Opcode, Snowflake};
use futures::StreamExt;
use log::debug;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{from_str, json};
use tokio::{sync::Mutex, time::sleep};
use tokio_tungstenite::tungstenite::{
    protocol::{frame::coding::CloseCode, CloseFrame},
    Message,
};

use crate::{
    errors::{Error, GatewayError},
    gateway::{DispatchEvent, DispatchEventType},
};

use super::{ConnectedUsers, Event, GatewayClient, GatewayPayload};

/// Handles all messages a client sends to the gateway post-handshake.
pub(super) async fn gateway_task(
    mut connection: super::WebSocketConnection,
    mut inbox: tokio::sync::broadcast::Receiver<Event>,
    mut heartbeat_send: tokio::sync::broadcast::Sender<GatewayHeartbeat>,
    last_sequence_number: Arc<Mutex<u64>>,
    connected_users: ConnectedUsers,
    user_id: Snowflake,
) {
    log::trace!(target: "symfonia::gateway::gateway_task", "Started a new gateway task!");
    let inbox_processor = tokio::spawn(process_inbox(connection.clone(), inbox.resubscribe()));

    /*
    Before we can respond to any gateway event we receive, we need to figure out what kind of event
    we are dealing with. For a lot of events, this is easy, because we can just look at the opcode
    and figure out the event type. For the dispatch events however, we also need to look at the event
    name to find out the exact dispatch event we are dealing with. -bitfl0wer
     */

    loop {
        tokio::select! {
            _ = connection.kill_receive.recv() => {
                let mut store_lock = connected_users.store.write();
                store_lock.users.remove(&user_id);
                store_lock.inboxes.remove(&user_id);
                // TODO(bitfl0wer) Add the user to the disconnected sessions
                drop(store_lock);
                return;
            },
            message_result = connection.receiver.recv() => {
                match message_result {
                    Ok(message_of_unknown_type) => {
                        log::trace!(target: "symfonia::gateway::gateway_task", "Received raw message {:?}", message_of_unknown_type);
                        let event = unwrap_event(Event::try_from(message_of_unknown_type), connection.clone(), connection.kill_send.clone());
                        log::trace!(target: "symfonia::gateway::gateway_task", "Event type of received message: {:?}", event);
                        match event {
                            Event::Dispatch(_) => {
                                // Receiving a dispatch event from a client is never correct
                                log::debug!(target: "symfonia::gateway::gateway_task", "Received an unexpected message: {:?}", event);
                                connection.sender.send(Message::Close(Some(CloseFrame { code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Library(4002), reason: "DECODE_ERROR".into() })));
                                connection.kill_send.send(()).expect("Failed to send kill_send");
                            },
                            Event::Heartbeat(hearbeat_event) => {
                                match heartbeat_send.send(hearbeat_event) {
                                    Err(e) => {
                                        log::debug!(target: "symfonia::gateway::gateway_task", "Received Heartbeat but HeartbeatHandler seems to be dead?");
                                        connection.sender.send(Message::Close(Some(CloseFrame { code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Library(4002), reason: "DECODE_ERROR".into() })));
                                        connection.kill_send.send(()).expect("Failed to send kill_send");
                                    },
                                    Ok(_) => {
                                        log::trace!(target: "symfonia::gateway::gateway_task", "Forwarded heartbeat message to HeartbeatHandler!");
                                    }
                                }
                            }
                            _ => {
                                log::error!(target: "symfonia::gateway::gateway_task", "Received an event type for which no code is yet implemented in the gateway_task. Please open a issue or PR at the symfonia repository. {:?}", event);
                            }
                        }

                    },
                    Err(error) => {
                        connection.sender.send(Message::Close(Some(CloseFrame { code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Library(4000), reason: "INTERNAL_SERVER_ERROR".into() })));
                        connection.kill_send.send(()).expect("Failed to send kill_send");
                    },
                }
            }
        }
    }

    todo!()
}

fn handle_event(
    event: Event,
    connection: super::WebSocketConnection,
    mut kill_send: tokio::sync::broadcast::Sender<()>,
) {
    todo!()
}

/// Unwraps an event from a Result<Event, Error> and handles the error if there is one. Errors will
/// shut down all tasks belonging to this session and will kill the gateway task through a panic.
fn unwrap_event(
    result: Result<Event, Error>,
    connection: super::WebSocketConnection,
    mut kill_send: tokio::sync::broadcast::Sender<()>,
) -> Event {
    match result {
        Err(e) => {
            match e {
                Error::Gateway(g) => match g {
                    GatewayError::UnexpectedOpcode(o) => {
                        log::debug!(target: "symfonia::gateway::gateway_task::unwrap_event", "Received an unexpected opcode: {:?}", o);
                        connection.sender.send(Message::Close(Some(CloseFrame { code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Library(4001), reason: "UNKNOWN_OPCODE".into() })));
                        kill_send.send(()).expect("Failed to send kill_send");
                        panic!("Killing gateway task: Received an unexpected opcode");
                    }
                    GatewayError::UnexpectedMessage(m) => {
                        log::debug!(target: "symfonia::gateway::gateway_task::unwrap_event", "Received an unexpected message: {:?}", m);
                        connection.sender.send(Message::Close(Some(CloseFrame { code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Library(4002), reason: "DECODE_ERROR".into() })));
                        kill_send.send(()).expect("Failed to send kill_send");
                        panic!("Killing gateway task: Received an unexpected message");
                    }
                    _ => {
                        log::debug!(target: "symfonia::gateway::gateway_task::unwrap_event", "Received an unexpected error: {:?}", g);
                        connection.sender.send(Message::Close(Some(CloseFrame { code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Library(4000), reason: "INTERNAL_SERVER_ERROR".into() })));
                        kill_send.send(()).expect("Failed to send kill_send");
                        panic!("Killing gateway task: Received an unexpected error");
                    }
                },
                _ => {
                    log::debug!(target: "symfonia::gateway::gateway_task::unwrap_event", "Received an unexpected error: {:?}", e);
                    connection.sender.send(Message::Close(Some(CloseFrame { code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Library(4000), reason: "INTERNAL_SERVER_ERROR".into() })));
                    kill_send.send(()).expect("Failed to send kill_send");
                    panic!("Killing gateway task: Received an unexpected error");
                }
            }
        }
        Ok(event) => event,
    }
}

/// Process events triggered by the HTTP API.
async fn process_inbox(
    mut connection: super::WebSocketConnection,
    mut inbox: tokio::sync::broadcast::Receiver<Event>,
) {
    loop {
        tokio::select! {
            _ = connection.kill_receive.recv() => {
                return;
            }
            event = inbox.recv() => {
                match event {
                    Ok(event) => {
                        let send_result = connection.sender.send(Message::Text(json!(event).to_string()));
                        match send_result {
                            Ok(_) => (), // TODO: Increase sequence number here
                            Err(_) => {
                                debug!("Failed to send event to WebSocket. Sending kill_send");
                                connection.kill_send.send(()).expect("Failed to send kill_send");
                            },
                        }
                    }
                    Err(_) => {
                        return;
                    }
                }
            }
        }
    }
}
