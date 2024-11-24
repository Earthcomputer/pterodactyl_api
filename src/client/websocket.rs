//! A Pterodactyl websocket client

use crate::client::{PowerSignal, Server, ServerState};
use crate::Error::WebsocketTokenExpired;
use async_tungstenite::tungstenite::Message;
use async_tungstenite::WebSocketStream;
use futures_io::{AsyncRead, AsyncWrite};
use futures_util::{SinkExt, StreamExt};
use reqwest::Method;
use serde::de::value::StrDeserializer;
use serde::{Deserialize, Serialize};
use std::future::Future;

#[doc(hidden)]
mod sealed {
    use super::WebSocketHandleImpl;
    use futures_io::{AsyncRead, AsyncWrite};

    pub(super) trait Sealed {}
    impl<S> Sealed for WebSocketHandleImpl<'_, S> where S: AsyncRead + AsyncWrite + Unpin + Send {}
}

struct WebSocketImpl<'a, S, L> {
    server: &'a Server<'a>,
    socket: WebSocketStream<S>,
    listener: L,
    ready: bool,
}

#[allow(missing_docs)]
#[derive(Debug)]
pub struct WebSocketHandleImpl<'a, S> {
    socket: &'a mut WebSocketStream<S>,
    stop: bool,
}

/// An event listener that gets called when websocket messages are received
pub trait PteroWebSocketListener<H: PteroWebSocketHandle>: Send {
    /// Called when the websocket is ready to use
    fn on_ready(&mut self, _handle: &mut H) -> impl Future<Output = crate::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when a server status message is received
    fn on_status(
        &mut self,
        _handle: &mut H,
        _status: ServerState,
    ) -> impl Future<Output = crate::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when a console output message is received
    fn on_console_output(
        &mut self,
        _handle: &mut H,
        _output: &str,
    ) -> impl Future<Output = crate::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when a server stats message is received
    fn on_stats(
        &mut self,
        _handle: &mut H,
        _stats: ServerStats,
    ) -> impl Future<Output = crate::Result<()>> + Send {
        async { Ok(()) }
    }
}

/// A handle to control the websocket
#[allow(private_bounds)]
pub trait PteroWebSocketHandle: Send + sealed::Sealed {
    /// Request server stats
    fn request_stats(&mut self) -> impl Future<Output = crate::Result<()>> + Send;
    /// Request server logs
    fn request_logs(&mut self) -> impl Future<Output = crate::Result<()>> + Send;
    /// Send a power signal to the server
    fn send_power_signal(
        &mut self,
        signal: PowerSignal,
    ) -> impl Future<Output = crate::Result<()>> + Send;
    /// Send a command to the server
    fn send_command(
        &mut self,
        command: impl Into<String> + Send,
    ) -> impl Future<Output = crate::Result<()>> + Send;
    /// End the websocket connection
    fn disconnect(&mut self);
}

/// Server stats received from a websocket
#[derive(Debug, Deserialize, Copy, Clone)]
#[non_exhaustive]
pub struct ServerStats {
    /// The used memory of the server in bytes
    pub memory_bytes: u64,
    /// The maximum amount of memory the server can use in bytes
    pub memory_limit_bytes: u64,
    /// The CPU usage of the server
    pub cpu_absolute: f32,
    /// The network statistics of the server
    pub network: ServerNetworkStats,
    /// The current state of the server
    pub state: ServerState,
    /// The disk usage of the server in bytes
    pub disk_bytes: u64,
}

/// Server network stats received from a websocket
#[derive(Debug, Deserialize, Copy, Clone)]
pub struct ServerNetworkStats {
    /// Number of bytes received
    pub rx_bytes: u64,
    /// Number of bytes transmitted
    pub tx_bytes: u64,
}

#[derive(Deserialize)]
struct WebSocketLink {
    token: String,
    socket: String,
}

impl<'a> Server<'a> {
    /// Runs the websocket loop until the websocket is disconnected. Takes a function which creates
    /// a websocket stream from a websocket URL, and an event listener
    pub async fn run_websocket_loop<S, F, L>(
        &self,
        create: impl FnOnce(String) -> F,
        listener: L,
    ) -> crate::Result<()>
    where
        F: Future<Output = async_tungstenite::tungstenite::Result<WebSocketStream<S>>>,
        L: for<'b> PteroWebSocketListener<WebSocketHandleImpl<'b, S>>,
        S: AsyncRead + AsyncWrite + Unpin + Send,
    {
        let WebSocketLink { token, socket: url } = self.get_websocket_link().await?;
        let socket = create(url).await?;
        let socket = WebSocketImpl {
            server: self,
            socket,
            listener,
            ready: false,
        };
        socket.run_loop(token).await
    }

    async fn get_websocket_link(&self) -> crate::Result<WebSocketLink> {
        #[derive(Deserialize)]
        struct Data {
            data: WebSocketLink,
        }
        self.client
            .request::<Data>(Method::GET, &format!("servers/{}/websocket", self.id))
            .await
            .map(|link| link.data)
    }
}

#[derive(Deserialize, PartialEq, Eq)]
#[non_exhaustive]
enum IncomingEvent {
    #[serde(rename = "auth success")]
    AuthSuccess,
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "console output")]
    ConsoleOutput,
    #[serde(rename = "stats")]
    Stats,
    #[serde(rename = "token expiring")]
    TokenExpiring,
    #[serde(rename = "token expired")]
    TokenExpired,
    #[serde(other)]
    Other,
}

impl<S, L> WebSocketImpl<'_, S, L>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
    L: for<'b> PteroWebSocketListener<WebSocketHandleImpl<'b, S>>,
{
    async fn run_loop(mut self, token: String) -> crate::Result<()> {
        self.auth(token).await?;
        while let Some(message) = self.socket.next().await {
            if let Message::Text(message) = message? {
                if self.handle_message(message).await? {
                    break;
                }
            } else {
                return Err(crate::Error::UnexpectedMessage);
            }
        }
        Ok(())
    }

    async fn handle_message(&mut self, message: String) -> crate::Result<bool> {
        #[derive(Deserialize)]
        struct Message {
            event: IncomingEvent,
            #[serde(default)]
            args: Vec<String>,
        }
        let message: Message = serde_json::from_str(&message)?;
        if message.event != IncomingEvent::AuthSuccess && !self.ready {
            return Err(crate::Error::UnexpectedMessage);
        }

        match message.event {
            IncomingEvent::AuthSuccess => {
                let mut handle = WebSocketHandleImpl {
                    socket: &mut self.socket,
                    stop: false,
                };
                if !self.ready {
                    self.ready = true;
                    self.listener.on_ready(&mut handle).await?;
                }
                Ok(handle.stop)
            }
            IncomingEvent::Status => {
                let mut handle = WebSocketHandleImpl {
                    socket: &mut self.socket,
                    stop: false,
                };
                self.listener
                    .on_status(
                        &mut handle,
                        ServerState::deserialize(StrDeserializer::<serde_json::Error>::new(
                            message
                                .args
                                .first()
                                .ok_or(crate::Error::UnexpectedMessage)?,
                        ))?,
                    )
                    .await?;
                Ok(handle.stop)
            }
            IncomingEvent::ConsoleOutput => {
                let mut handle = WebSocketHandleImpl {
                    socket: &mut self.socket,
                    stop: false,
                };
                for output in message.args {
                    self.listener
                        .on_console_output(&mut handle, &output)
                        .await?;
                }
                Ok(handle.stop)
            }
            IncomingEvent::Stats => {
                let mut handle = WebSocketHandleImpl {
                    socket: &mut self.socket,
                    stop: false,
                };
                let json = message
                    .args
                    .first()
                    .ok_or(crate::Error::UnexpectedMessage)?;
                let stats: ServerStats = serde_json::from_str(json)?;
                self.listener.on_stats(&mut handle, stats).await?;
                Ok(handle.stop)
            }
            IncomingEvent::TokenExpiring => {
                self.auth(self.server.get_websocket_link().await?.token)
                    .await?;
                Ok(false)
            }
            IncomingEvent::TokenExpired => Err(WebsocketTokenExpired),
            IncomingEvent::Other => Ok(false),
        }
    }

    async fn auth(&mut self, token: String) -> crate::Result<()> {
        #[derive(Serialize)]
        struct AuthEvent {
            event: &'static str,
            args: [String; 1],
        }
        let payload = serde_json::to_string(&AuthEvent {
            event: "auth",
            args: [token],
        })?;
        Ok(self.socket.send(Message::text(payload)).await?)
    }
}

impl<S> PteroWebSocketHandle for WebSocketHandleImpl<'_, S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    async fn request_stats(&mut self) -> crate::Result<()> {
        Ok(self
            .socket
            .send(Message::text("{\"event\":\"send stats\",\"args\":[null]}"))
            .await?)
    }

    async fn request_logs(&mut self) -> crate::Result<()> {
        Ok(self
            .socket
            .send(Message::text("{\"event\":\"send logs\",\"args\":[null]}"))
            .await?)
    }

    async fn send_power_signal(&mut self, signal: PowerSignal) -> crate::Result<()> {
        #[derive(Serialize)]
        struct PowerSignalEvent {
            event: &'static str,
            args: [PowerSignal; 1],
        }
        let payload = serde_json::to_string(&PowerSignalEvent {
            event: "set state",
            args: [signal],
        })?;
        Ok(self.socket.send(Message::text(payload)).await?)
    }

    async fn send_command(&mut self, command: impl Into<String> + Send) -> crate::Result<()> {
        #[derive(Serialize)]
        struct CommandEvent {
            event: &'static str,
            args: [String; 1],
        }
        let payload = serde_json::to_string(&CommandEvent {
            event: "send command",
            args: [command.into()],
        })?;
        Ok(self.socket.send(Message::text(payload)).await?)
    }

    fn disconnect(&mut self) {
        self.stop = true;
    }
}
