use crate::client::{Client, RateLimits};
use reqwest::{Body, Method, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

impl Client {
    pub(crate) async fn request<Response: ResponseBody>(
        &self,
        method: Method,
        endpoint: &str,
    ) -> crate::Result<Response> {
        self.request_with_body::<Response, _>(method, endpoint, EmptyBody)
            .await
    }

    pub(crate) async fn request_with_body<Response: ResponseBody, Body: RequestBody>(
        &self,
        method: Method,
        endpoint: &str,
        body: Body,
    ) -> crate::Result<Response> {
        Response::decode(
            self.get_response::<_, NullErrorHandler>(method, endpoint, body)
                .await?,
        )
        .await
    }

    pub(crate) async fn request_with_error_handler<
        Response: ResponseBody,
        Body: RequestBody,
        EHandler: ErrorHandler,
    >(
        &self,
        method: Method,
        endpoint: &str,
        body: Body,
    ) -> crate::Result<Response> {
        Response::decode(
            self.get_response::<_, EHandler>(method, endpoint, body)
                .await?,
        )
        .await
    }

    pub(crate) async fn get_response<Body: RequestBody, EHandler: ErrorHandler>(
        &self,
        method: Method,
        endpoint: &str,
        body: Body,
    ) -> crate::Result<Response> {
        let request = self
            .client
            .request(method, format!("{}{}", self.url, endpoint))
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key));
        let request = body.encode(request)?;
        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            if let Some(err) = EHandler::get_error(response).await {
                return Err(err);
            }
            return Err(Self::translate_error(status));
        }

        if let Some(limit) = response
            .headers()
            .get("x-ratelimit-limit")
            .and_then(|header| header.to_str().ok())
            .and_then(|header| header.parse().ok())
        {
            if let Some(limit_remaining) = response
                .headers()
                .get("x-ratelimit-remaining")
                .and_then(|header| header.to_str().ok())
                .and_then(|header| header.parse().ok())
            {
                *self.rate_limits.write().unwrap() = Some(RateLimits {
                    limit,
                    limit_remaining,
                });
            }
        }

        Ok(response)
    }

    fn translate_error(status: StatusCode) -> crate::Error {
        match status {
            StatusCode::FORBIDDEN => crate::Error::PermissionError,
            StatusCode::NOT_FOUND => crate::Error::ResourceNotFound,
            StatusCode::TOO_MANY_REQUESTS => crate::Error::RateLimit,
            status => crate::Error::Http(status),
        }
    }

    #[cfg(test)]
    pub(crate) async fn dump_response<Body: RequestBody>(
        &self,
        method: Method,
        endpoint: &str,
        body: Body,
    ) -> crate::Result<String> {
        Ok(serde_json::to_string_pretty(
            &self
                .request_with_body::<serde_json::Value, Body>(method, endpoint, body)
                .await?,
        )?)
    }
}

pub(crate) trait RequestBody {
    fn encode(self, request: RequestBuilder) -> crate::Result<RequestBuilder>;
}

pub(crate) trait ResponseBody {
    async fn decode(response: Response) -> crate::Result<Self>
    where
        Self: Sized;
}

pub(crate) struct EmptyBody;
impl RequestBody for EmptyBody {
    fn encode(self, request: RequestBuilder) -> crate::Result<RequestBuilder> {
        Ok(request)
    }
}
impl ResponseBody for EmptyBody {
    async fn decode(_: Response) -> crate::Result<Self> {
        Ok(Self)
    }
}

impl<T: Serialize> RequestBody for &T {
    fn encode(self, request: RequestBuilder) -> crate::Result<RequestBuilder> {
        Ok(request.body(serde_json::to_string(self)?))
    }
}

impl<T: DeserializeOwned> ResponseBody for T {
    async fn decode(response: Response) -> crate::Result<Self> {
        let bytes = response.bytes().await?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

pub(crate) struct RawBody<T>(pub(crate) T);
impl<T> RequestBody for RawBody<T>
where
    T: Into<Body>,
{
    fn encode(self, request: RequestBuilder) -> crate::Result<RequestBuilder> {
        Ok(request.body(self.0))
    }
}

pub(crate) trait ErrorHandler {
    async fn get_error(response: Response) -> Option<crate::Error>;
}

pub(crate) struct NullErrorHandler;
impl ErrorHandler for NullErrorHandler {
    async fn get_error(_response: Response) -> Option<crate::Error> {
        None
    }
}
