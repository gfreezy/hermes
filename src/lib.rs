#![allow(clippy::unreadable_literal)]

pub mod dns;

pub use dns::context::{ResolveStrategy, ServerContext};
pub use dns::protocol::{DnsRecord, TransientTtl};
pub use dns::server::DnsUdpServer;
