//! oxitone-core — IDs, units, errors, pcg32-v1, canonical JSON, and the
//! wire types mirroring `@oxitone/protocol`. See
//! `.agents/docs/01-architecture.md` for ownership boundaries and
//! `.agents/docs/04-api-contracts.md` for the wire contract.

pub mod beat;
pub mod canonical;
pub mod error;
pub mod id;
pub mod pcg32;
pub mod version;
pub mod wire;

pub use beat::Beat;
pub use error::{codes, OxitoneError};
pub use pcg32::{hash64, Hash64Part, Pcg32, PCG32_INCREMENT, PCG32_MULTIPLIER};
pub use version::{check_protocol_version, PROTOCOL_VERSION};
