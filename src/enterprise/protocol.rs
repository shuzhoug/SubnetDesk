use serde::{Deserialize, Serialize};

pub const LAN_CAP_ENTERPRISE_AUTH_V1: u64 = 1 << 0;
pub const LAN_CAP_ENTERPRISE_AUTH_REQUESTED: u64 = 1 << 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnterpriseAuthChallenge {
    pub protocol_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnterpriseAuthRequest {
    pub protocol_version: u32,
    pub remote_grant: String,
    pub requested_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnterpriseAuthResult {
    pub protocol_version: u32,
    pub accepted: bool,
    pub authorization_id: Option<String>,
    pub granted_capabilities: Vec<String>,
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enterprise_capability_bits_do_not_overlap() {
        assert_ne!(LAN_CAP_ENTERPRISE_AUTH_V1, LAN_CAP_ENTERPRISE_AUTH_REQUESTED);
        assert_eq!(LAN_CAP_ENTERPRISE_AUTH_V1 & LAN_CAP_ENTERPRISE_AUTH_REQUESTED, 0);
    }

    #[test]
    fn enterprise_auth_wire_types_round_trip_json() {
        let request = EnterpriseAuthRequest {
            protocol_version: 1,
            remote_grant: "header.payload.signature".to_owned(),
            requested_capabilities: vec!["remote_control".to_owned(), "clipboard".to_owned()],
        };
        let encoded = serde_json::to_string(&request).unwrap();
        let decoded: EnterpriseAuthRequest = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, request);
    }
}
