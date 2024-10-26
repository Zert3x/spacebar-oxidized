// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

pub mod dispatchevent;
pub mod event;

pub use dispatchevent::*;
pub use event::*;

use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    ops::{Deref, DerefMut},
    sync::{Arc, Weak},
};

use ::serde::{de::DeserializeOwned, Deserialize, Serialize};
use chorus::types::{
    ChannelCreate, ChannelDelete, ChannelUpdate, GatewayHeartbeat, GatewayHeartbeatAck,
    GatewayHello, GatewayIdentifyPayload, GatewayInvalidSession, GatewayReady,
    GatewayReadySupplemental, GatewayRequestGuildMembers, GatewayResume, GuildBanAdd,
    GuildBanRemove, GuildCreate, GuildDelete, GuildEmojisUpdate, GuildIntegrationsUpdate,
    GuildMemberAdd, GuildMemberRemove, GuildMemberUpdate, GuildMembersChunk, GuildUpdate,
    InteractionCreate, InviteCreate, InviteDelete, MessageCreate, MessageDelete, MessageDeleteBulk,
    MessageReactionAdd, MessageReactionRemove, MessageReactionRemoveAll,
    MessageReactionRemoveEmoji, MessageUpdate, Opcode, PresenceUpdate, Snowflake,
    StageInstanceCreate, StageInstanceDelete, StageInstanceUpdate, ThreadCreate, ThreadDelete,
    ThreadListSync, ThreadMemberUpdate, ThreadMembersUpdate, ThreadUpdate, TypingStartEvent,
    UserUpdate, VoiceServerUpdate, VoiceStateUpdate, WebhooksUpdate,
};
use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use log::log;
use parking_lot::RwLock;
use pubserve::Subscriber;
use serde_json::from_str;
use sqlx::PgPool;
use sqlx_pg_uint::PgU64;
use tokio::{net::TcpStream, sync::Mutex};
use tokio_tungstenite::{
    tungstenite::{
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message,
    },
    WebSocketStream,
};

use crate::{
    errors::{Error, GatewayError},
    WebSocketReceive, WebSocketSend,
};

use super::ResumableClientsStore;

#[derive(Serialize, Clone, PartialEq, Debug)]
/// A de-/serializable data payload for transmission over the gateway.
pub struct GatewayPayload<T>
where
    T: Serialize + DeserializeOwned,
{
    #[serde(rename = "op")]
    pub op_code: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "d")]
    pub event_data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "s")]
    pub sequence_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "t")]
    pub event_name: Option<String>,
}

impl<T: Serialize + DeserializeOwned> GatewayPayload<T> {
    pub fn has_data(&self) -> bool {
        self.event_data.is_some()
    }

    pub fn has_sequence(&self) -> bool {
        self.sequence_number.is_some()
    }

    pub fn has_event_name(&self) -> bool {
        self.event_name.is_some()
    }
}

impl<'de, T: DeserializeOwned + Serialize> Deserialize<'de> for GatewayPayload<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let op_code = value["op"].as_u64().unwrap() as u8;
        let event_data = match value.get("d").cloned() {
            Some(data) => match serde_json::from_value(data) {
                Ok(t) => t,
                Err(e) => return Err(::serde::de::Error::custom(e)),
            },
            None => None,
        };
        let sequence_number = value.get("s").cloned().map(|v| v.as_u64().unwrap());
        let event_name = match value.get("t") {
            Some(v) => v.as_str().map(|v_str| v_str.to_string()),
            None => None,
        };
        Ok(GatewayPayload {
            op_code,
            event_data,
            sequence_number,
            event_name,
        })
    }
}

#[derive(Default, Clone)]
pub struct ConnectedUsers {
    pub store: Arc<RwLock<ConnectedUsersInner>>,
    pub role_user_map: Arc<Mutex<RoleUserMap>>,
}

/// A mapping of Snowflake IDs to the "inbox" of a [GatewayUser].
///
/// An "inbox" is a [tokio::sync::mpsc::Sender] that can be used to send [Event]s to all connected
/// clients of a [GatewayUser].
#[derive(Default)]
pub struct ConnectedUsersInner {
    pub inboxes: HashMap<Snowflake, tokio::sync::broadcast::Sender<Event>>,
    pub users: HashMap<Snowflake, Arc<Mutex<GatewayUser>>>,
    pub resumeable_clients_store: ResumableClientsStore,
}

/// A single identifiable User connected to the Gateway - possibly using many clients at the same
/// time.
pub struct GatewayUser {
    /// The "inbox" of a [GatewayUser]. This is a [tokio::sync::mpsc::Receiver]. Events sent to
    /// this inbox will be sent to all connected clients of this user.
    pub inbox: tokio::sync::broadcast::Receiver<Event>,
    /// The "outbox" of a [GatewayUser]. This is a [tokio::sync::mpsc::Sender]. From this outbox,
    /// more inboxes can be created.
    outbox: tokio::sync::broadcast::Sender<Event>,
    /// Sessions a User is connected with. HashMap of SessionToken -> GatewayClient
    clients: HashMap<String, Arc<Mutex<GatewayClient>>>,
    /// The Snowflake ID of the User.
    pub id: Snowflake,
    /// A collection of [Subscribers](Subscriber) to [Event] [Publishers](pubserve::Publisher).
    ///
    /// A GatewayUser may have many [GatewayClients](GatewayClient), but he only gets subscribed to
    /// all relevant [Publishers](pubserve::Publisher) *once* to save resources.
    subscriptions: Vec<Box<dyn Subscriber<Event>>>,
    /// [Weak] reference to the [ConnectedUsers] store.
    connected_users: ConnectedUsers,
}

/// A concrete session, that a [GatewayUser] is connected to the Gateway with.
pub struct GatewayClient {
    connection: WebSocketConnection,
    /// A [Weak] reference to the [GatewayUser] this client belongs to.
    pub parent: Weak<Mutex<GatewayUser>>,
    // Handle to the main Gateway task for this client
    main_task_handle: tokio::task::JoinHandle<()>,
    // Handle to the heartbeat task for this client
    heartbeat_task_handle: tokio::task::JoinHandle<()>,
    /// Token of the session token used for this connection
    pub session_token: String,
    /// The last sequence number received from the client. Shared between the main task, heartbeat
    /// task, and this struct.
    last_sequence: Arc<Mutex<u64>>,
}

impl ConnectedUsers {
    /// Create a new, empty [ConnectedUsers] instance.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bulk_message_builder(&self) -> BulkMessageBuilder {
        BulkMessageBuilder::default()
    }

    /// Initialize the [RoleUserMap] with data from the database.
    ///
    /// This method will query the database for all roles and all users that have these roles.
    /// The data will then populate the map.
    ///
    /// Due to the possibly large number of roles and users returned by the database, this method
    /// should only be executed once. The [RoleUserMap] should be kept synchronized with the database
    /// through means that do not involve this method.
    ///
    /// ## Locking
    ///
    /// This method acquires a lock on `role_user_map` for the duration of its runtime.
    pub async fn init_role_user_map(&self, db: &PgPool) -> Result<(), crate::errors::Error> {
        self.role_user_map.lock().await.init(db).await
    }

    /// Get a [GatewayUser] by its Snowflake ID if it already exists in the store, or create a new
    /// [GatewayUser] if it does not exist using [ConnectedUsers::new_user].
    ///
    /// ## Locking
    ///
    /// This method always acquires a read lock on `store`. If the user does not yet exist in the
    /// store, a `write` lock will be acquired additionally.
    pub fn get_user_or_new(&self, id: Snowflake) -> Arc<Mutex<GatewayUser>> {
        let inner = self.store.clone();
        log::trace!(target: "symfonia::gateway::types::ConnectedUsers::get_user_or_new", "Acquiring lock on ConnectedUsersInner...");
        let mut lock = inner.read();
        log::trace!(target: "symfonia::gateway::types::ConnectedUsers::get_user_or_new", "Lock acquired!");
        if let Some(user) = lock.users.get(&id) {
            log::trace!(target: "symfonia::gateway::types::ConnectedUsers::get_user_or_new", "Found user {id} in store");
            user.clone()
        } else {
            drop(lock);
            log::trace!(target: "symfonia::gateway::types::ConnectedUsers::get_user_or_new", "Creating new user {id} in store");
            self.new_user(HashMap::new(), id, Vec::new())
        }
    }

    pub fn inner(&self) -> Arc<RwLock<ConnectedUsersInner>> {
        self.store.clone()
    }

    /// Register a new [GatewayUser] with the [ConnectedUsers] instance.
    ///
    /// ## Locking
    ///
    /// This method acquires a write lock on `store` for the duration of its runtime.
    fn register(&self, user: GatewayUser) -> Arc<Mutex<GatewayUser>> {
        log::trace!(target: "symfonia::gateway::types::ConnectedUsers::register", "Acquiring lock on ConnectedUsersInner...");
        self.store
            .write()
            .inboxes
            .insert(user.id, user.outbox.clone());
        log::trace!(target: "symfonia::gateway::types::ConnectedUsers::register", "Lock acquired!");
        let id = user.id;
        let arc = Arc::new(Mutex::new(user));
        self.store.write().users.insert(id, arc.clone());
        log::trace!(target: "symfonia::gateway::types::ConnectedUsers::register", "Inserted user {id} into users store");
        arc
    }

    /// Deregister a [GatewayUser] from the [ConnectedUsers] instance.
    ///
    /// ## Locking
    ///
    /// This method acquires a write lock on `store` for the duration of its runtime.
    pub fn deregister(&self, user: &GatewayUser) {
        self.store.write().inboxes.remove(&user.id);
        self.store.write().users.remove(&user.id);
    }

    /// Get the "inbox" of a [GatewayUser] by its Snowflake ID.
    ///
    /// ## Locking
    ///
    /// This method acquires a read lock on `store` for the duration of its runtime.
    pub async fn inbox(&self, id: Snowflake) -> Option<tokio::sync::broadcast::Sender<Event>> {
        self.store.read().inboxes.get(&id).cloned()
    }

    /// Create a new [GatewayUser] with the given Snowflake ID, [GatewayClient]s, and subscriptions.
    /// Registers the new [GatewayUser] with the [ConnectedUsers] instance.
    ///
    /// ## Locking
    ///
    /// This method calls [Self::register]. Refer to that method for information on locking behavior.
    pub fn new_user(
        &self,
        clients: HashMap<String, Arc<Mutex<GatewayClient>>>,
        id: Snowflake,
        subscriptions: Vec<Box<dyn Subscriber<Event>>>,
    ) -> Arc<Mutex<GatewayUser>> {
        let channel = tokio::sync::broadcast::channel(20);
        let user = GatewayUser {
            inbox: channel.1,
            outbox: channel.0.clone(),
            clients,
            id,
            subscriptions,
            connected_users: self.clone(),
        };
        self.register(user)
    }

    /// Create a new [GatewayClient] with the given [GatewayUser], [Connection], and other data.
    /// Also handles appending the new [GatewayClient] to the [GatewayUser]'s list of clients.
    ///
    /// ## Locking
    ///
    /// This method acquires a lock on the [Arc<Mutex<GatewayUser>>] that is passed as `user`.
    #[allow(clippy::too_many_arguments)]
    pub async fn new_client(
        &self,
        user: Arc<Mutex<GatewayUser>>,
        connection: WebSocketConnection,
        main_task_handle: tokio::task::JoinHandle<()>,
        heartbeat_task_handle: tokio::task::JoinHandle<()>,
        session_token: &str,
        last_sequence: Arc<Mutex<u64>>,
    ) -> Arc<Mutex<GatewayClient>> {
        let client = GatewayClient {
            connection,
            parent: Arc::downgrade(&user),
            main_task_handle,
            heartbeat_task_handle,
            session_token: session_token.to_string(),
            last_sequence,
        };
        let arc = Arc::new(Mutex::new(client));
        log::trace!(target: "symfonia::gateway::ConnectedUsers::new_client", "Acquiring lock on user...");
        user.lock()
            .await
            .clients
            .insert(session_token.to_string(), arc.clone());
        log::trace!(target: "symfonia::gateway::ConnectedUsers::new_client", "Lock acquired!");
        log::trace!(target: "symfonia::gateway::ConnectedUsers::new_client", "Inserted into map. Done.");
        arc
    }
}

impl std::hash::Hash for GatewayUser {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for GatewayUser {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for GatewayUser {}

impl GatewayClient {
    /// Disconnects a [GatewayClient] properly, including un-registering it from the memory store
    /// and creating a resumeable session.
    pub async fn die(mut self, connected_users: ConnectedUsers) {
        self.connection.kill_send.send(()).unwrap();
        let disconnect_info = DisconnectInfo {
            session_token: self.session_token.clone(),
            disconnected_at_sequence: *self.last_sequence.lock().await,
            parent: self.parent.clone(),
        };
        self.parent
            .upgrade()
            .unwrap()
            .lock()
            .await
            .clients
            .remove(&self.session_token);
        connected_users.deregister(self.parent.upgrade().unwrap().lock().await.deref());
        connected_users
            .store
            .write()
            .resumeable_clients_store
            .insert(self.session_token.clone(), disconnect_info);
    }
}

#[derive(Default, Clone)]
/// `BulkMessageBuilder` can be used to build and send GatewayMessages to the inboxes of all
/// currently connected [GatewayClients](GatewayClient). Recipients can be added either via
/// User or Role snowflake IDs.
pub struct BulkMessageBuilder {
    users: Vec<Snowflake>,
    roles: Vec<Snowflake>,
    message: Option<Event>,
}

impl BulkMessageBuilder {
    /// Add the given list of user snowflake IDs to the list of recipients.
    pub async fn add_user_recipients(&mut self, users: &[Snowflake]) {
        self.users.extend_from_slice(users);
    }

    /// Add all members which have the given role snowflake IDs to the list of recipients.
    pub async fn add_role_recipients(&mut self, roles: &[Snowflake]) {
        self.roles.extend_from_slice(roles);
    }

    /// Set the message to be sent to the recipients.
    pub async fn set_message(&mut self, message: Event) {
        self.message = Some(message);
    }

    /// Send the message to all recipients.
    pub async fn send(self, connected_users: ConnectedUsers) -> Result<(), crate::errors::Error> {
        if self.message.is_none() {
            return Err(crate::errors::Error::Custom(
                "No message to send".to_string(),
            ));
        }
        let mut recipients = HashSet::new();
        let lock = connected_users.role_user_map.lock().await;
        for role in self.roles.iter() {
            if let Some(users) = lock.get(role) {
                for user in users.iter() {
                    recipients.insert(*user);
                }
            }
            for user in self.users.iter() {
                recipients.insert(*user);
            }
        }
        if recipients.is_empty() {
            return Ok(());
        }
        for recipient in recipients.iter() {
            if let Some(inbox) = connected_users.inbox(*recipient).await {
                inbox.send(self.message.clone().unwrap()).map_err(|e| {
                    crate::errors::Error::Custom(format!("tokio broadcast error: {}", e))
                })?;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
/// Represents all existing roles on the server and the users that have these roles.
pub struct RoleUserMap {
    /// Map Role Snowflake ID to a list of User Snowflake IDs
    map: HashMap<Snowflake, HashSet<Snowflake>>,
}

impl Deref for RoleUserMap {
    type Target = HashMap<Snowflake, HashSet<Snowflake>>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl DerefMut for RoleUserMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl RoleUserMap {
    /// Initialize the [RoleUserMap] with data from the database.
    ///
    /// This method will query the database for all roles and all users that have these roles.
    /// The data will then populate the map.
    ///
    /// Due to the possibly large number of roles and users returned by the database, this method
    /// should only be executed once. The [RoleUserMap] should be kept synchronized with the database
    /// through means that do not involve this method.
    ///
    /// TODO:
    /// Things that need to be accounted for:
    /// - Users who delete or deactivate their accounts
    /// - Users who are deleted from the database
    /// - Users creating an account while the server is running
    ///
    /// If any of these are not accounted for, the RoleUserMap could get out of sync with the database.
    /// This could result in users not receiving events or errors when trying to send an event to
    /// a user that no longer exists.
    pub async fn init(&mut self, db: &PgPool) -> Result<(), crate::errors::Error> {
        // First, get all role ids from the roles table and insert them into the map
        let all_role_ids: Vec<PgU64> = sqlx::query_as("SELECT id FROM roles")
            .fetch_all(db)
            .await
            .map_err(crate::errors::Error::Sqlx)?;
        for role_id in all_role_ids.iter() {
            self.map
                .insert(Snowflake::from(role_id.to_uint()), HashSet::new());
        }
        // Then, query member_roles and insert the user ids into the map
        let all_member_roles: Vec<(PgU64, PgU64)> =
            sqlx::query_as("SELECT index, role_id FROM member_roles")
                .fetch_all(db)
                .await
                .map_err(crate::errors::Error::Sqlx)?;
        for (user_id, role_id) in all_member_roles.iter() {
            // Unwrapping is fine here, as the member_roles table has a foreign key constraint
            // which states that role_id must be a valid id in the roles table.
            let users_for_role_id = self.map.get_mut(&role_id.to_uint().into()).unwrap();
            users_for_role_id.insert(user_id.to_uint().into());
        }
        Ok(())
    }
}

/// Connection to a WebSocket client with sending and receiving capabilities.
///
/// A [WebSocketConnection] is essentially an adapter from tungstenites sink/stream to a
/// [tokio::sync::broadcast] channel. Broadcast channels are used in favor of sink/stream, because
/// to clone a sink/stream to pass it around to different tasks which need sending/receiving
/// capabilities, an `Arc<Mutex<T>>` has to be used. This means, that no more than one task can
/// listen for incoming messages at a time, as a lock on the [Mutex] has to be acquired.
///
/// Read up on [tokio::sync::broadcast] channels if you'd like to understand how they work.
pub struct WebSocketConnection {
    pub sender: tokio::sync::broadcast::Sender<Message>,
    pub receiver: tokio::sync::broadcast::Receiver<Message>,
    pub kill_receive: tokio::sync::broadcast::Receiver<()>,
    pub kill_send: tokio::sync::broadcast::Sender<()>,
    sender_task: Arc<tokio::task::JoinHandle<()>>,
    receiver_task: Arc<tokio::task::JoinHandle<()>>,
}

impl WebSocketConnection {
    /// Create a new [WebSocketConnection] from a tungstenite Sink/Stream pair.
    pub fn new(mut sink: WebSocketSend, mut stream: WebSocketReceive) -> Self {
        // "100" is an arbitrary limit. Feel free to adjust this, if you have a good reason for it. -bitfl0wer
        let (mut websocketsend_sender, mut websocketsend_receiver) =
            tokio::sync::broadcast::channel(100);
        let (mut websocketreceive_sender, mut websocketreceive_receiver) =
            tokio::sync::broadcast::channel(100);

        // The sender task concerns itself with sending messages to the WebSocket client.
        let sender_task = tokio::spawn(async move {
            log::trace!(target: "symfonia::gateway::types::WebSocketConnection", "spawned sender_task");
            loop {
                let message: Result<Message, tokio::sync::broadcast::error::RecvError> =
                    websocketsend_receiver.recv().await;
                match message {
                    Ok(msg) => {
                        let send_result = sink.send(msg).await;
                        match send_result {
                            Ok(_) => (),
                            Err(e) => {
                                log::debug!(target: "symfonia::gateway::types::WebSocketConnection::sender_task", "Error when sending message to WebSocket: {e}");
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!(target: "symfonia::gateway::types::WebSocketConnection::sender_task", "Error when trying to receive through websocketsend_receiver: {e}");
                        break;
                    }
                }
            }
        });

        // The receiver task receives messages from the WebSocket client and sends them to the
        // broadcast channel.
        let receiver_task = tokio::spawn(async move {
            log::trace!(target: "symfonia::gateway::types::WebSocketConnection", "spawned receiver_task");
            loop {
                let web_socket_receive_result = match stream.next().await {
                    Some(res) => res,
                    None => {
                        log::debug!(target: "symfonia::gateway::WebSocketConnection::receiver_task", "WebSocketReceive yielded None. Closing channel");
                        break;
                    }
                };
                let web_socket_receive_message = match web_socket_receive_result {
                    Ok(message) => message,
                    Err(e) => {
                        log::debug!(target: "symfonia::gateway::WebSocketConnection::receiver_task", "Received malformed message, closing channel: {e}");
                        break;
                    }
                };
                match websocketreceive_sender.send(web_socket_receive_message) {
                    Ok(_) => (),
                    Err(e) => {
                        log::debug!(target: "symfonia::gateway::WebSocketConnection::receiver_task", "Unable to send received WebSocket message to channel recipients. Closing channel: {e}");
                        break;
                    }
                }
            }
        });
        let (kill_send, kill_receive) = tokio::sync::broadcast::channel(1);
        Self {
            sender: websocketsend_sender,
            receiver: websocketreceive_receiver,
            sender_task: Arc::new(sender_task),
            receiver_task: Arc::new(receiver_task),
            kill_receive,
            kill_send,
        }
    }
}

impl Clone for WebSocketConnection {
    fn clone(&self) -> Self {
        log::trace!(target: "symfonia::gateway::WebSocketConnection", "WebSocketConnection cloned!");
        Self {
            sender: self.sender.clone(),
            receiver: self.receiver.resubscribe(),
            sender_task: self.sender_task.clone(),
            receiver_task: self.receiver_task.clone(),
            kill_receive: self.kill_receive.resubscribe(),
            kill_send: self.kill_send.clone(),
        }
    }
}

#[derive(Clone)]
pub struct DisconnectInfo {
    /// session token that was used for this connection
    pub session_token: String,
    pub disconnected_at_sequence: u64,
    pub parent: Weak<Mutex<GatewayUser>>,
}

impl
    From<(
        SplitSink<WebSocketStream<TcpStream>, tokio_tungstenite::tungstenite::Message>,
        SplitStream<WebSocketStream<TcpStream>>,
    )> for WebSocketConnection
{
    fn from(
        value: (
            SplitSink<WebSocketStream<TcpStream>, tokio_tungstenite::tungstenite::Message>,
            SplitStream<WebSocketStream<TcpStream>>,
        ),
    ) -> Self {
        Self::new(value.0, value.1)
    }
}

/// Represents a new successful connection to the gateway. The user is already part of the [ConnectedUsers]
/// and the client is already registered with the [GatewayClient] "clients" map.
pub struct NewWebSocketConnection {
    pub user: Arc<Mutex<GatewayUser>>,
    pub client: Arc<Mutex<GatewayClient>>,
}
