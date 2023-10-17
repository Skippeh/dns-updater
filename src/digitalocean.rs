use reqwest::{header::HeaderMap, Client, ClientBuilder, Method, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Record {
    /// A unique identifier for each domain record.
    pub id: i32,
    /// The type of the DNS record. For example: A, CNAME, TXT, ...
    #[serde(rename = "type")]
    pub ty: String,
    /// The host name, alias, or service being defined by the record.
    pub name: String,
    /// Variable data depending on record type.
    /// For example, the "data" value for an A record would be the IPv4 address to which the domain will be mapped.
    /// For a CAA record, it would contain the domain name of the CA being granted permission to issue certificates.
    pub data: String,
    /// The priority for SRV and MX records.
    pub priority: Option<i32>,
    /// The port for SRV records.
    pub port: Option<i32>,
    /// This value is the time to live for the record, in seconds.
    /// This defines the time frame that clients can cache queried information before a refresh should be requested.
    pub ttl: i32,
    /// The weight for SRV records.
    pub weight: Option<i32>,
    /// An unsigned integer between 0-255 used for CAA records.
    pub flags: Option<i32>,
    /// The parameter tag for CAA records. Valid values are "issue", "issuewild", or "iodef"
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Domain {
    /// The name of the domain itself. This should follow the standard domain format of domain.TLD.
    /// For instance, example.com is a valid domain name.
    pub name: String,
    /// This value is the time to live for the records on this domain, in seconds.
    /// This defines the time frame that clients can cache queried information before a refresh should be requested.
    pub ttl: Option<i32>,
    /// This attribute contains the complete contents of the zone file for the selected domain.
    /// Individual domain record resources should be used to get more granular control over records.
    /// However, this attribute can also be used to get information about the SOA record,
    /// which is created automatically and is not accessible as an individual record resource.
    pub zone_file: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    _id: Option<String>,
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct ListDomainRecordsResponse {
    pub domain_records: Vec<Record>,
}

#[derive(Debug, Deserialize)]
pub struct ListAllDomainsResponse {
    pub domains: Vec<Domain>,
}

#[derive(Debug, Serialize)]
struct UpdateRecordRequestData<'a> {
    #[serde(rename = "type")]
    ty: &'a str,
    data: &'a str,
}

#[derive(Debug, Deserialize)]
struct UpdateRecordResponseData {
    pub domain_record: Record,
}

pub struct DigitalOcean {
    client: Client,
}

const API_BASE: &str = "https://api.digitalocean.com";

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("RateLimited: {0}")]
    RateLimited(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("NotFound: {0}")]
    NotFound(String),
    #[error("ServerError: {0}")]
    ServerError(String),
    #[error("Unexpected status code: {0}")]
    UnexpectedStatus(StatusCode),
    #[error("SerializationError: {0}")]
    ReqwestError(reqwest::Error),
}

impl From<reqwest::Error> for QueryError {
    fn from(err: reqwest::Error) -> Self {
        QueryError::ReqwestError(err)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NewClientError {
    #[error("ReqwestError: {0}")]
    ReqwestError(reqwest::Error),
    #[error("Invalid api key format")]
    InvalidApiKey,
}

impl From<reqwest::Error> for NewClientError {
    fn from(err: reqwest::Error) -> Self {
        NewClientError::ReqwestError(err)
    }
}

impl DigitalOcean {
    pub fn new(api_key: String) -> Result<Self, NewClientError> {
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            "Authorization",
            format!("Bearer {api_key}")
                .parse()
                .map_err(|_| NewClientError::InvalidApiKey)?,
        );
        default_headers.insert(
            "Content-Type",
            "application/json"
                .parse()
                .expect("application/json should always be a valid header value"),
        );

        Ok(DigitalOcean {
            client: ClientBuilder::new()
                .default_headers(default_headers)
                .build()
                .map_err(|err| NewClientError::ReqwestError(err))?,
        })
    }

    /// Note: Technically only queries the first 200 domains. Will fix if requested.
    pub async fn list_all_domains(&self) -> Result<Vec<Domain>, QueryError> {
        Ok(self
            .make_request::<ListAllDomainsResponse>("/v2/domains?per_page=200", Method::GET)
            .await?
            .domains)
    }

    pub async fn query_domain_records(&self, domain_name: &str) -> Result<Vec<Record>, QueryError> {
        Ok(self
            .make_request::<ListDomainRecordsResponse>(
                &format!("/v2/domains/{domain_name}/records"),
                Method::GET,
            )
            .await?
            .domain_records)
    }

    pub async fn update_record(
        &self,
        domain_name: &str,
        record_id: i32,
        new_type: &str,
        new_value: &str,
    ) -> Result<Record, QueryError> {
        let path = format!("/v2/domains/{domain_name}/records/{record_id}");

        Ok(self
            .make_request_with_data::<_, UpdateRecordResponseData>(
                &path,
                Method::PATCH,
                &UpdateRecordRequestData {
                    data: new_value,
                    ty: new_type,
                },
            )
            .await?
            .domain_record)
    }

    async fn make_request<ResponseData: DeserializeOwned>(
        &self,
        path: &str,
        method: Method,
    ) -> Result<ResponseData, QueryError> {
        let path = format!("{}{}", API_BASE, path);
        let builder = self.client.request(method, &path);

        let response = builder.send().await?;

        Self::handle_response(response).await
    }

    async fn make_request_with_data<RequestData: Serialize, ResponseData: DeserializeOwned>(
        &self,
        path: &str,
        method: Method,
        data: &RequestData,
    ) -> Result<ResponseData, QueryError> {
        let path = format!("{}{}", API_BASE, path);
        let builder = self.client.request(method, &path);
        let response = builder.json(data).send().await?;

        Self::handle_response(response).await
    }

    async fn handle_response<ResponseData: DeserializeOwned>(
        response: Response,
    ) -> Result<ResponseData, QueryError> {
        let status_code = response.status();

        match status_code {
            StatusCode::OK => Ok(response.json().await?),
            StatusCode::UNAUTHORIZED
            | StatusCode::NOT_FOUND
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR => {
                let err_data = response.json::<ErrorResponse>().await?;

                match status_code {
                    StatusCode::UNAUTHORIZED => Err(QueryError::Unauthorized(err_data.message)),
                    StatusCode::NOT_FOUND => Err(QueryError::NotFound(err_data.message)),
                    StatusCode::TOO_MANY_REQUESTS => Err(QueryError::RateLimited(err_data.message)),
                    StatusCode::INTERNAL_SERVER_ERROR => {
                        Err(QueryError::ServerError(err_data.message))
                    }
                    _ => unreachable!(),
                }
            }
            other => Err(QueryError::UnexpectedStatus(other)),
        }
    }
}
