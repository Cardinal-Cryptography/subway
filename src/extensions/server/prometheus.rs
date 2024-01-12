use futures::{future::BoxFuture, FutureExt};
use jsonrpsee::server::middleware::rpc::RpcServiceT;
use jsonrpsee::types::Request;
use jsonrpsee::MethodResponse;
use prometheus_endpoint::{register, Counter, Histogram, HistogramOpts, Registry, U64};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type MetricPair = (Counter<U64>, Histogram);

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
