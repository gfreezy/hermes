//! The `ServerContext in this thread holds the common state across the server

use std::io::Result;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::dns::authority::Authority;
use crate::dns::cache::SynchronizedCache;
use crate::dns::client::{DnsClient, DnsNetworkClient};
use crate::dns::resolve::{DnsResolver, ForwardingDnsResolver, RecursiveDnsResolver};

pub struct ServerStatistics {
    pub tcp_query_count: AtomicUsize,
    pub udp_query_count: AtomicUsize,
}

impl ServerStatistics {
    pub fn get_tcp_query_count(&self) -> usize {
        self.tcp_query_count.load(Ordering::Acquire)
    }

    pub fn get_udp_query_count(&self) -> usize {
        self.udp_query_count.load(Ordering::Acquire)
    }
}

pub enum ResolveStrategy {
    Recursive,
    Forward { host: String, port: u16 },
}

pub struct ServerContext {
    pub authority: Authority,
    pub cache: SynchronizedCache,
    pub client: Box<dyn DnsClient + Sync + Send>,
    pub dns_port: u16,
    pub resolve_strategy: ResolveStrategy,
    pub allow_recursive: bool,
    pub statistics: ServerStatistics,
}
//
//impl Default for ServerContext {
//    fn default() -> Self {
//        ServerContext::new()
//    }
//}

impl ServerContext {
    pub async fn new() -> ServerContext {
        ServerContext {
            authority: Authority::new(),
            cache: SynchronizedCache::new(),
            client: Box::new(DnsNetworkClient::new(34255).await),
            dns_port: 53,
            resolve_strategy: ResolveStrategy::Recursive,
            allow_recursive: true,
            statistics: ServerStatistics {
                tcp_query_count: AtomicUsize::new(0),
                udp_query_count: AtomicUsize::new(0),
            },
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        // Load authority data
        self.authority.load().await?;

        Ok(())
    }

    pub fn create_resolver(&self, ptr: Arc<ServerContext>) -> Box<dyn DnsResolver> {
        match self.resolve_strategy {
            ResolveStrategy::Recursive => Box::new(RecursiveDnsResolver::new(ptr)),
            ResolveStrategy::Forward { ref host, port } => {
                Box::new(ForwardingDnsResolver::new(ptr, (host.clone(), port)))
            }
        }
    }
}

#[cfg(test)]
pub mod tests {

    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    use crate::dns::authority::Authority;
    use crate::dns::cache::SynchronizedCache;

    use crate::dns::client::tests::{DnsStubClient, StubCallback};

    use super::*;

    pub fn create_test_context(callback: Box<StubCallback>) -> Arc<ServerContext> {
        Arc::new(ServerContext {
            authority: Authority::new(),
            cache: SynchronizedCache::new(),
            client: Box::new(DnsStubClient::new(callback)),
            dns_port: 53,
            resolve_strategy: ResolveStrategy::Recursive,
            allow_recursive: true,
            enable_udp: true,
            statistics: ServerStatistics {
                tcp_query_count: AtomicUsize::new(0),
                udp_query_count: AtomicUsize::new(0),
            },
        })
    }
}
