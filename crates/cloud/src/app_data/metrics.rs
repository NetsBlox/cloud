use std::net::IpAddr;

use actix_web_prom::{PrometheusMetrics, PrometheusMetricsBuilder};
use prometheus::{opts, IntCounterVec};

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
}

impl Metrics {
    pub(crate) fn new() -> Self {
        let prometheus = PrometheusMetricsBuilder::new("metrics")
            .endpoint("/metrics")
            .build()
            .unwrap();

        let login_opts = opts!("logins", "NetsBlox logins").namespace("metrics");
        let logins = IntCounterVec::new(login_opts, &["username"]).unwrap();
        Self { prometheus, logins }
    }

    pub(crate) fn handler(&self) -> PrometheusMetrics {
        self.prometheus.clone()
    }

    // TODO: record failed login attempts?
    pub(crate) fn record_login(&self, username: &str, ip: Option<IpAddr>) {
        let addr = ip.map(|addr| addr.to_string()).unwrap_or_else(|| "".into());
        self.logins.with_label_values(&[username, &addr]).inc(); // FIXME: can I leave ip blank if unknown?
    }

    pub(crate) fn record_signup(&self, username: &str) {
        todo!();
        // TODO
    }

    pub(crate) fn record_active_users(&self, count: u32) {
        todo!();
        // TODO
    }

    pub(crate) fn record_msg_sent(&self, sender: &str, address: &str) {
        todo!();
        // TODO
    }
}
