use actix_web::{web, HttpResponse};
use actix_web::{get, post};

// Functionality:
//   - send invite
//   - view invites
//   - respond to invite
//   - list online friends

//   - list friends
//   - remove friend
#[get("/{owner}")]
async fn list_friends() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[get("/{username}/invite/")]
async fn list_invites() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[post("/{recipient}/invite/")]
async fn send_invite() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[post("/{recipient}/invite/{id}")]
async fn respond_to_invite() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(list_friends)
        .service(list_invites)
        .service(send_invite)
        .service(respond_to_invite);
}

