use actix_web::{get, post};
use actix_web::{web, HttpResponse};
use serde::Deserialize;

#[get("/{recipient}/")]
async fn list_invites() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[post("/{recipient}/")]
async fn send_invite(invite: web::Json<CollaborateInvite>) -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

// #[derive(Deserialize)]
// enum CollaborateResponse {
//     ACCEPTED,
//     REJECTED,
// }

#[derive(Deserialize)]
struct CollaborateResponse {
    response: bool, // TODO: should this be an enum instead? PENDING, REJECTED, ACCEPTED?
}

#[post("/{recipient}/{id}")]
async fn respond_to_invite(
    response: web::Json<CollaborateResponse>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: ensure the project still exists?
    unimplemented!();
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_invites)
        .service(send_invite)
        .service(respond_to_invite);
}

#[cfg(test)]
mod tests {}
