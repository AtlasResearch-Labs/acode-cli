//! Atlas Daedalus — Security & Permissions Module
//! (C) 2026 Atlas Research Labs — MIT License

#![allow(unused_imports)]

pub mod policy;
pub mod storage;
pub mod gate;
pub mod threat_shield;

pub use policy::{PermissionDecision, RiskLevel, SecurityMode, SecurityPolicy};
pub use gate::PermissionGate;
pub use storage::{SecurityConfig, SecurityStorage};
pub use threat_shield::{ThreatShield, ThreatAlert};
