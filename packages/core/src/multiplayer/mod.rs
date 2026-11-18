//! Online sessions between launchers. The session logic is a server module the launcher loads
//! ([`client`]); what stays here is the data the pages render ([`types`]) and what this player's profile
//! plays ([`fingerprint`]).

pub mod client;
pub mod fingerprint;
pub mod types;
