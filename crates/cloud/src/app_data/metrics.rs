use actix_web_prom::{PrometheusMetrics, PrometheusMetricsBuilder};
use prometheus::{opts, IntCounter, IntCounterVec, IntGauge};

/// Metrics are used to record various metrics on the server. These include:
///  - logins (username, program?)
///  - signups (username)
///  - active users (program)
///  - messages sent (sender, receiver)
///  - general API endpoint usage?

#[derive(Clone)]
pub(crate) struct Metrics {
    prometheus: PrometheusMetrics,
    logins: IntCounterVec,
    signups: IntCounter,
    clients: IntGauge,
    sent_messages: IntCounterVec,
}

impl Metrics {
    pub(crate) fn new() -> Self {
        let prometheus = PrometheusMetricsBuilder::new("metrics")
            .endpoint("/metrics")
            .build()
            .unwrap();

        let login_opts = opts!("netsblox_logins", "logins");
        let logins = IntCounterVec::new(login_opts, &["username"]).unwrap();
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

        let sent_message_opts = opts!("netsblox_sent_messages", "NetsBlox messages sent (by user)");
        let sent_messages = IntCounterVec::new(sent_message_opts, &["sender"]).unwrap();
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

    // TODO: record failed login attempts?
    pub(crate) fn record_login(&self, username: &str) {
        self.logins.with_label_values(&[username]).inc();
    }

    pub(crate) fn record_signup(&self) {
        self.signups.inc();
    }

    pub(crate) fn record_connected_clients(&self, count: usize) {
        self.clients.set(count as i64);
    }

    pub(crate) fn record_msg_sent(&self, sender: Option<&str>) {
        let sender_lbl = sender.unwrap_or("_guest");
        self.sent_messages.with_label_values(&[sender_lbl]).inc();
    }
}
