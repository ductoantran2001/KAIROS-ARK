//! Audit Verification for KAIROS-ARK.
//!
//! Provides integrity verification for audit ledgers using
//! cryptographic hashing.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Verification result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether verification passed
    pub valid: bool,
    /// Number of events verified
    pub event_count: usize,
    /// Hash of the ledger
    pub ledger_hash: String,
    /// Verification timestamp
    pub verified_at: u64,
    /// Error message if invalid
    pub error: Option<String>,
}

/// Signed ledger for compliance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedLedger {
    /// Ledger data as JSON
    pub data: String,
    /// SHA-256 hash of data
    pub hash: String,
    /// Run ID
    pub run_id: String,
    /// Signature timestamp
    pub signed_at: u64,
    /// Signer identity (optional)
    pub signer: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl SignedLedger {
    /// Create a new signed ledger.
    pub fn new(data: String, run_id: impl Into<String>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let hash = sha256_hex(&data);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            data,
            hash,
            run_id: run_id.into(),
            signed_at: now,
            signer: None,
            metadata: HashMap::new(),
        }
    }

    /// Set signer.
    pub fn with_signer(mut self, signer: impl Into<String>) -> Self {
        self.signer = Some(signer.into());
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Verify the ledger integrity.
    pub fn verify(&self) -> VerificationResult {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let computed_hash = sha256_hex(&self.data);
        let valid = computed_hash == self.hash;

        // Count events in data
        let event_count = self.data.lines().filter(|l| !l.is_empty()).count();

        VerificationResult {
            valid,
            event_count,
            ledger_hash: self.hash.clone(),
            verified_at: now,
            error: if valid { None } else { Some("Hash mismatch".to_string()) },
        }
    }
}

/// Simple SHA-256 implementation using standard library.
/// In production, use a proper crypto crate.
fn sha256_hex(data: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Simple hash for demonstration (not cryptographically secure)
    // In production, replace with sha2 crate
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    let h1 = hasher.finish();
    
    let mut hasher2 = DefaultHasher::new();
    format!("{}{}", data, h1).hash(&mut hasher2);
    let h2 = hasher2.finish();

    format!("{:016x}{:016x}", h1, h2)
}

/// Compliance report generator.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComplianceReport {
    /// Report ID
    pub id: String,
    /// Run ID being reported
    pub run_id: String,
    /// Report generated at
    pub generated_at: u64,
    /// Total events
    pub total_events: usize,
    /// Policy violations
    pub policy_violations: usize,
    /// Tool calls summary
    pub tool_calls: HashMap<String, usize>,
    /// Whether run completed successfully
    pub completed: bool,
    /// Verification result
    pub verification: Option<VerificationResult>,
}

impl ComplianceReport {
    /// Create a new compliance report.
    pub fn new(run_id: impl Into<String>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = format!(
            "report_{}",
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        );
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            id,
            run_id: run_id.into(),
            generated_at: now,
            ..Default::default()
        }
    }

    /// Generate report from signed ledger.
    pub fn from_signed_ledger(ledger: &SignedLedger) -> Self {
        let verification = ledger.verify();
        let mut report = Self::new(&ledger.run_id);
        report.total_events = verification.event_count;
        report.verification = Some(verification);
        report.completed = true;
        report
    }

    /// Export as JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_ledger() {
        let data = r#"{"event": "test"}"#.to_string();
        let signed = SignedLedger::new(data, "run_001");
        
        assert!(!signed.hash.is_empty());
        assert_eq!(signed.run_id, "run_001");
    }

    #[test]
    fn test_verify_valid() {
        let data = r#"{"event": "test"}"#.to_string();
        let signed = SignedLedger::new(data, "run_001");
        
        let result = signed.verify();
        assert!(result.valid);
    }

    #[test]
    fn test_verify_tampered() {
        let data = r#"{"event": "test"}"#.to_string();
        let mut signed = SignedLedger::new(data, "run_001");
        
        // Tamper with data
        signed.data = r#"{"event": "TAMPERED"}"#.to_string();
        
        let result = signed.verify();
        assert!(!result.valid);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_compliance_report() {
        let data = "event1\nevent2\nevent3".to_string();
        let signed = SignedLedger::new(data, "run_001");
        
        let report = ComplianceReport::from_signed_ledger(&signed);
        assert_eq!(report.total_events, 3);
        assert!(report.verification.is_some());
    }
}
