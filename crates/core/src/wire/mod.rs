//! Wire types mirroring `@oxitone/protocol` zod schemas
//! (04-api-contracts.md). Unknown JSON fields are ignored on decode
//! (serde default), matching the TypeScript strip behavior. Sync is enforced
//! by the fixture round-trip tests in `tests/fixtures.rs`.

mod authoring;
mod automation;
mod basic;
mod commands;
mod plugin;
pub use plugin::*;
mod serde_util;
mod snapshot;

pub use authoring::*;
pub use automation::*;
pub use basic::*;
pub use commands::*;
pub use snapshot::*;

/// Opaque entity identifier; structural checks live in `crate::id`.
pub type EntityId = String;
