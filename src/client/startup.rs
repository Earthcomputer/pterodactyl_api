//! API for endpoints under `api/client/servers/{server}/startup`

use crate::client::Server;
use crate::structs::PteroObject;
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// The startup data for a server
#[derive(Debug)]
pub struct StartupData {
    /// The startup command without variables substituted
    pub startup_command: String,
    /// The startup command with variables substituted
    pub raw_startup_command: String,
    /// The startup variables
    pub variables: Vec<Variable>,
}

/// A startup variable
#[derive(Debug, Deserialize)]
pub struct Variable {
    /// The name of the variable
    pub name: String,
    /// The description of the variable
    pub description: String,
    /// The corresponding environment variable for this variable
    pub env_variable: String,
    /// The default value
    pub default_value: String,
    /// The current value
    pub server_value: String,
    /// Whether this variable is editable
    pub is_editable: bool,
    /// The rules for what this variable can hold
    pub rules: String,
}

impl Server<'_> {
    /// Gets the startup data for this server
    pub async fn get_startup_data(&self) -> crate::Result<StartupData> {
        #[derive(Deserialize)]
        struct StartupMeta {
            startup_command: String,
            raw_startup_command: String,
        }
        #[derive(Deserialize)]
        struct StartupDataObj {
            data: Vec<PteroObject<Variable>>,
            meta: StartupMeta,
        }
        self.client
            .request::<StartupDataObj>(Method::GET, &format!("servers/{}/startup", self.id))
            .await
            .map(|data| StartupData {
                startup_command: data.meta.startup_command,
                raw_startup_command: data.meta.raw_startup_command,
                variables: data.data.into_iter().map(|var| var.attributes).collect(),
            })
    }

    /// Sets a startup variable for this server
    pub async fn set_startup_variable(
        &self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> crate::Result<Variable> {
        #[derive(Serialize)]
        struct SetStartupVariableBody {
            key: String,
            value: String,
        }
        self.client
            .request_with_body::<PteroObject<Variable>, _>(
                Method::PUT,
                &format!("servers/{}/startup/variable", self.id),
                &SetStartupVariableBody {
                    key: name.into(),
                    value: value.into(),
                },
            )
            .await
            .map(|variable| variable.attributes)
    }
}
