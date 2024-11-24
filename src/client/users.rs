//! API for endpoints under `api/client/servers/{server}/users`

use crate::client::Server;
use crate::http::EmptyBody;
use crate::structs::{PteroList, PteroObject};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// A user on a server that holds permissions for that server
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct User {
    /// The ID of this user
    pub uuid: Uuid,

    /// The username of this user
    pub username: String,

    /// The email of this user
    pub email: String,

    /// The avatar URL of this user
    pub image: String,

    /// Whether this user has 2fa enabled
    #[serde(rename = "2fa_enabled")]
    pub two_factor_enabled: bool,

    /// When this user was added to this server
    #[serde(deserialize_with = "crate::structs::iso_time")]
    pub created_at: OffsetDateTime,

    /// The permissions of this user
    pub permissions: Vec<String>,
}

impl Server<'_> {
    /// Lists the users with permissions on this server
    pub async fn list_users(&self) -> crate::Result<Vec<User>> {
        self.client
            .request::<PteroList<User>>(Method::GET, &format!("servers/{}/users", self.id))
            .await
            .map(|users| users.data)
    }

    /// Adds a user with the given email to this server and assign them the given permissions
    pub async fn add_user(
        &self,
        email: impl Into<String>,
        permissions: Vec<String>,
    ) -> crate::Result<User> {
        #[derive(Serialize)]
        struct AddUserBody {
            email: String,
            permissions: Vec<String>,
        }
        self.client
            .request_with_body::<PteroObject<User>, _>(
                Method::POST,
                &format!("servers/{}/users", self.id),
                &AddUserBody {
                    email: email.into(),
                    permissions,
                },
            )
            .await
            .map(|user| user.attributes)
    }

    /// Gets the user with permissions with the given UUID
    pub async fn get_user(&self, id: Uuid) -> crate::Result<User> {
        self.client
            .request::<PteroObject<User>>(Method::GET, &format!("servers/{}/users/{}", self.id, id))
            .await
            .map(|user| user.attributes)
    }

    /// Sets the permissions for the user with the given ID
    pub async fn set_user_permissions(
        &self,
        id: Uuid,
        permissions: Vec<String>,
    ) -> crate::Result<User> {
        #[derive(Serialize)]
        struct SetUserPermissionsBody {
            permissions: Vec<String>,
        }
        self.client
            .request_with_body::<PteroObject<User>, _>(
                Method::POST,
                &format!("servers/{}/users/{}", self.id, id),
                &SetUserPermissionsBody { permissions },
            )
            .await
            .map(|user| user.attributes)
    }

    /// Removes a user from the server
    pub async fn delete_user(&self, id: Uuid) -> crate::Result<()> {
        self.client
            .request::<EmptyBody>(Method::DELETE, &format!("servers/{}/users/{}", self.id, id))
            .await?;
        Ok(())
    }
}
