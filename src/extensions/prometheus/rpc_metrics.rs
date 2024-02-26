use prometheus_endpoint::{register, Counter, CounterVec, Opts, Registry, U64};

#[derive(Clone)]
pub enum RpcMetrics {
    Prometheus(InnerMetrics),
    Noop,
}

impl RpcMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self::Prometheus(InnerMetrics::new(registry))
    }

    pub fn noop() -> Self {
        Self::Noop
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

    pub fn cache_query(&self, method: &str) {
        if let Self::Prometheus(inner) = self {
            inner.cache_query(method);
        }
    }
    pub fn cache_miss(&self, method: &str) {
        if let Self::Prometheus(inner) = self {
            inner.cache_miss(method);
        }
    }
}

#[derive(Clone)]
pub struct InnerMetrics {
    open_session_count: Counter<U64>,
    closed_session_count: Counter<U64>,
    cache_query_counter: CounterVec<U64>,
    cache_miss_counter: CounterVec<U64>,
}

impl InnerMetrics {
    fn new(registry: &Registry) -> Self {
        let open_counter = Counter::new("open_ws_counter", "No help").unwrap();
        let closed_counter = Counter::new("closed_ws_counter", "No help").unwrap();
        let cache_miss_counter = CounterVec::new(Opts::new("cache_miss_counter", "No help"), &["method"]).unwrap();
        let cache_query_counter = CounterVec::new(Opts::new("cache_query_counter", "No help"), &["method"]).unwrap();

        let open_session_count = register(open_counter, registry).unwrap();
        let closed_session_count = register(closed_counter, registry).unwrap();
        let cache_query_counter = register(cache_query_counter, registry).unwrap();
        let cache_miss_counter = register(cache_miss_counter, registry).unwrap();

        Self {
            cache_miss_counter,
            cache_query_counter,
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

    fn cache_query(&self, method: &str) {
        self.cache_query_counter.with_label_values(&[method]).inc();
    }

    fn cache_miss(&self, method: &str) {
        self.cache_miss_counter.with_label_values(&[method]).inc();
    }
}
