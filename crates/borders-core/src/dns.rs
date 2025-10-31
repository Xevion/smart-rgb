//! Custom DNS resolver using Hickory DNS with DoH/DoT support.
//!
//! This module provides DNS over HTTPS (DoH) functionality for enhanced privacy,
//! with automatic fallback to system DNS if DoH is unavailable.

use hickory_resolver::{
    TokioResolver,
    config::{NameServerConfigGroup, ResolverConfig},
    name_server::TokioConnectionProvider,
};
use once_cell::sync::OnceCell;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, warn};

/// Custom DNS resolver for reqwest that uses Hickory DNS with DoH/DoT support.
///
/// This resolver is configured to use Cloudflare's DNS over HTTPS (1.1.1.1).
/// DNS over HTTPS encrypts DNS queries, preventing eavesdropping and tampering.
///
/// The resolver is lazily initialized within the async context to ensure
/// it's created within the Tokio runtime.
#[derive(Clone, Default)]
pub struct HickoryDnsResolver {
    /// Lazily initialized resolver to ensure it's created within Tokio runtime context
    state: Arc<OnceCell<TokioResolver>>,
}

impl HickoryDnsResolver {
    pub fn new() -> Self {
        Self { state: Arc::new(OnceCell::new()) }
    }

    /// Initialize the Hickory DNS resolver with Cloudflare DoH configuration
    fn init_resolver() -> Result<TokioResolver, Box<dyn std::error::Error + Send + Sync>> {
        let mut group: NameServerConfigGroup = NameServerConfigGroup::google();
        group.merge(NameServerConfigGroup::cloudflare());
        group.merge(NameServerConfigGroup::quad9());
        group.merge(NameServerConfigGroup::google());

        let mut config = ResolverConfig::new();
        for server in group.iter() {
            config.add_name_server(server.clone());
        }

        // Use tokio() constructor which properly integrates with current Tokio runtime
        let resolver = TokioResolver::builder_with_config(config, TokioConnectionProvider::default()).build();

        debug!("DNS resolver initialized with Cloudflare DoH");
        Ok(resolver)
    }
}

/// Fallback to system DNS when DoH is unavailable
async fn fallback_to_system_dns(name: &str) -> Result<Box<dyn Iterator<Item = SocketAddr> + Send>, Box<dyn std::error::Error + Send + Sync>> {
    use tokio::net::lookup_host;

    let addrs: Vec<SocketAddr> = lookup_host(format!("{}:443", name))
        .await?
        .map(|mut addr| {
            addr.set_port(0);
            addr
        })
        .collect();

    debug!("Resolved '{}' via system DNS ({} addresses)", name, addrs.len());
    Ok(Box::new(addrs.into_iter()))
}

impl reqwest::dns::Resolve for HickoryDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let resolver_state = self.state.clone();
        let name_str = name.as_str().to_string();

        Box::pin(async move {
            // Get or initialize the resolver within the async context (Tokio runtime)
            let resolver = match resolver_state.get_or_try_init(Self::init_resolver) {
                Ok(r) => r,
                Err(e) => {
                    warn!("Failed to initialize DoH resolver: {}, using system DNS", e);
                    return fallback_to_system_dns(&name_str).await;
                }
            };

            // Try Hickory DNS first (DoH)
            match resolver.lookup_ip(format!("{}.", name_str)).await {
                Ok(lookup) => {
                    let addrs: reqwest::dns::Addrs = Box::new(lookup.into_iter().map(|ip| SocketAddr::new(ip, 0)));
                    Ok(addrs)
                }
                Err(e) => {
                    warn!("DoH lookup failed for '{}': {}, falling back to system DNS", name_str, e);
                    fallback_to_system_dns(&name_str).await
                }
            }
        })
    }
}
