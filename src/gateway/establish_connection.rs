use std::{collections::HashMap, sync::Arc};

use chorus::types::{
    GatewayHeartbeat, GatewayHeartbeatAck, GatewayHello, GatewayIdentifyPayload, GatewayReady,
    GatewayResume, Snowflake,
};
use futures::{SinkExt, StreamExt};
use log::{debug, trace};
use rand::seq;
use serde_json::{from_str, json};
use sqlx::PgPool;
use tokio::{
    net::TcpStream,
    sync::{broadcast::Sender, Mutex},
    task::JoinHandle,
};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message,
    },
};

use crate::{
    database::entities::Config,
    errors::{Error, GatewayError},
    gateway::{
        gateway_task, heartbeat::HeartbeatHandler, ready::create_ready, Event, GatewayPayload,
        GatewayUser,
    },
    util::token::check_token,
};

use super::{
    ConnectedUsers, GatewayClient, NewWebSocketConnection, ResumableClientsStore,
    WebSocketConnection,
};

/// Internal use only state struct to pass around data to the `finish_connecting` function.
struct State {
    connection: WebSocketConnection,
    db: PgPool,
    config: Config,
    connected_users: ConnectedUsers,
    sequence_number: Arc<Mutex<u64>>,
    /// Receiver for heartbeat messages. The `HeartbeatHandler` will receive messages from this channel.
    heartbeat_receive: tokio::sync::broadcast::Receiver<GatewayHeartbeat>,
    /// Sender for heartbeat messages. The main gateway task will send messages to this channel for the `HeartbeatHandler` to receive and handle.
    heartbeat_send: tokio::sync::broadcast::Sender<GatewayHeartbeat>,
    session_id_send: tokio::sync::broadcast::Sender<String>,
    session_id_receive: tokio::sync::broadcast::Receiver<String>,
}

/// `establish_connection` is the entrypoint method that gets called when a client tries to connect
/// to the WebSocket server.
///
/// If successful, returns a [NewConnection] with a new [Arc<Mutex<GatewayUser>>] and a
/// [GatewayClient], whose `.parent` field contains a [Weak] reference to the new [GatewayUser].
pub(super) async fn establish_connection(
    stream: TcpStream,
    db: PgPool,
    config: Config,
    connected_users: ConnectedUsers,
) -> Result<NewWebSocketConnection, Error> {
    trace!(target: "symfonia::gateway::establish_connection::establish_connection", "Beginning process to establish connection (handshake)");
    // Accept the connection and split it into its sender and receiver halves.
    let ws_stream = accept_async(stream).await?.split();
    let mut connection = WebSocketConnection::new(ws_stream.0, ws_stream.1);
    trace!(target: "symfonia::gateway::establish_connection::establish_connection", "Sending hello message");
    // Hello message
    match connection
        .sender
        .send(Message::Text(json!(GatewayHello::default()).to_string()))
    {
        Ok(_) => (),
        Err(e) => {
            log::debug!(target: "symfonia::gateway::establish_connection", "Error when sending hello message. Aborting connection: {e}");
            return Err(GatewayError::Internal.into());
        }
    };
    trace!(target: "symfonia::gateway::establish_connection::establish_connection", "Sent hello message");

    let mut received_identify_or_resume = false;

    let (kill_send, mut kill_receive) = tokio::sync::broadcast::channel::<()>(1);
    // Inter-task communication channels. The main gateway task will send received heartbeat related messages
    // to the `HeartbeatHandler` task via the `message_send` channel, which the `HeartbeatHandler` task will
    // then receive and handle.
    let (message_send, message_receive) = tokio::sync::broadcast::channel::<GatewayHeartbeat>(4);

    let sequence_number = Arc::new(Mutex::new(0u64)); // TODO: Actually use this, as in: Increment it when needed. Currently, this is not being done.

    // Used to inform the `HeartbeatHandler` task of the session_id of the client, if we receive it after a heartbeat handler task has been spawned.
    let (session_id_send, session_id_receive) = tokio::sync::broadcast::channel::<String>(1);

    let state = State {
        connection: connection.clone(),
        db: db.clone(),
        config: config.clone(),
        connected_users: connected_users.clone(),
        sequence_number: sequence_number.clone(),
        heartbeat_receive: message_receive.resubscribe(),
        heartbeat_send: message_send.clone(),
        session_id_send: session_id_send.clone(),
        session_id_receive: session_id_receive.resubscribe(),
    };

    // This JoinHandle `.is_some()` if we receive a heartbeat message *before* we receive an
    // identify or resume message.
    let mut heartbeat_handler_handle: Option<JoinHandle<()>> = None;

    trace!(target: "symfonia::gateway::establish_connection::establish_connection", "Waiting for next message, timeout or kill signal...");
    let mut second_kill_receive = kill_receive.resubscribe();
    // Either we time out, the connection is killed, or we receive succesful output from `finish_connecting`.
    tokio::select! {
        _ = second_kill_receive.recv() => {
            debug!(target: "symfonia::gateway::establish_connection::establish_connection", "Connection was closed before we could establish it");
            Err(GatewayError::Closed.into())
        }
        // If we do not receive an identifying or resuming message within 30 seconds, we close the connection.
        _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
            debug!(target: "symfonia::gateway::establish_connection::establish_connection", "Connection timed out: No message received within 30 seconds");
            Err(GatewayError::Timeout.into())
        }
        // Since async closures are not yet stable, we have to use a dedicated function to handle the
        // connection establishment process. :(
        new_connection = finish_connecting(heartbeat_handler_handle, state)
         => {
            log::trace!(target: "symfonia::gateway::establish_connection", "Connection established.");
            new_connection
        }
    }
}

/// `finish_connecting` is the second part of the connection establishment process. It picks up after
/// the initial `Hello` message has been sent to the client. It then waits on the next message from
/// the client, which should be either a `Heartbeat`, `Identify` or `Resume` message, handling each
/// case accordingly.
#[allow(clippy::too_many_arguments)]
async fn finish_connecting(
    mut heartbeat_handler_handle: Option<JoinHandle<()>>,
    mut state: State,
) -> Result<NewWebSocketConnection, Error> {
    loop {
        trace!(target: "symfonia::gateway::establish_connection::finish_connecting", "Waiting for next message...");
        let raw_message = match state.connection.receiver.recv().await {
            Ok(next) => next,
            Err(_) => {
                log::debug!(target: "symfonia::gateway::finish_connecting", "Encountered error when trying to receive message. Sending kill signal...");
                state
                    .connection
                    .kill_send
                    .send(())
                    .expect("Failed to send kill_send");
                return Err(GatewayError::Timeout.into());
            }
        };
        debug!(target: "symfonia::gateway::establish_connection::finish_connecting", "Received message");
        trace!("Message: {}", raw_message);
        let event = match Event::try_from(raw_message.clone()) {
            Ok(event) => event,
            Err(e) => {
                log::debug!("Message could not be deserialized to Event: {e}");
                return Err(Error::Gateway(GatewayError::UnexpectedMessage(
                    e.to_string(),
                )));
            }
        };
        if let Event::Heartbeat(heartbeat) = event {
            log::trace!(target: "symfonia::gateway::establish_connection::finish_connecting", "Received heartbeat");
            match heartbeat_handler_handle {
                None => {
                    // This only happens *once*. You will find that we have to `.resubscribe()` to
                    // the channels to make the borrow checker happy, because the channels are otherwise
                    // moved into the spawned task, which, *technically* could occur multiple times,
                    // due to the loop {} construct. However, this is not the case, because this code
                    // executes only if heartbeat_handler_handle is None, which is only true once,
                    // as we set it to Some(_) in this block. We could perhaps make this a little
                    // nicer by using unsafe rust magic, which would also allow us to use more appropriate
                    // channel types such as `oneshot` for the session_id_receive channel. However,
                    // I don't see that this is needed at the moment.
                    heartbeat_handler_handle = Some(tokio::spawn({
                        let mut heartbeat_handler = HeartbeatHandler::new(
                            state.connection.clone(),
                            state.heartbeat_receive.resubscribe(),
                            state.sequence_number.clone(),
                            state.session_id_receive.resubscribe(),
                        );
                        async move {
                            heartbeat_handler.run().await;
                        }
                    }))
                }
                Some(_) => {
                    state.heartbeat_send.send(heartbeat);
                }
            }
        } else if let Event::Identify(identify) = event {
            log::trace!(target: "symfonia::gateway::establish_connection::finish_connecting", "Received identify payload");
            let claims = match check_token(
                &state.db,
                &identify.event_data.as_ref().unwrap().token,
                &state.config.security.jwt_secret,
            )
            .await
            {
                Ok(claims) => {
                    trace!(target: "symfonia::gateway::establish_connection::finish_connecting", "Token verified");
                    claims
                }
                Err(_) => {
                    log::trace!(target: "symfonia::gateway::establish_connection::finish_connecting", "Failed to verify token");
                    state
                        .connection
                        .kill_send
                        .send(())
                        .expect("Failed to send kill signal");
                    return Err(crate::errors::UserError::InvalidToken.into());
                }
            };
            log::trace!(target: "symfonia::gateway::establish_connection::finish_connecting", "Getting gateway_user");
            let mut gateway_user = state.connected_users.get_user_or_new(claims.id);
            log::trace!(target: "symfonia::gateway::establish_connection::finish_connecting", "Creating main gateway task handle");
            let main_task_handle = tokio::spawn(gateway_task::gateway_task(
                state.connection.clone(),
                gateway_user.lock().await.inbox.resubscribe(),
                state.heartbeat_send.clone(),
                state.sequence_number.clone(),
                state.connected_users.clone(),
                claims.id,
            ));
            log::trace!(target: "symfonia::gateway::establish_connection::finish_connecting", "Creating gateway_client");
            let gateway_client = state
                .connected_users
                .new_client(
                    gateway_user.clone(),
                    state.connection.clone(),
                    main_task_handle,
                    match heartbeat_handler_handle {
                        Some(handle) => handle,
                        None => tokio::spawn({
                            log::trace!(target: "symfonia::gateway::establish_connection::finish_connecting", "No heartbeat_handler yet. Creating one...");
                            let mut heartbeat_handler = HeartbeatHandler::new(
                                state.connection.clone(),
                                state.heartbeat_receive.resubscribe(),
                                state.sequence_number.clone(),
                                state.session_id_receive.resubscribe(),
                            );
                            async move {
                                heartbeat_handler.run().await;
                            }
                        }),
                    },
                    &identify.event_data.as_ref().unwrap().token,
                    state.sequence_number.clone(),
                )
                .await;
            match state
                .session_id_send
                .send(identify.event_data.unwrap().token)
            {
                Ok(_) => (),
                Err(_) => {
                    log::error!(target: "symfonia::gateway::establish_connection::finish_connecting", "Failed to send session_id to heartbeat handler");
                    state
                        .connection
                        .kill_send
                        .send(())
                        .expect("Failed to send kill signal");
                    return Err(GatewayError::Internal.into());
                }
            }
            let formatted_payload = GatewayPayload::<GatewayReady> {
                op_code: 0,
                event_data: Some(create_ready(claims.id, &state.db).await?),
                sequence_number: None,
                event_name: Some("READY".to_string()),
            };
            state
                .connection
                .sender
                .send(Message::Text(json!(formatted_payload).to_string()))?;
            log::trace!(target: "symfonia::gateway::establish_connection::finish_connecting", "Done!");
            return Ok(NewWebSocketConnection {
                user: gateway_user,
                client: gateway_client.clone(),
            });
        } else if let Event::Resume(resume) = event {
            log::trace!(target: "symfonia::gateway::establish_connection::finish_connecting", "Received resume payload");
            log::warn!(target: "symfonia::gateway::establish_connection::finish_connecting", "Resuming connections is not yet implemented. Telling client to identify instead.");
            state
                .connection
                .sender
                .send(Message::Close(Some(CloseFrame {
                    code: CloseCode::from(4007u16),
                    reason: "Resuming connections is not yet implemented. Please identify instead."
                        .into(),
                })))?;
            state
                .connection
                .kill_send
                .send(())
                .expect("Failed to send kill signal");
        } else {
            debug!(target: "symfonia::gateway::establish_connection::finish_connecting", "Message could not be decoded as resume, heartbeat or identify: {}", raw_message);
            return Err(GatewayError::UnexpectedMessage("Received payload other than Heartbeat, Identify or Resume before the connection was established".to_string()).into());
        }
    }
}
