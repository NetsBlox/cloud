use actix_web_prom::PrometheusMetricsBuilder;

/// Metrics are used to record various metrics on the server. These include:
///  - logins (username, program?)
///  - signups (username)
///  - active users (program)
///  - messages sent (sender, receiver)
///  - general API endpoint usage?

struct Metrics {}

impl Metrics {
    pub(crate) fn new() {
        let prometheus = PrometheusMetricsBuilder::new("metrics")
            .endpoint("/metrics")
            .build()
            .unwrap();
        //.wrap(prometheus.clone())
    }
    // record
}
