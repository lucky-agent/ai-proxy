use serde::{Deserialize, Serialize};

/// A key-value pair representing an HTTP header.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeaderPair {
    pub key: String,
    pub value: String,
}

/// A collection of API requests organized in a tree structure.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiCollection {
    pub id: String,
    pub name: String,
    pub children: Vec<ApiTreeNode>,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
    #[serde(rename = "updatedAt")]
    pub updated_at: u64,
}

/// A node in the API collection tree — either a folder or a request.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ApiTreeNode {
    #[serde(rename = "folder")]
    Folder {
        id: String,
        name: String,
        children: Vec<ApiTreeNode>,
    },
    #[serde(rename = "request")]
    Request {
        id: String,
        name: String,
        method: String,
        url: String,
        headers: Vec<HeaderPair>,
        params: Vec<HeaderPair>,
        cookies: Vec<HeaderPair>,
        #[serde(rename = "bodyType")]
        body_type: String,
        body: String,
        #[serde(rename = "authType")]
        auth_type: Option<String>,
        #[serde(rename = "authData")]
        auth_data: Option<String>,
    },
}
