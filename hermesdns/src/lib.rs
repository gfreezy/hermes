#![allow(clippy::unreadable_literal)]

mod dns;

pub use dns::client::{DnsClient, DnsNetworkClient};
pub use dns::context::{ResolveStrategy, ServerContext};
pub use dns::protocol::{DnsRecord, TransientTtl};
pub use dns::resolve::{DnsResolver, ForwardingDnsResolver, RecursiveDnsResolver};
pub use dns::server::DnsUdpServer;
