//! API for endpoints under `api/client/server/{server}/network`

use crate::client::{ErrorResponse, Server};
use crate::http::{EmptyBody, ErrorHandler};
use crate::structs::{PteroList, PteroObject};
use reqwest::{Method, Response, StatusCode};
use serde::{Deserialize, Serialize};

/// A network allocation on the server
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct Allocation {
    /// The ID of this allocation
    pub id: u64,
    /// The IP of this allocation
    pub ip: String,
    /// The IP alias of this allocation
    pub ip_alias: Option<String>,
    /// The port of this allocation
    pub port: u16,
    /// Notes attached to this allocation
    pub notes: Option<String>,
    /// Whether this allocation is the default allocation on this server
    pub is_default: bool,
}

impl Server<'_> {
    /// Lists the network allocations on this server
    pub async fn list_network_allocations(&self) -> crate::Result<Vec<Allocation>> {
        self.client
            .request::<PteroList<Allocation>>(
                Method::GET,
                &format!("servers/{}/network/allocations", self.id),
            )
            .await
            .map(|allocations| allocations.data)
    }

    /// Automatically assigns a new network allocation if auto-assign is enabled on the instance
    pub async fn create_network_allocation(&self) -> crate::Result<Allocation> {
        self.client
            .request::<PteroObject<Allocation>>(
                Method::POST,
                &format!("servers/{}/network/allocations", self.id),
            )
            .await
            .map(|allocation| allocation.attributes)
    }

    /// Sets the notes of a network allocation
    pub async fn set_network_allocation_notes(
        &self,
        allocation_id: u64,
        notes: impl Into<String>,
    ) -> crate::Result<Allocation> {
        #[derive(Serialize)]
        struct SetNetworkAllocationNotesBody {
            notes: String,
        }
        self.client
            .request_with_body::<PteroObject<Allocation>, _>(
                Method::POST,
                &format!("servers/{}/network/allocations/{}", self.id, allocation_id),
                &SetNetworkAllocationNotesBody {
                    notes: notes.into(),
                },
            )
            .await
            .map(|allocation| allocation.attributes)
    }

    /// Sets the given network allocation to be the primary allocation for this server
    pub async fn set_network_allocation_primary(
        &self,
        allocation_id: u64,
    ) -> crate::Result<Allocation> {
        self.client
            .request::<PteroObject<Allocation>>(
                Method::POST,
                &format!(
                    "servers/{}/network/allocations/{}/primary",
                    self.id, allocation_id
                ),
            )
            .await
            .map(|allocation| allocation.attributes)
    }

    /// Deletes the given network allocation. Returns [`crate::Error::PrimaryAllocation`] if the
    /// given allocation is the primary allocation and therefore cannot be deleted
    pub async fn delete_network_allocation(&self, allocation_id: u64) -> crate::Result<()> {
        struct DeleteNetworkAllocationErrorHandler;
        impl ErrorHandler for DeleteNetworkAllocationErrorHandler {
            async fn get_error(response: Response) -> Option<crate::Error> {
                if response.status() != StatusCode::BAD_REQUEST {
                    return None;
                }
                let error: ErrorResponse = response.json().await.ok()?;
                if error.is_error("DisplayException") {
                    Some(crate::Error::PrimaryAllocation)
                } else {
                    None
                }
            }
        }
        self.client
            .request_with_error_handler::<EmptyBody, _, DeleteNetworkAllocationErrorHandler>(
                Method::DELETE,
                &format!("servers/{}/network/allocations/{}", self.id, allocation_id),
                EmptyBody,
            )
            .await?;
        Ok(())
    }
}
