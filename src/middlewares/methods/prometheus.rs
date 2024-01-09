use crate::config::RpcMethod;
use crate::middlewares::{CallRequest, CallResult, Middleware, MiddlewareBuilder, NextFn};
use crate::utils::{TypeRegistry, TypeRegistryRef};
use async_trait::async_trait;
use prometheus_endpoint::{register, Counter, Histogram, HistogramOpts, Registry, U64};

pub struct PrometheusMiddleware {
    call_counter: Counter<U64>,
    call_exec_time: Histogram,
}

impl PrometheusMiddleware {
    fn new(registry: &Registry, method: &RpcMethod) -> Self {
        let counter_name = format!("{}_count", method.method);
        let histogram_name = format!("{}_histogram", method.method);

        let counter = Counter::new(counter_name, "No help").unwrap();
        let histogram = Histogram::with_opts(HistogramOpts::new(histogram_name, "No help")).unwrap();

        let counter = register(counter, registry).unwrap();
        let histogram = register(histogram, registry).unwrap();

        Self {
            call_counter: counter,
            call_exec_time: histogram,
        }
    }
}

#[async_trait]
impl MiddlewareBuilder<RpcMethod, CallRequest, CallResult> for PrometheusMiddleware {
    async fn build(
        method: &RpcMethod,
        extensions: &TypeRegistryRef,
    ) -> Option<Box<dyn Middleware<CallRequest, CallResult>>> {
        let prometheus = extensions
            .read()
            .await
            .get::<crate::extensions::prometheus::Prometheus>()
            .expect("Prometheus extension not found");
        let registry = prometheus.registry();

        Some(Box::new(PrometheusMiddleware::new(registry, method)))
    }
}

#[async_trait]
impl Middleware<CallRequest, CallResult> for PrometheusMiddleware {
    async fn call(
        &self,
        request: CallRequest,
        context: TypeRegistry,
        next: NextFn<CallRequest, CallResult>,
    ) -> CallResult {
        self.call_counter.inc();

        let timer = self.call_exec_time.start_timer();
        let result = next(request, context).await;
        timer.stop_and_record();

        result
    }
}
