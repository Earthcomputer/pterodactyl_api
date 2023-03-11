//! Pterodactyl Client API implementation, for all endpoints under `api/client`

use crate::http::EmptyBody;
use crate::structs::{PteroList, PteroObject};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
pub use structs::*;

pub mod account;
pub mod backups;
pub mod databases;
pub mod files;
pub mod network;
pub mod schedules;
pub mod settings;
pub mod startup;
mod structs;
pub mod users;
#[cfg(feature = "websocket")]
pub mod websocket;

/// The rate limits of the API key
#[derive(Debug, Copy, Clone)]
pub struct RateLimits {
    /// The request limit per minute
    pub limit: u32,
    /// The number of requests remaining in this minute
    pub limit_remaining: u32,
}

/// A Pterodactyl client, to make requests to the Pterodactyl client API
#[derive(Debug)]
pub struct Client {
    pub(crate) url: String,
    pub(crate) client: reqwest::Client,
    pub(crate) api_key: String,
    pub(crate) rate_limits: RwLock<Option<RateLimits>>,
}

impl Client {
    /// Gets the rate limit information after the previous request
    pub fn get_rate_limits(&self) -> Option<RateLimits> {
        *self.rate_limits.read().unwrap()
    }

    /// Lists the servers that this account has access to
    pub async fn list_servers(&self) -> crate::Result<Vec<ServerStruct>> {
        self.request::<PteroList<ServerStruct>>(Method::GET, "")
            .await
            .map(|servers| servers.data)
    }

    /// Gets all available permissions on this instance of Pterodactyl
    pub async fn get_permissions(&self) -> crate::Result<HashMap<String, PermissionGroup>> {
        #[derive(Deserialize)]
        struct Permissions {
            permissions: HashMap<String, PermissionGroup>,
        }
        self.request::<PteroObject<Permissions>>(Method::GET, "permissions")
            .await
            .map(|permissions| permissions.attributes.permissions)
    }

    /// Gets a server with a specific ID, which can be used to make requests specific to that server
    pub fn get_server(&self, id: impl Into<String>) -> Server<'_> {
        Server {
            id: id.into(),
            client: self,
        }
    }

    #[cfg(test)]
    pub(crate) fn get_test_server(&self) -> Server {
        self.get_server(
            std::env::var("TEST_SERVER").expect("Expected TEST_SERVER in environment variables"),
        )
    }
}

/// A builder for a client
#[derive(Debug)]
pub struct ClientBuilder {
    url: String,
    client: Option<reqwest::Client>,
    api_key: String,
}

impl ClientBuilder {
    /// Creates a new client builder, connecting to the given URL where a Pterodactyl server is
    /// hosted, using the given API key for authentication
    pub fn new(url: impl Into<String>, api_key: impl Into<String>) -> Self {
        let mut url = url.into();
        if !url.ends_with('/') {
            url.push('/');
        }
        url.push_str("api/client/");
        Self {
            url,
            client: None,
            api_key: api_key.into(),
        }
    }

    /// Uses the specified [`reqwest::Client`] for requests instead of making a default one
    pub fn with_client(self, client: reqwest::Client) -> Self {
        Self {
            client: Some(client),
            ..self
        }
    }

    /// Builds a client
    pub fn build(self) -> Client {
        Client {
            url: self.url,
            client: self.client.unwrap_or_default(),
            api_key: self.api_key,
            rate_limits: RwLock::new(None),
        }
    }
}

/// Represents a Pterodactyl server, with which requests specific to a server can be made
#[derive(Debug)]
pub struct Server<'a> {
    pub(crate) id: String,
    pub(crate) client: &'a Client,
}

impl Server<'_> {
    /// Gets information about this server
    pub async fn get_details(&self) -> crate::Result<ServerStruct> {
        self.client
            .request::<PteroObject<ServerStruct>>(Method::GET, &format!("servers/{}", self.id))
            .await
            .map(|server| server.attributes)
    }

    /// Gets resources for this server
    pub async fn get_resources(&self) -> crate::Result<ServerResources> {
        self.client
            .request::<PteroObject<ServerResources>>(
                Method::GET,
                &format!("servers/{}/resources", self.id),
            )
            .await
            .map(|resources| resources.attributes)
    }

    /// Sends a command to this server
    pub async fn send_command(&self, command: impl Into<String>) -> crate::Result<()> {
        #[derive(Serialize)]
        struct SendCommandBody {
            command: String,
        }
        self.client
            .request_with_body::<EmptyBody, _>(
                Method::POST,
                &format!("servers/{}/command", self.id),
                &SendCommandBody {
                    command: command.into(),
                },
            )
            .await?;
        Ok(())
    }

    /// Sends a power signal to this server
    pub async fn send_power_signal(&self, signal: PowerSignal) -> crate::Result<()> {
        #[derive(Serialize)]
        struct SendPowerSignalBody {
            signal: PowerSignal,
        }
        self.client
            .request_with_body::<EmptyBody, _>(
                Method::POST,
                &format!("servers/{}/power", self.id),
                &SendPowerSignalBody { signal },
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::client::backups::BackupParams;
    use crate::client::schedules::ScheduleParams;
    use crate::client::websocket::{PteroWebSocketHandle, PteroWebSocketListener};
    use crate::client::{Client, ClientBuilder, EmptyBody, ServerState};
    use async_trait::async_trait;

    fn make_test_client() -> Client {
        ClientBuilder::new(
            std::env::var("API_URL").expect("Expected API_URL in environment variables"),
            std::env::var("API_KEY").expect("Expected API_KEY in environment variables"),
        )
        .build()
    }

    #[tokio::test]
    async fn test_list_servers() {
        println!("{:?}", make_test_client().list_servers().await);
    }

    #[tokio::test]
    async fn test_get_permissions() {
        println!("{:?}", make_test_client().get_permissions().await);
    }

    #[tokio::test]
    async fn test_get_account_details() {
        println!("{:?}", make_test_client().get_account_details().await)
    }

    #[tokio::test]
    async fn test_get_account_2fa_details() {
        println!("{:?}", make_test_client().get_account_2fa_details().await)
    }

    #[tokio::test]
    async fn test_get_api_keys() {
        println!("{:?}", make_test_client().get_api_keys().await)
    }

    #[tokio::test]
    async fn test_get_server_details() {
        println!(
            "{:?}",
            make_test_client().get_test_server().get_details().await
        )
    }

    #[tokio::test]
    async fn test_get_server_resources() {
        println!(
            "{:?}",
            make_test_client().get_test_server().get_resources().await
        )
    }

    #[tokio::test]
    async fn test_list_databases() {
        println!(
            "{:?}",
            make_test_client().get_test_server().list_databases().await
        )
    }

    #[tokio::test]
    async fn test_list_files() {
        println!(
            "{:?}",
            make_test_client().get_test_server().list_files("/").await
        )
    }

    #[tokio::test]
    async fn test_file_content() {
        println!(
            "{:?}",
            make_test_client()
                .get_test_server()
                .file_contents_text("eula.txt")
                .await
        )
    }
}
