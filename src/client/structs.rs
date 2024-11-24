use crate::client::network::Allocation;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use uuid::Uuid;

/// A server
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct ServerStruct {
    /// Whether the connected account is the owner of this server
    pub server_owner: bool,
    /// The ID of this server
    pub identifier: String,
    /// The UUID of this server
    pub uuid: Uuid,
    /// The name of this server
    pub name: String,
    /// The node that this server is running on
    pub node: String,
    /// Whether the node that this server is running on is currently under maintenance
    #[serde(default)]
    pub is_node_under_maintenance: bool,
    /// The SFTP IP and port for this server
    pub sftp_details: IpAndPort,
    /// The description of this server
    pub description: Option<String>,
    /// The virtual hardware limits for this server
    pub limits: ServerLimits,
    /// The startup command for this server
    pub invocation: String,
    /// The docker image for this server
    pub docker_image: String,
    /// The egg features enabled on this server
    #[serde(deserialize_with = "crate::structs::default_on_null")]
    pub egg_features: Vec<String>,
    /// Limits for various features on this server
    pub feature_limits: ServerFeatureLimits,
    /// The current status of this server
    pub status: Option<ServerStatus>,
    /// Whether this server is being transferred
    pub is_transferring: bool,
    /// Extra metadata for this server
    #[serde(default)]
    pub relationships: ServerRelationships,
}

/// Represents an IP and port combination
#[derive(Debug, Deserialize)]
pub struct IpAndPort {
    /// The IP
    #[serde(alias = "address")]
    pub ip: String,
    /// The port
    pub port: u16,
}

/// Virtual hardware limits for a server
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct ServerLimits {
    /// Maximum memory, or 0 for unlimited
    pub memory: u64,
    /// Maximum swap memory, or 0 for unlimited
    pub swap: i64,
    /// Maximum disk space, or 0 for unlimited
    pub disk: u64,
    /// Maximum I/O speed, or 0 for unlimited
    pub io: u32,
    /// Maximum CPU usage, or 0 for unlimited
    pub cpu: f32,
    /// Which threads this server should run on, or an empty list for unrestricted
    #[serde(deserialize_with = "threads")]
    pub threads: Option<Vec<u64>>,
    /// Whether the out of memory killer is enabled on this server, or None for unknown
    pub oom_killer: Option<bool>,
}

fn threads<'de, D>(deserializer: D) -> Result<Option<Vec<u64>>, D::Error>
where
    D: Deserializer<'de>,
{
    let str: Option<String> = Deserialize::deserialize(deserializer)?;
    if let Some(str) = str {
        Some(
            str.split(',')
                .map(|part| part.parse())
                .collect::<Result<_, _>>()
                .map_err(|err| <D::Error as serde::de::Error>::custom(format!("{err}"))),
        )
        .transpose()
    } else {
        Ok(None)
    }
}

/// Limits for various feature on the server
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct ServerFeatureLimits {
    /// The maximum number of databases
    pub databases: Option<u64>,
    /// The maximum number of network allocations
    pub allocations: Option<u64>,
    /// The maximum number of backups
    pub backups: Option<u64>,
}

/// The status of a server
#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
    /// This server is being installed
    Installing,
    /// The installation of this server has failed
    InstallFailed,
    /// The reinstallation of this server has failed
    ReinstallFailed,
    /// This server has been suspended
    Suspended,
    /// Currently restoring a backup
    RestoringBackup,
}

/// Extra metadata for a server
#[derive(Debug, Default, Deserialize)]
#[non_exhaustive]
pub struct ServerRelationships {
    /// The network allocations of this server
    #[serde(deserialize_with = "crate::structs::ptero_list")]
    pub allocations: Vec<Allocation>,
}

/// A group of permissions
#[derive(Debug, Deserialize)]
pub struct PermissionGroup {
    /// The description of this group
    pub description: String,
    /// The permissions in this group. Keys represent permission names, values represent their
    /// descriptions
    pub keys: HashMap<String, String>,
}

/// Represents the current resources of a server
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct ServerResources {
    /// The current state of the server
    pub current_state: ServerState,
    /// Whether the server is suspended
    pub is_suspended: bool,
    /// Used resources of the server
    pub resources: ServerResourcesResources,
}

/// Represents resource usage of a server
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct ServerResourcesResources {
    /// The amount of memory used, in bytes
    pub memory_bytes: u64,
    /// The CPU usage
    pub cpu_absolute: f32,
    /// Disk usage in bytes
    pub disk_bytes: u64,
    /// The amount of data transmitted over the network, in bytes
    pub network_rx_bytes: u64,
    /// The amount of data received over the network, in bytes
    pub network_tx_bytes: u64,
    /// Uptime in seconds
    pub uptime: u64,
}

/// The state of a server
#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ServerState {
    /// The server is offline
    Offline,
    /// The server is starting
    Starting,
    /// The server is running
    Running,
    /// The server is stopping
    Stopping,
}

/// A power signal to send to the server
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Hash, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum PowerSignal {
    /// Start the server
    Start,
    /// Stop the server gracefully
    Stop,
    /// Restart the server gracefully
    Restart,
    /// Stop the server forcefully
    Kill,
}

impl Display for PowerSignal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PowerSignal::Start => f.write_str("start"),
            PowerSignal::Stop => f.write_str("stop"),
            PowerSignal::Restart => f.write_str("restart"),
            PowerSignal::Kill => f.write_str("kill"),
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct ErrorResponse {
    pub(crate) errors: Vec<ErrorResponseError>,
}

impl ErrorResponse {
    pub(crate) fn is_error(&self, error: &str) -> bool {
        self.errors.iter().any(|e| e.code == error)
    }
}

#[derive(Deserialize)]
pub(crate) struct ErrorResponseError {
    pub(crate) code: String,
}
