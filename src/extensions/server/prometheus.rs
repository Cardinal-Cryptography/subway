use futures::{future::BoxFuture, FutureExt};
use jsonrpsee::server::middleware::rpc::RpcServiceT;
use jsonrpsee::types::Request;
use jsonrpsee::MethodResponse;
use prometheus_endpoint::{register, Counter, Histogram, HistogramOpts, Registry, U64};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type MetricPair = (Counter<U64>, Histogram);

#[derive(Clone)]
pub enum WsMetrics {
    Prometheus(InnerMetrics),
    Noop,
}

impl WsMetrics {
    pub fn new(registry: Option<&Registry>) -> Self {
        match registry {
            None => Self::Noop,
            Some(r) => Self::Prometheus(InnerMetrics::new(r)),
        }
    }

    pub fn ws_open(&self) {
        if let Self::Prometheus(inner) = self {
            inner.ws_open();
        }
    }

    pub fn ws_closed(&self) {
        if let Self::Prometheus(inner) = self {
            inner.ws_closed();
        }
    }
}

#[derive(Clone)]
pub struct InnerMetrics {
    open_session_count: Counter<U64>,
    closed_session_count: Counter<U64>,
}

impl InnerMetrics {
    fn new(registry: &Registry) -> Self {
        let open_counter = Counter::new("open_ws_counter", "No help").unwrap();
        let closed_counter = Counter::new("closed_ws_counter", "No help").unwrap();

        let open_session_count = register(open_counter, registry).unwrap();
        let closed_session_count = register(closed_counter, registry).unwrap();
        Self {
            open_session_count,
            closed_session_count,
        }
    }
    fn ws_open(&self) {
        self.open_session_count.inc();
    }

    fn ws_closed(&self) {
        self.closed_session_count.inc();
    }
}

#[derive(Clone)]
pub struct PrometheusService<S> {
    inner: S,
    registry: Registry,
    call_metrics: Arc<Mutex<HashMap<String, MetricPair>>>,
}

impl<S> PrometheusService<S> {
    pub fn new(inner: S, registry: &Registry, call_metrics: Arc<Mutex<HashMap<String, MetricPair>>>) -> Self {
        Self {
            inner,
            call_metrics,
            registry: registry.clone(),
        }
    }

    fn register_metrics_for(&self, method: String) -> MetricPair {
        let counter_name = format!("{}_count", method);
        let histogram_name = format!("{}_histogram", method);

        let counter = Counter::new(counter_name, "No help").unwrap();
        let histogram = Histogram::with_opts(HistogramOpts::new(histogram_name, "No help")).unwrap();

        let counter = register(counter, &self.registry).unwrap();
        let histogram = register(histogram, &self.registry).unwrap();

        (counter, histogram)
    }

    fn metrics_for(&self, method: String) -> MetricPair {
        let mut metrics = self.call_metrics.lock().unwrap();
        let (counter, histogram) = metrics
            .entry(method.clone())
            .or_insert_with(|| self.register_metrics_for(method));

        (counter.clone(), histogram.clone())
    }
}

impl<'a, S> RpcServiceT<'a> for PrometheusService<S>
where
    S: RpcServiceT<'a> + Send + Sync + Clone + 'static,
{
    type Future = BoxFuture<'a, MethodResponse>;

    fn call(&self, req: Request<'a>) -> Self::Future {
        let (counter, histogram) = self.metrics_for(req.method.to_string());

        let service = self.inner.clone();
        async move {
            counter.inc();

            let timer = histogram.start_timer();
            let res = service.call(req).await;
            timer.stop_and_record();

            res
        }
        .boxed()
    }
}
