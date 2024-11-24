//! API for endpoints under `api/client/servers/{server}/databases`

use crate::client::{IpAndPort, Server};
use crate::http::EmptyBody;
use crate::structs::{PteroList, PteroObject};
use reqwest::Method;
use serde::{Deserialize, Deserializer, Serialize};

/// A database on a server
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct ServerDatabase {
    /// The ID of the database
    pub id: String,
    /// The IP of the database
    pub host: IpAndPort,
    /// The name of the database
    pub name: String,
    /// The username required to login to the database
    pub username: String,
    /// Where the database accepts connections from
    pub connections_from: String,
    /// The maximum number of connections to the database at a time
    pub max_connections: u64,
    /// Additional information about the database
    #[serde(default)]
    pub relationships: DatabaseRelationships,
}

/// Additional information about a database
#[derive(Debug, Deserialize, Default)]
#[non_exhaustive]
pub struct DatabaseRelationships {
    /// The database password required to login to the database. May not be present if the request
    /// doesn't return the password.
    #[serde(deserialize_with = "deserialize_password")]
    #[serde(default)]
    pub password: Option<String>,
}

fn deserialize_password<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct PasswordObj {
        password: String,
    }
    let password: PteroObject<PasswordObj> = Deserialize::deserialize(deserializer)?;
    Ok(Some(password.attributes.password))
}

impl Server<'_> {
    /// Lists the databases on a server
    pub async fn list_databases(&self) -> crate::Result<Vec<ServerDatabase>> {
        self.client
            .request::<PteroList<ServerDatabase>>(
                Method::GET,
                &format!("servers/{}/databases", self.id),
            )
            .await
            .map(|databases| databases.data)
    }

    /// Creates a database with the given name. You must also specify who can connect to the
    /// database, or else use the `"%"` wildcard
    pub async fn create_database(
        &self,
        name: impl Into<String>,
        remote: impl Into<String>,
    ) -> crate::Result<ServerDatabase> {
        #[derive(Serialize)]
        struct CreateDatabaseBody {
            database: String,
            remote: String,
        }
        self.client
            .request_with_body::<PteroObject<ServerDatabase>, _>(
                Method::POST,
                &format!("servers/{}/databases", self.id),
                &CreateDatabaseBody {
                    database: name.into(),
                    remote: remote.into(),
                },
            )
            .await
            .map(|database| database.attributes)
    }

    /// Rotates the password of the given database
    pub async fn rotate_database_password(
        &self,
        id: impl Into<String>,
    ) -> crate::Result<ServerDatabase> {
        self.client
            .request::<PteroObject<ServerDatabase>>(
                Method::POST,
                &format!(
                    "servers/{}/databases/{}/rotate-password",
                    self.id,
                    id.into()
                ),
            )
            .await
            .map(|database| database.attributes)
    }

    /// Deletes the given database
    pub async fn delete_database(&self, id: impl Into<String>) -> crate::Result<()> {
        self.client
            .request::<EmptyBody>(
                Method::DELETE,
                &format!("servers/{}/databases/{}", self.id, id.into()),
            )
            .await?;
        Ok(())
    }
}
