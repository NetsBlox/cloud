use actix_web::{web, HttpResponse};
use actix_web::{get, post, delete, patch};
use mongodb::Database;

#[post("/")]
async fn create_project(db: web::Data<Database>) -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

//#[post("/import")]  // TODO: should I consolidate w/ the previous one? Called "create" or something? (or just post /)
//async fn import_project(db: web::Data<Database>) -> Result<HttpResponse, std::io::Error> {
    //unimplemented!();
//}

#[get("/list/{owner}")]
async fn list_user_projects() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[get("/list/{owner}/shared")]
async fn list_shared_projects() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[get("/{projectID}")]
async fn get_project() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[delete("/{projectID}")]
async fn delete_project() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[patch("/{projectID}")]
async fn update_project() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();  // TODO: rename, etc
}

#[get("/{projectID}/latest")]  // Include unsaved data
async fn get_latest_project() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[get("/{projectID}/thumbnail")]
async fn get_project_thumbnail() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[post("/{projectID}/")]
async fn create_role() -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message?
    unimplemented!();
}

#[get("/{projectID}/{roleID}")]
async fn get_role() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[delete("/{projectID}/{roleID}")]
async fn delete_role() -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message?
    unimplemented!();
}

#[post("/{projectID}/{roleID}")]
async fn save_role() -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message?
    unimplemented!();
}

#[patch("/{projectID}/{roleID}")]
async fn rename_role() -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message?
    unimplemented!();
}

#[get("/{projectID}/{roleID}/latest")]
async fn get_latest_role() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

// TODO: add project management endpoints?
// - invite collaborator
// - rescind invitation
// - remove collaborator

// TODO: add (open) project management endpoints?
// - invite occupant
// - evict user

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(create_project);
}


