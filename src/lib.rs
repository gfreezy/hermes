#![allow(clippy::unreadable_literal)]

pub mod dns;

pub use crate::dns::context::{ResolveStrategy, ServerContext};
pub use crate::dns::protocol::{DnsRecord, TransientTtl};
pub use crate::dns::server::DnsUdpServer;
