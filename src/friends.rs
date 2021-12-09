use actix_web::{get, post};
use actix_web::{web, HttpResponse};

// Functionality:
//   - send invite
//   - view invites
//   - respond to invite
//
//   - list online friends

//   - list friends
//   - remove friend
#[get("/{owner}/")]
async fn list_friends() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[get("/{owner}/online")]
async fn list_online_friends() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[get("/{owner}/unfriend/{username}")]
async fn unfriend() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[get("/{username}/invite/")]
async fn list_invites() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[post("/{recipient}/invite/")] // TODO: set the sender (not just the session)
async fn send_invite() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[post("/{recipient}/invite/{id}")]
async fn respond_to_invite() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_friends)
        .service(list_invites)
        .service(send_invite)
        .service(respond_to_invite);
}
