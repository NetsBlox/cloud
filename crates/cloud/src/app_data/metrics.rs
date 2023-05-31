use actix_web_prom::{PrometheusMetrics, PrometheusMetricsBuilder};
use prometheus::{opts, IntCounter, IntCounterVec, IntGauge};

/// This is used to record various server metrics for use with prometheus. Metrics include:
///  - logins (username, program?)
///  - signups (username)
///  - active users (program)
///  - messages sent (sender, receiver)
///  - general API endpoint usage?

#[derive(Clone)]
pub(crate) struct Metrics {
    prometheus: PrometheusMetrics,
    logins: IntCounter,
    signups: IntCounter,
    clients: IntGauge,
    sent_messages: IntCounter,
}

impl Metrics {
    pub(crate) fn new() -> Self {
        let prometheus = PrometheusMetricsBuilder::new("metrics")
            .endpoint("/metrics")
            .build()
            .unwrap();

        let logins = IntCounter::new("netsblox_logins", "NetsBlox logins").unwrap();
        prometheus
            .registry
            .register(Box::new(logins.clone()))
            .unwrap();

        let signups = IntCounter::new("netsblox_signups", "New account creation count").unwrap();
        prometheus
            .registry
            .register(Box::new(signups.clone()))
            .unwrap();

        let clients = IntGauge::new("netsblox_clients", "Connected clients").unwrap();
        prometheus
            .registry
            .register(Box::new(clients.clone()))
            .unwrap();

        let sent_messages =
            IntCounter::new("netsblox_sent_messages", "NetsBlox messages sent").unwrap();
        prometheus
            .registry
            .register(Box::new(sent_messages.clone()))
            .unwrap();

        Self {
            prometheus,

            logins,
            signups,

            clients,
            sent_messages,
        }
    }

    pub(crate) fn handler(&self) -> PrometheusMetrics {
        self.prometheus.clone()
    }

    pub(crate) fn record_login(&self) {
        self.logins.inc();
    }

    pub(crate) fn record_signup(&self) {
        self.signups.inc();
    }

    pub(crate) fn record_connected_clients(&self, count: usize) {
        self.clients.set(count as i64);
    }

    pub(crate) fn record_msg_sent(&self) {
        self.sent_messages.inc();
    }
}
