use actix_web::{web, HttpResponse};
use actix_web::{get, post};

// Functionality:
//   - 
#[post("/{client}/state")]
async fn set_client_state() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(set_client_state);
}

