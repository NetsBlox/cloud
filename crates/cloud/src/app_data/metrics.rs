use std::net::IpAddr;

use actix_web_prom::{PrometheusMetrics, PrometheusMetricsBuilder};
use prometheus::{opts, IntCounterVec, IntGauge};

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
    signups: IntCounterVec,
    clients: IntGauge,
}

impl Metrics {
    pub(crate) fn new() -> Self {
        let prometheus = PrometheusMetricsBuilder::new("metrics")
            .endpoint("/metrics")
            .build()
            .unwrap();

        let login_opts = opts!("logins", "logins").namespace("metrics");
        let logins = IntCounterVec::new(login_opts, &["username"]).unwrap();

        let signup_opts = opts!("signups", "signups").namespace("metrics");
        let signups = IntCounterVec::new(signup_opts, &["username"]).unwrap();

        let clients = IntGauge::new("clients", "Connected clients").unwrap();
        Self {
            prometheus,

            logins,
            signups,
            clients,
        }
    }

    pub(crate) fn handler(&self) -> PrometheusMetrics {
        self.prometheus.clone()
    }

    // TODO: record failed login attempts?
    pub(crate) fn record_login(&self, username: &str) {
        self.logins.with_label_values(&[username]).inc();
    }

    pub(crate) fn record_signup(&self, username: &str) {
        self.signups.with_label_values(&[username]).inc();
    }

    pub(crate) fn record_connected_clients(&self, count: usize) {
        //self.clients.set(count);
        todo!()
    }

    pub(crate) fn record_msg_sent(&self, sender: &str, address: &str) {
        todo!();
        // TODO
    }
}
