//! client for sending DNS queries to other servers

use async_std::net::UdpSocket;
use std::io::{Error, ErrorKind, Result};
use std::marker::{Send, Sync};
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use crate::dns::buffer::BytePacketBuffer;
use crate::dns::protocol::{DnsPacket, DnsQuestion, QueryType};

#[async_trait]
pub trait DnsClient {
    fn get_sent_count(&self) -> usize;
    fn get_failed_count(&self) -> usize;

    async fn send_query(
        &self,
        qname: &str,
        qtype: QueryType,
        server: (&str, u16),
        recursive: bool,
    ) -> Result<DnsPacket>;
}

/// The UDP client
///
/// This includes a fair bit of synchronization due to the stateless nature of UDP.
/// When many queries are sent in parallell, the response packets can come back
/// in any order. For that reason, we fire off replies on the sending thread, but
/// handle replies on a single thread. A channel is created for every response,
/// and the caller will block on the channel until the a response is received.
pub struct DnsNetworkClient {
    total_sent: AtomicUsize,
    total_failed: AtomicUsize,

    /// Counter for assigning packet ids
    seq: AtomicUsize,

    /// The listener socket
    socket: UdpSocket,
}

unsafe impl Send for DnsNetworkClient {}
unsafe impl Sync for DnsNetworkClient {}

impl DnsNetworkClient {
    pub async fn new(port: u16) -> DnsNetworkClient {
        DnsNetworkClient {
            total_sent: AtomicUsize::new(0),
            total_failed: AtomicUsize::new(0),
            seq: AtomicUsize::new(0),
            socket: UdpSocket::bind(("0.0.0.0", port)).await.unwrap(),
        }
    }

    /// Send a DNS query using UDP transport
    ///
    /// This will construct a query packet, and fire it off to the specified server.
    /// The query is sent from the callee thread, but responses are read on a
    /// worker thread, and returned to this thread through a channel. Thus this
    /// method is thread safe, and can be used from any number of threads in
    /// parallell.
    pub async fn send_udp_query(
        &self,
        qname: &str,
        qtype: QueryType,
        server: (&str, u16),
        recursive: bool,
    ) -> Result<DnsPacket> {
        let _ = self.total_sent.fetch_add(1, Ordering::Release);

        // Prepare request
        let mut packet = DnsPacket::new();

        packet.header.id = self.seq.fetch_add(1, Ordering::SeqCst) as u16;
        if packet.header.id + 1 == 0xFFFF {
            self.seq.compare_and_swap(0xFFFF, 0, Ordering::SeqCst);
        }

        packet.header.questions = 1;
        packet.header.recursion_desired = recursive;

        packet
            .questions
            .push(DnsQuestion::new(qname.to_string(), qtype));

        // Send query
        let mut req_buffer = BytePacketBuffer::new();
        packet.write(&mut req_buffer, 512)?;
        self.socket
            .send_to(&req_buffer.buf[0..req_buffer.pos], server)
            .await?;
        // Read data into a buffer
        let mut res_buffer = BytePacketBuffer::new();
        let (size, _src) = self.socket.recv_from(&mut res_buffer.buf).await?;
        assert!(res_buffer.buf.len() > size);

        // Construct a DnsPacket from buffer, skipping the packet if parsing
        // failed
        // Wait for response
        if let Ok(res) = DnsPacket::from_buffer(&mut res_buffer) {
            Ok(res)
        } else {
            // Otherwise, fail
            let _ = self.total_failed.fetch_add(1, Ordering::Release);
            Err(Error::new(ErrorKind::InvalidInput, "Lookup failed"))
        }
    }
}

#[async_trait]
impl DnsClient for DnsNetworkClient {
    fn get_sent_count(&self) -> usize {
        self.total_sent.load(Ordering::Acquire)
    }

    fn get_failed_count(&self) -> usize {
        self.total_failed.load(Ordering::Acquire)
    }

    async fn send_query(
        &self,
        qname: &str,
        qtype: QueryType,
        server: (&str, u16),
        recursive: bool,
    ) -> Result<DnsPacket> {
        let packet = self.send_udp_query(qname, qtype, server, recursive).await?;
        if !packet.header.truncated_message {
            return Ok(packet);
        }

        Err(Error::new(ErrorKind::UnexpectedEof, "truncated message"))
    }
}

#[cfg(test)]
pub mod tests {

    use async_std::task::block_on;
    use std::io::Result;

    use super::*;
    use crate::dns::protocol::{DnsPacket, DnsRecord, QueryType};

    pub type StubCallback = dyn Fn(&str, QueryType, (&str, u16), bool) -> Result<DnsPacket>;

    pub struct DnsStubClient {
        callback: Box<StubCallback>,
    }

    impl<'a> DnsStubClient {
        pub fn new(callback: Box<StubCallback>) -> DnsStubClient {
            DnsStubClient { callback }
        }
    }

    unsafe impl Send for DnsStubClient {}
    unsafe impl Sync for DnsStubClient {}

    #[async_trait]
    impl DnsClient for DnsStubClient {
        fn get_sent_count(&self) -> usize {
            0
        }

        fn get_failed_count(&self) -> usize {
            0
        }

        async fn send_query(
            &self,
            qname: &str,
            qtype: QueryType,
            server: (&str, u16),
            recursive: bool,
        ) -> Result<DnsPacket> {
            (self.callback)(qname, qtype, server, recursive)
        }
    }

    #[test]
    pub fn test_udp_client() {
        block_on(async {
            let client = DnsNetworkClient::new(31456).await;

            let res = client
                .send_udp_query("google.com", QueryType::A, ("8.8.8.8", 53), true)
                .await
                .unwrap();

            assert_eq!(res.questions[0].name, "google.com");
            assert!(res.answers.len() > 0);

            match res.answers[0] {
                DnsRecord::A { ref domain, .. } => {
                    assert_eq!("google.com", domain);
                }
                _ => panic!(),
            }
        });
    }
}
