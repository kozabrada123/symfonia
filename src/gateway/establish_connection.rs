use std::sync::Arc;

use chorus::types::{
    GatewayHeartbeat, GatewayHeartbeatAck, GatewayHello, GatewayIdentifyPayload, GatewayResume,
    Snowflake,
};
use futures::{SinkExt, StreamExt};
use log::trace;
use rand::seq;
use serde_json::{from_str, json};
use sqlx::PgPool;
use tokio::net::TcpStream;
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use crate::database::entities::Config;
use crate::errors::{Error, GatewayError};
use crate::gateway::heartbeat::HeartbeatHandler;
use crate::gateway::resume_connection::resume_connection;
use crate::gateway::{gateway_task, GatewayUser};
use crate::util::token::check_token;

use super::{Connection, GatewayClient, GatewayUsersStore, NewConnection};

/// `establish_connection` is the entrypoint method that gets called when a client tries to connect
/// to the WebSocket server.
///
/// If successful, returns a [NewConnection] with a new [Arc<Mutex<GatewayUser>>] and a
/// [GatewayClient], whose `.parent` field contains a [Weak] reference to the new [GatewayUser].
pub(super) async fn establish_connection(
    stream: TcpStream,
    db: PgPool, // TODO: Do we need db here?
    config: Config,
    gateway_users_store: GatewayUsersStore,
) -> Result<NewConnection, Error> {
    trace!(target: "symfonia::gateway::establish_connection", "Beginning process to establish connection (handshake)");
    let ws_stream = accept_async(stream).await?;
    let mut connection: Connection = ws_stream.split().into();
    trace!(target: "symfonia::gateway::establish_connection", "Sending hello message");
    // Hello message
    connection
        .sender
        .send(Message::Text(json!(GatewayHello::default()).to_string()))
        .await?;
    trace!(target: "symfonia::gateway::establish_connection", "Sent hello message");

    let connection = Arc::new(Mutex::new(connection));

    let mut received_identify_or_resume = false;

    let (kill_send, mut kill_receive) = tokio::sync::broadcast::channel::<()>(1);
    let (message_send, message_receive) = tokio::sync::broadcast::channel::<GatewayHeartbeat>(4);
    let sequence_number = Arc::new(Mutex::new(0u64));
    let (session_id_send, session_id_receive) = tokio::sync::broadcast::channel::<String>(1);

    // This JoinHandle `.is_some()` if we receive a heartbeat message *before* we receive an
    // identify or resume message.
    let mut heartbeat_handler_handle: Option<JoinHandle<()>> = None;

    trace!(target: "symfonia::gateway::establish_connection", "Waiting for next message, timeout or kill signal...");
    let mut second_kill_receive = kill_receive.resubscribe();
    tokio::select! {
        _ = second_kill_receive.recv() => {
            trace!(target: "symfonia::gateway::establish_connection", "Connection was closed before we could establish it");
            return Err(GatewayError::Closed.into());
        }
        // If we do not receive an identifying or resuming message within 30 seconds, we close the connection.
        _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
            trace!(target: "symfonia::gateway::establish_connection", "Connection timed out: No message received within 30 seconds");
            return Err(GatewayError::Timeout.into());
        }
        new_connection = finish_connecting(
            connection.clone(),
            heartbeat_handler_handle,
            kill_receive,
            kill_send,
            message_receive,
            message_send,
            sequence_number,
            session_id_receive,
            db,
            &config,
            gateway_users_store.clone(),
        ) => {
            return new_connection;
        }
    }

    todo!()
}

/// `get_or_new_gateway_user` is a helper function that retrieves a [GatewayUser] from the store if it exists,
/// or creates a new user, stores it in the store and then returns it, if it does not exist.
async fn get_or_new_gateway_user(
    user_id: Snowflake,
    store: GatewayUsersStore,
) -> Arc<tokio::sync::Mutex<GatewayUser>> {
    let mut store = store.lock().await;
    if let Some(user) = store.get(&user_id) {
        return user.clone();
    }
    let user = Arc::new(Mutex::new(GatewayUser {
        id: user_id,
        clients: Vec::new(),
        subscriptions: Vec::new(),
    }));
    store.insert(user_id, user.clone());
    user
}

async fn finish_connecting(
    connection: Arc<Mutex<Connection>>,
    mut heartbeat_handler_handle: Option<JoinHandle<()>>,
    kill_receive: tokio::sync::broadcast::Receiver<()>,
    kill_send: tokio::sync::broadcast::Sender<()>,
    message_receive: tokio::sync::broadcast::Receiver<GatewayHeartbeat>,
    message_send: tokio::sync::broadcast::Sender<GatewayHeartbeat>,
    sequence_number: Arc<Mutex<u64>>,
    session_id_receive: tokio::sync::broadcast::Receiver<String>,
    db: PgPool,
    config: &Config,
    gateway_users_store: GatewayUsersStore,
) -> Result<NewConnection, Error> {
    loop {
        trace!(target: "symfonia::gateway::establish_connection", "No resume or identify message received yet, waiting for next message...");
        trace!(target: "symfonia::gateway::establish_connection", "Waiting for next message...");
        let raw_message = match connection.lock().await.receiver.next().await {
            Some(next) => next,
            None => return Err(GatewayError::Timeout.into()),
        }?;
        trace!(target: "symfonia::gateway::establish_connection", "Received message: {:?}", raw_message);

        if let Ok(heartbeat) = from_str::<GatewayHeartbeat>(&raw_message.to_string()) {
            log::trace!(target: "symfonia::gateway::establish_connection", "Received heartbeat");
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
                            connection.clone(),
                            kill_receive.resubscribe(),
                            kill_send.clone(),
                            message_receive.resubscribe(),
                            sequence_number.clone(),
                            session_id_receive.resubscribe(),
                        );
                        async move {
                            heartbeat_handler.run().await;
                        }
                    }))
                }
                Some(_) => {
                    message_send.send(heartbeat);
                }
            }
        } else if let Ok(identify) = from_str::<GatewayIdentifyPayload>(&raw_message.to_string()) {
            log::trace!(target: "symfonia::gateway::establish_connection", "Received identify payload");
            let claims = match check_token(&db, &identify.token, &config.security.jwt_secret).await
            {
                Ok(claims) => claims,
                Err(_) => {
                    log::trace!(target: "symfonia::gateway::establish_connection", "Failed to verify token");
                    kill_send.send(()).expect("Failed to send kill signal");
                    return Err(crate::errors::UserError::InvalidToken.into());
                }
            };
            let mut gateway_user =
                get_or_new_gateway_user(claims.id, gateway_users_store.clone()).await;
            let gateway_client = GatewayClient {
                parent: Arc::downgrade(&gateway_user),
                connection: connection.clone(),
                main_task_handle: tokio::spawn(gateway_task::gateway_task(connection.clone())),
                heartbeat_task_handle: match heartbeat_handler_handle {
                    Some(handle) => handle,
                    None => tokio::spawn({
                        let mut heartbeat_handler = HeartbeatHandler::new(
                            connection.clone(),
                            kill_receive.resubscribe(),
                            kill_send.clone(),
                            message_receive.resubscribe(),
                            sequence_number.clone(),
                            session_id_receive.resubscribe(),
                        );
                        async move {
                            heartbeat_handler.run().await;
                        }
                    }),
                },
                kill_send,
                disconnect_info: None,
                session_token: identify.token,
            };
            let gateway_client_arc_mutex = Arc::new(Mutex::new(gateway_client));
            gateway_user
                .lock()
                .await
                .clients
                .push(gateway_client_arc_mutex.clone());
            return Ok(NewConnection {
                user: gateway_user,
                client: gateway_client_arc_mutex.clone(),
            });
        } else if let Ok(resume) = from_str::<GatewayResume>(&raw_message.to_string()) {
            log::trace!(target: "symfonia::gateway::establish_connection", "Received resume payload");
            return resume_connection(connection, db, config.to_owned(), resume).await;
        } else {
            trace!(target: "symfonia::gateway::establish_connection", "Received unexpected message: {:?}", raw_message);
            return Err(GatewayError::UnexpectedMessage.into());
        }
    }
}
