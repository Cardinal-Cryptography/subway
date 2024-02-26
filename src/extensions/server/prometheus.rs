use futures::{future::BoxFuture, FutureExt};
use jsonrpsee::server::middleware::rpc::RpcServiceT;
use jsonrpsee::types::Request;
use jsonrpsee::MethodResponse;
use prometheus_endpoint::{register, Counter, CounterVec, HistogramOpts, HistogramVec, Opts, Registry, U64};

use std::fmt::Display;

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

#[derive(Clone, Copy)]
pub enum Protocol {
    Ws,
    Http,
}

impl Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::Ws => "ws".to_string(),
            Self::Http => "http".to_string(),
        };
        write!(f, "{}", str)
    }
}

#[derive(Clone)]
pub struct PrometheusService<S> {
    inner: S,
    protocol: Protocol,
    call_times: HistogramVec,
    calls_started: CounterVec<U64>,
    calls_finished: CounterVec<U64>,
}

impl<S> PrometheusService<S> {
    pub fn new(inner: S, registry: &Registry, protocol: Protocol) -> Self {
        let call_times =
            HistogramVec::new(HistogramOpts::new("rpc_calls_time", "No help"), &["protocol", "method"]).unwrap();
        let calls_started_counter =
            CounterVec::new(Opts::new("rpc_calls_started", "No help"), &["protocol", "method"]).unwrap();
        let calls_finished_counter = CounterVec::new(
            Opts::new("rpc_calls_finished", "No help"),
            &["protocol", "method", "is_error"],
        )
        .unwrap();

        let call_times = register(call_times, registry).unwrap();
        let calls_started = register(calls_started_counter, registry).unwrap();
        let calls_finished = register(calls_finished_counter, registry).unwrap();

        Self {
            inner,
            protocol,
            calls_started,
            calls_finished,
            call_times,
        }
    }
}

impl<'a, S> RpcServiceT<'a> for PrometheusService<S>
where
    S: RpcServiceT<'a> + Send + Sync + Clone + 'static,
{
    type Future = BoxFuture<'a, MethodResponse>;

    fn call(&self, req: Request<'a>) -> Self::Future {
        let protocol = self.protocol.to_string();
        let method = req.method.to_string();

        let histogram = self.call_times.with_label_values(&[&protocol, &method]);
        let started = self.calls_started.with_label_values(&[&protocol, &method]);
        let finished = self.calls_finished.clone();

        let service = self.inner.clone();
        async move {
            started.inc();

            let timer = histogram.start_timer();
            let res = service.call(req).await;
            timer.stop_and_record();
            finished
                .with_label_values(&[&protocol, &method, &res.is_error().to_string()])
                .inc();

            res
        }
        .boxed()
    }
}
