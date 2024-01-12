use super::{Extension, ExtensionRegistry};
use async_trait::async_trait;
use prometheus_endpoint::init_prometheus;
use prometheus_endpoint::Registry;
use serde::Deserialize;
use std::iter;
use std::net::{Ipv4Addr, SocketAddr};
use tokio::task::JoinHandle;

pub struct Prometheus {
    pub registry: Registry,
    pub exporter_task: JoinHandle<()>,
}

impl Drop for Prometheus {
    fn drop(&mut self) {
        self.exporter_task.abort();
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct PrometheusConfig {
    pub port: u16,
    pub prefix: Option<String>,
    pub chain_label: Option<String>,
}

#[async_trait]
impl Extension for Prometheus {
    type Config = PrometheusConfig;

    async fn from_config(config: &Self::Config, _registry: &ExtensionRegistry) -> Result<Self, anyhow::Error> {
        Ok(Self::new(config.clone()))
    }
}

impl Prometheus {
    pub fn new(config: PrometheusConfig) -> Self {
        let labels = config
            .chain_label
            .clone()
            .map(|l| iter::once(("chain".to_string(), l.clone())).collect());
        let prefix = match config.prefix {
            Some(p) if p.is_empty() => None,
            p => p,
        };
        let registry = Registry::new_custom(prefix, labels).expect("Can't happen");

        let exporter_task = start_prometheus_exporter(registry.clone(), config.port);
        Self {
            registry,
            exporter_task,
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

fn start_prometheus_exporter(registry: Registry, port: u16) -> JoinHandle<()> {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

    tokio::spawn(async move {
        init_prometheus(addr, registry).await.unwrap();
    })
}
