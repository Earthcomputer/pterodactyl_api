//! API for endpoints under `api/client/servers/{server}/settings`

use crate::client::Server;
use crate::http::EmptyBody;
use reqwest::Method;
use serde::Serialize;

impl Server<'_> {
    /// Renames this server
    pub async fn rename(&self, name: impl Into<String>) -> crate::Result<()> {
        #[derive(Serialize)]
        struct RenameBody {
            name: String,
        }
        self.client
            .request_with_body::<EmptyBody, _>(
                Method::POST,
                &format!("servers/{}/settings/rename", self.id),
                &RenameBody { name: name.into() },
            )
            .await?;
        Ok(())
    }

    /// Reinstalls this server
    pub async fn reinstall(&self) -> crate::Result<()> {
        self.client
            .request::<EmptyBody>(
                Method::POST,
                &format!("servers/{}/settings/reinstall", self.id),
            )
            .await?;
        Ok(())
    }
}
