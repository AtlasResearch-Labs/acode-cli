//! Atlas Daedalus — Security & Permissions Module
//! (C) 2026 Atlas Research Labs — MIT License

#![allow(unused_imports)]

pub mod gate;
pub mod policy;
pub mod storage;
pub mod threat_shield;

pub use gate::PermissionGate;
pub use policy::{PermissionDecision, RiskLevel, SecurityMode, SecurityPolicy};
pub use storage::{SecurityConfig, SecurityStorage};
pub use threat_shield::{ThreatAlert, ThreatShield};
