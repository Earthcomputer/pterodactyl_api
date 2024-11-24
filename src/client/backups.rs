//! API for endpoints under `api/client/servers/{server}/backups`

use crate::client::Server;
use crate::http::EmptyBody;
use crate::structs::{PteroList, PteroObject};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// A backup created on a server
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct Backup {
    /// The backup ID
    pub uuid: Uuid,
    /// The backup name
    pub name: String,
    /// The files that were ignored while creating this backup
    pub ignored_files: Vec<String>,
    /// The checksum of the files in the backup
    pub checksum: Option<String>,
    /// The size of this backup in bytes
    pub bytes: u64,
    /// When this backup was created
    #[serde(deserialize_with = "crate::structs::iso_time")]
    pub created_at: OffsetDateTime,
    /// When this backup was completed
    #[serde(deserialize_with = "crate::structs::optional_iso_time")]
    pub completed_at: Option<OffsetDateTime>,
    /// Whether the backup is locked
    pub is_locked: bool,
}

/// The parameters to create a backup
#[derive(Debug, Default, Serialize, PartialEq, Eq, Hash, Clone)]
pub struct BackupParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "core::ops::Not::not")]
    is_locked: bool,
}

impl BackupParams {
    /// Creates the default backup parameters
    pub fn new() -> Self {
        BackupParams::default()
    }

    /// Sets the name of the backup
    pub fn with_name(self, name: impl Into<String>) -> Self {
        BackupParams {
            name: Some(name.into()),
            ..self
        }
    }

    /// Makes the backup locked. Please note that this requires extra permissions
    pub fn set_locked(self) -> Self {
        BackupParams {
            is_locked: true,
            ..self
        }
    }
}

impl<T> From<T> for BackupParams
where
    T: Into<String>,
{
    fn from(value: T) -> Self {
        BackupParams::new().with_name(value)
    }
}

impl From<Backup> for BackupParams {
    fn from(value: Backup) -> Self {
        BackupParams {
            name: Some(value.name),
            is_locked: value.is_locked,
        }
    }
}

impl Server<'_> {
    /// Gets the list of backups for this server
    pub async fn list_backups(&self) -> crate::Result<Vec<Backup>> {
        self.client
            .request::<PteroList<Backup>>(Method::GET, &format!("servers/{}/backups", self.id))
            .await
            .map(|backups| backups.data)
    }

    /// Creates a backup with the default parameters
    pub async fn create_backup(&self) -> crate::Result<Backup> {
        self.create_backup_with_params(BackupParams::new()).await
    }

    /// Creates a backup with the given parameters
    ///
    /// ```
    /// # use pterodactyl_api::client::backups::BackupParams;
    /// # use pterodactyl_api::client::Server;
    /// # let server: Server = todo!();
    /// # async {
    /// server.create_backup_with_params("Test Backup").await?;
    /// server.create_backup_with_params(BackupParams::new().with_name("Test Backup 2").set_locked()).await?;
    /// # }
    /// ```
    pub async fn create_backup_with_params(
        &self,
        options: impl Into<BackupParams>,
    ) -> crate::Result<Backup> {
        self.client
            .request_with_body::<PteroObject<Backup>, _>(
                Method::POST,
                &format!("servers/{}/backups", self.id),
                &options.into(),
            )
            .await
            .map(|backup| backup.attributes)
    }

    /// Gets the backup with the given ID
    pub async fn get_backup(&self, id: Uuid) -> crate::Result<Backup> {
        self.client
            .request::<PteroObject<Backup>>(
                Method::GET,
                &format!("servers/{}/backups/{}", self.id, id),
            )
            .await
            .map(|backup| backup.attributes)
    }

    /// Gets a one-time use download link for a backup
    pub async fn get_backup_download_link(&self, id: Uuid) -> crate::Result<String> {
        #[derive(Deserialize)]
        struct Url {
            url: String,
        }
        self.client
            .request::<PteroObject<Url>>(
                Method::GET,
                &format!("servers/{}/backups/{}/download", self.id, id),
            )
            .await
            .map(|url| url.attributes.url)
    }

    /// Deletes the backup with the given ID
    pub async fn delete_backup(&self, id: Uuid) -> crate::Result<()> {
        self.client
            .request::<EmptyBody>(
                Method::DELETE,
                &format!("servers/{}/backups/{}", self.id, id),
            )
            .await?;
        Ok(())
    }
}
