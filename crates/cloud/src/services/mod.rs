mod hosts;
mod settings;

use actix_web::web;
pub use hosts::ensure_is_authorized_host;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/hosts").configure(hosts::config))
        .service(web::scope("/settings").configure(settings::config));
}
