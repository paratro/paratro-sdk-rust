use serde::{Deserialize, Serialize};

use crate::client::MpcClient;
use crate::error::Error;

/// A security factor entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFactorItem {
    pub id: String,
    pub factor_type: String,
    pub chain: String,
    pub address: String,
    pub label: String,
    pub status: String,
    pub reason: String,
    pub added_by: String,
    pub created_at: String,
}

/// Response for listing security factor entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSecurityFactorResponse {
    pub items: Vec<SecurityFactorItem>,
    pub total: i64,
}

/// Request to add a security factor entry.
#[derive(Debug, Clone, Serialize)]
pub struct AddSecurityFactorRequest {
    pub factor_type: String,
    pub chain: String,
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub mfa_code: String,
}

/// Request to delete a security factor entry.
#[derive(Debug, Clone, Serialize)]
pub struct DeleteSecurityFactorRequest {
    pub mfa_code: String,
}

/// Request to set the status of a security factor entry.
#[derive(Debug, Clone, Serialize)]
pub struct SetSecurityFactorStatusRequest {
    pub status: String,
    pub mfa_code: String,
}

impl MpcClient {
    /// Lists security factor entries, optionally filtered by chain.
    pub async fn list_security_factors(
        &self,
        chain: Option<&str>,
    ) -> Result<ListSecurityFactorResponse, Error> {
        let mut params = Vec::new();
        if let Some(c) = chain {
            params.push(("chain".to_string(), c.to_string()));
        }
        if params.is_empty() {
            self.get("/v1/client/security-factors").await
        } else {
            self.get_with_query("/v1/client/security-factors", &params)
                .await
        }
    }

    /// Adds a new security factor entry.
    pub async fn add_security_factor(
        &self,
        req: &AddSecurityFactorRequest,
    ) -> Result<SecurityFactorItem, Error> {
        self.post("/v1/client/security-factors", req).await
    }

    /// Deletes a security factor entry by ID.
    pub async fn delete_security_factor(
        &self,
        factor_id: &str,
        req: &DeleteSecurityFactorRequest,
    ) -> Result<(), Error> {
        let path = format!("/v1/client/security-factors/{}", factor_id);
        self.delete_with_body(&path, req).await
    }

    /// Sets the status of a security factor entry.
    pub async fn set_security_factor_status(
        &self,
        factor_id: &str,
        req: &SetSecurityFactorStatusRequest,
    ) -> Result<SecurityFactorItem, Error> {
        let path = format!("/v1/client/security-factors/{}/status", factor_id);
        self.put(&path, req).await
    }
}
