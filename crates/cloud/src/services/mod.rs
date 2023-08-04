pub(crate) mod hosts;
pub(crate) mod settings;

use actix_web::web;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/hosts").configure(hosts::config))
        .service(web::scope("/settings").configure(settings::config));
}
