//! Human-in-the-Loop (HITL) Approval Gateway.
//!
//! Provides pause-for-approval nodes that suspend execution
//! until an external signal is received.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Approval request status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    /// Waiting for approval
    Pending,
    /// Request approved
    Approved,
    /// Request rejected
    Rejected,
    /// Request timed out
    Timeout,
    /// Request cancelled
    Cancelled,
}

impl ApprovalStatus {
    /// Check if terminal state.
    pub fn is_terminal(&self) -> bool {
        !matches!(self, Self::Pending)
    }

    /// Get as string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Approval request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique request ID
    pub id: String,
    /// Associated run ID
    pub run_id: String,
    /// Node that requested approval
    pub node_id: String,
    /// Human-readable reason for approval
    pub reason: String,
    /// Timestamp when created (ms since epoch)
    pub created_at: u64,
    /// Timeout in milliseconds (None = no timeout)
    pub timeout_ms: Option<u64>,
    /// Current status
    pub status: ApprovalStatus,
    /// Response message (if rejected)
    pub response_message: Option<String>,
    /// Timestamp when resolved
    pub resolved_at: Option<u64>,
    /// Who approved/rejected (for audit)
    pub resolved_by: Option<String>,
}

impl ApprovalRequest {
    /// Create a new approval request.
    pub fn new(
        run_id: impl Into<String>,
        node_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let id = format!("approval_{}", COUNTER.fetch_add(1, Ordering::SeqCst));
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            id,
            run_id: run_id.into(),
            node_id: node_id.into(),
            reason: reason.into(),
            created_at: now,
            timeout_ms: None,
            status: ApprovalStatus::Pending,
            response_message: None,
            resolved_at: None,
            resolved_by: None,
        }
    }

    /// Set timeout.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Check if request has timed out.
    pub fn is_timed_out(&self) -> bool {
        if let Some(timeout) = self.timeout_ms {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            now > self.created_at + timeout
        } else {
            false
        }
    }

    /// Time remaining until timeout (if set).
    pub fn time_remaining_ms(&self) -> Option<u64> {
        self.timeout_ms.map(|timeout| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let deadline = self.created_at + timeout;
            deadline.saturating_sub(now)
        })
    }
}

/// Human-in-the-Loop Approval Gateway.
///
/// Manages approval requests and provides signaling for suspended runs.
pub struct ApprovalGateway {
    /// Pending and resolved requests
    requests: RwLock<HashMap<String, ApprovalRequest>>,
}

impl ApprovalGateway {
    /// Create a new approval gateway.
    pub fn new() -> Self {
        Self {
            requests: RwLock::new(HashMap::new()),
        }
    }

    /// Submit a new approval request.
    pub fn request_approval(&self, request: ApprovalRequest) -> String {
        let id = request.id.clone();
        self.requests.write().insert(id.clone(), request);
        id
    }

    /// Approve a request.
    pub fn approve(&self, request_id: &str, approver: Option<&str>) -> bool {
        let mut requests = self.requests.write();
        if let Some(req) = requests.get_mut(request_id) {
            if req.status == ApprovalStatus::Pending {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                req.status = ApprovalStatus::Approved;
                req.resolved_at = Some(now);
                req.resolved_by = approver.map(String::from);
                return true;
            }
        }
        false
    }

    /// Reject a request.
    pub fn reject(&self, request_id: &str, reason: &str, rejector: Option<&str>) -> bool {
        let mut requests = self.requests.write();
        if let Some(req) = requests.get_mut(request_id) {
            if req.status == ApprovalStatus::Pending {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                req.status = ApprovalStatus::Rejected;
                req.response_message = Some(reason.to_string());
                req.resolved_at = Some(now);
                req.resolved_by = rejector.map(String::from);
                return true;
            }
        }
        false
    }

    /// Cancel a request.
    pub fn cancel(&self, request_id: &str) -> bool {
        let mut requests = self.requests.write();
        if let Some(req) = requests.get_mut(request_id) {
            if req.status == ApprovalStatus::Pending {
                req.status = ApprovalStatus::Cancelled;
                return true;
            }
        }
        false
    }

    /// Check request status.
    pub fn check_status(&self, request_id: &str) -> Option<ApprovalStatus> {
        let mut requests = self.requests.write();
        if let Some(req) = requests.get_mut(request_id) {
            // Check for timeout
            if req.status == ApprovalStatus::Pending && req.is_timed_out() {
                req.status = ApprovalStatus::Timeout;
            }
            return Some(req.status);
        }
        None
    }

    /// Get request details.
    pub fn get_request(&self, request_id: &str) -> Option<ApprovalRequest> {
        self.requests.read().get(request_id).cloned()
    }

    /// List all pending requests.
    pub fn list_pending(&self) -> Vec<ApprovalRequest> {
        self.requests
            .read()
            .values()
            .filter(|r| r.status == ApprovalStatus::Pending)
            .cloned()
            .collect()
    }

    /// List requests for a specific run.
    pub fn list_for_run(&self, run_id: &str) -> Vec<ApprovalRequest> {
        self.requests
            .read()
            .values()
            .filter(|r| r.run_id == run_id)
            .cloned()
            .collect()
    }

    /// Clear all requests.
    pub fn clear(&self) {
        self.requests.write().clear();
    }

    /// Get request count.
    pub fn count(&self) -> usize {
        self.requests.read().len()
    }
}

impl Default for ApprovalGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ApprovalGateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApprovalGateway")
            .field("requests", &self.count())
            .finish()
    }
}

/// Global approval gateway singleton.
static GLOBAL_GATEWAY: std::sync::OnceLock<ApprovalGateway> = std::sync::OnceLock::new();

/// Get the global approval gateway.
pub fn global_gateway() -> &'static ApprovalGateway {
    GLOBAL_GATEWAY.get_or_init(ApprovalGateway::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_request() {
        let req = ApprovalRequest::new("run_1", "node_a", "Please approve this action");
        assert_eq!(req.status, ApprovalStatus::Pending);
        assert!(req.id.starts_with("approval_"));
    }

    #[test]
    fn test_approve() {
        let gateway = ApprovalGateway::new();
        let req = ApprovalRequest::new("run_1", "node_a", "Approve?");
        let id = gateway.request_approval(req);

        assert!(gateway.approve(&id, Some("admin")));
        assert_eq!(gateway.check_status(&id), Some(ApprovalStatus::Approved));
    }

    #[test]
    fn test_reject() {
        let gateway = ApprovalGateway::new();
        let req = ApprovalRequest::new("run_1", "node_a", "Approve?");
        let id = gateway.request_approval(req);

        assert!(gateway.reject(&id, "Not authorized", Some("admin")));
        assert_eq!(gateway.check_status(&id), Some(ApprovalStatus::Rejected));
    }

    #[test]
    fn test_list_pending() {
        let gateway = ApprovalGateway::new();
        gateway.request_approval(ApprovalRequest::new("run_1", "n1", "r1"));
        gateway.request_approval(ApprovalRequest::new("run_1", "n2", "r2"));

        assert_eq!(gateway.list_pending().len(), 2);
    }
}
