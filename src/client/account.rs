//! API for endpoints under `api/client/account`

use crate::client::{Client, ErrorResponse};
use crate::http::{EmptyBody, ErrorHandler};
use crate::structs::{PteroData, PteroList, PteroObject};
use reqwest::{Method, Response, StatusCode};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Contains information about your client account
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct Account {
    /// The account ID
    pub id: u64,
    /// Whether the account has admin
    pub admin: bool,
    /// The account username
    pub username: String,
    /// The account email
    pub email: String,
    /// The account first name
    pub first_name: String,
    /// The account last name
    pub last_name: String,
    /// The account language ("en" by default)
    pub language: String,
}

/// Account 2fa information
#[derive(Debug, Deserialize)]
pub struct Account2fa {
    /// The TOTP QR code image to allow the setup of 2FA
    pub image_url_data: String,
    /// The secret
    pub secret: String,
}

/// A list of 2fa recovery tokens
#[derive(Debug, Deserialize)]
pub struct RecoveryTokens {
    /// The tokens
    pub tokens: Vec<String>,
}

/// An API key to allow access to this account via the API
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct ApiKey {
    /// The ID for the API key
    pub identifier: String,
    /// The description for the API key
    pub description: String,
    /// The allowed IPs that can use this key. An empty list indicates that anyone can use the key.
    pub allowed_ips: Vec<String>,
    /// When the key was last used
    #[serde(deserialize_with = "crate::structs::optional_iso_time")]
    pub last_used_at: Option<OffsetDateTime>,
    /// When the key was created
    #[serde(deserialize_with = "crate::structs::iso_time")]
    pub created_at: OffsetDateTime,
}

/// An API key that has just been created, which includes the token used to login
#[derive(Debug)]
pub struct CreatedApiKey {
    /// Metadata about the API key
    pub key: ApiKey,
    /// The token used to login using the key
    pub secret_token: String,
}

impl Client {
    /// Gets the account details of the connected account
    pub async fn get_account_details(&self) -> crate::Result<Account> {
        self.request::<PteroObject<Account>>(Method::GET, "account")
            .await
            .map(|account| account.attributes)
    }

    /// Gets the 2fa details of the connected account
    pub async fn get_account_2fa_details(&self) -> crate::Result<Account2fa> {
        self.request::<PteroData<Account2fa>>(Method::GET, "account/two-factor")
            .await
            .map(|account_2fa| account_2fa.data)
    }

    /// Enables 2fa on the connected account using the given token. Returns
    /// [`crate::Error::Invalid2faToken`] if the token is invalid
    pub async fn enable_2fa(&self, token: impl Into<String>) -> crate::Result<RecoveryTokens> {
        #[derive(Serialize)]
        struct Enable2faBody {
            code: String,
        }
        struct Enable2faErrorHandler;
        impl ErrorHandler for Enable2faErrorHandler {
            async fn get_error(response: Response) -> Option<crate::Error> {
                if response.status() != StatusCode::BAD_REQUEST {
                    return None;
                }
                let error: ErrorResponse = response.json().await.ok()?;
                if !error.is_error("TwoFactorAuthenticationTokenInvalid") {
                    return None;
                }
                Some(crate::Error::Invalid2faToken)
            }
        }
        self.request_with_error_handler::<PteroObject<RecoveryTokens>, _, Enable2faErrorHandler>(
            Method::POST,
            "account/two-factor",
            &Enable2faBody { code: token.into() },
        )
        .await
        .map(|tokens| tokens.attributes)
    }

    /// Disables 2fa on the connected account. Returns [`crate::Error::IncorrectPassword`] if the
    /// password is incorrect
    pub async fn disable_2fa(&self, password: impl Into<String>) -> crate::Result<()> {
        #[derive(Serialize)]
        struct Disable2faBody {
            password: String,
        }
        struct Disable2faErrorHandler;
        impl ErrorHandler for Disable2faErrorHandler {
            async fn get_error(response: Response) -> Option<crate::Error> {
                if response.status() != StatusCode::BAD_REQUEST {
                    return None;
                }
                Some(crate::Error::IncorrectPassword)
            }
        }
        self.request_with_error_handler::<EmptyBody, _, Disable2faErrorHandler>(
            Method::DELETE,
            "account/two-factor",
            &Disable2faBody {
                password: password.into(),
            },
        )
        .await?;
        Ok(())
    }

    /// Updates the email for the connected account. Returns [`crate::Error::IncorrectPassword`] if
    /// the password is incorrect, and [`crate::Error::InvalidEmail`] if the email is invalid
    pub async fn update_email(
        &self,
        email: impl Into<String>,
        password: impl Into<String>,
    ) -> crate::Result<()> {
        #[derive(Serialize)]
        struct UpdateEmailBody {
            email: String,
            password: String,
        }
        struct UpdateEmailErrorHandler;
        impl ErrorHandler for UpdateEmailErrorHandler {
            async fn get_error(response: Response) -> Option<crate::Error> {
                if response.status() != StatusCode::BAD_REQUEST {
                    return None;
                }
                let error: ErrorResponse = response.json().await.ok()?;
                if error.is_error("email") {
                    Some(crate::Error::InvalidEmail)
                } else if error.is_error("InvalidPasswordProvidedException") {
                    Some(crate::Error::IncorrectPassword)
                } else {
                    None
                }
            }
        }
        self.request_with_error_handler::<EmptyBody, _, UpdateEmailErrorHandler>(
            Method::PUT,
            "account/email",
            &UpdateEmailBody {
                email: email.into(),
                password: password.into(),
            },
        )
        .await?;
        Ok(())
    }

    /// Updates the password for the connected account. Returns [`crate::Error::IncorrectPassword`]
    /// if the existing password is incorrect
    pub async fn update_password(
        &self,
        current_password: impl Into<String>,
        new_password: impl Into<String>,
    ) -> crate::Result<()> {
        #[derive(Serialize)]
        struct UpdatePasswordBody {
            current_password: String,
            password: String,
            password_confirmation: String,
        }
        struct UpdatePasswordErrorHandler;
        impl ErrorHandler for UpdatePasswordErrorHandler {
            async fn get_error(response: Response) -> Option<crate::Error> {
                if response.status() != StatusCode::BAD_REQUEST {
                    return None;
                }
                Some(crate::Error::IncorrectPassword)
            }
        }
        let new_password = new_password.into();
        self.request_with_error_handler::<EmptyBody, _, UpdatePasswordErrorHandler>(
            Method::PUT,
            "account/password",
            &UpdatePasswordBody {
                current_password: current_password.into(),
                password: new_password.clone(),
                password_confirmation: new_password,
            },
        )
        .await?;
        Ok(())
    }

    /// Gets the list of metadata for API keys that can be used to connect with this account
    pub async fn get_api_keys(&self) -> crate::Result<Vec<ApiKey>> {
        self.request::<PteroList<ApiKey>>(Method::GET, "account/api-keys")
            .await
            .map(|keys| keys.data)
    }

    /// Creates a new API key that can be used to connect with this account. Returns the token
    /// needed to connect, and metadata about the created key
    pub async fn create_api_key(
        &self,
        description: impl Into<String>,
    ) -> crate::Result<CreatedApiKey> {
        self.create_api_key_with_optional_allowed_ips(description, None)
            .await
    }

    /// Creates a new API key that can be used to connect with this account. Only the IPs listed
    /// will be allowed to connect using this key, unless the list is empty in which case anyone can
    /// connect. Returns the token needed to connect, and metadata about the created key
    pub async fn create_api_key_with_allowed_ips(
        &self,
        description: impl Into<String>,
        allowed_ips: Vec<String>,
    ) -> crate::Result<CreatedApiKey> {
        self.create_api_key_with_optional_allowed_ips(description, Some(allowed_ips))
            .await
    }

    async fn create_api_key_with_optional_allowed_ips(
        &self,
        description: impl Into<String>,
        allowed_ips: Option<Vec<String>>,
    ) -> crate::Result<CreatedApiKey> {
        #[derive(Deserialize)]
        struct Meta {
            secret_token: String,
        }
        #[derive(Deserialize)]
        struct CreatedApiKeyObj {
            attributes: ApiKey,
            meta: Meta,
        }
        #[derive(Serialize)]
        struct CreateApiKeyBody {
            description: String,
            allowed_ips: Option<Vec<String>>,
        }
        self.request_with_body::<CreatedApiKeyObj, _>(
            Method::POST,
            "account/api-keys",
            &CreateApiKeyBody {
                description: description.into(),
                allowed_ips,
            },
        )
        .await
        .map(|obj| CreatedApiKey {
            key: obj.attributes,
            secret_token: obj.meta.secret_token,
        })
    }

    /// Deletes an API key that can be used to connect to this account
    pub async fn delete_api_key(&self, id: impl Into<String>) -> crate::Result<()> {
        self.request::<EmptyBody>(Method::DELETE, &format!("account/api-keys/{}", id.into()))
            .await?;
        Ok(())
    }
}
