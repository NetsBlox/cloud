use actix_web::{web, App, HttpResponse, HttpRequest, HttpServer, middleware, cookie::Cookie};
use actix_web::{get, post, delete, patch};
use mongodb::Database;
use futures::stream::{TryStreamExt};
use mongodb::bson::doc;
use serde::{Serialize, Deserialize};
use mongodb::options::FindOptions;

#[post("/new")]
async fn new_project(db: web::Data<Database>) -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[post("/import")]  // TODO: should I consolidate w/ the previous one? Called "create" or something?
async fn import_project(db: web::Data<Database>) -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

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

#[get("/{projectID}/latest")]  // Include unsaved data
async fn get_latest_project() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[get("/{projectID}/{roleID}")]
async fn get_role() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[post("/{projectID}/{roleID}")]
async fn save_role() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[get("/{projectID}/{roleID}/latest")]
async fn get_latest_role() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(new_project);
}


