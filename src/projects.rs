use crate::app_data::AppData;
use actix_web::{delete, get, patch, post};
use actix_web::{web, HttpResponse};
use serde::Deserialize;

#[post("/")]
async fn create_project(app: web::Data<AppData>) -> Result<HttpResponse, std::io::Error> {
    todo!();
    // TODO: add allow_rename query string parameter?
    // TODO: return the project name/id, role name/id
}

//#[post("/import")]  // TODO: should I consolidate w/ the previous one? Called "create" or something? (or just post /)
//async fn import_project(db: web::Data<AppData>) -> Result<HttpResponse, std::io::Error> {
//todo!();
//}

#[get("/named/{owner}/{name}")] // TODO: better name
async fn get_project_named() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[get("/{projectID}")]
async fn get_project() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[delete("/{projectID}")]
async fn delete_project() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[patch("/{projectID}")]
async fn update_project() -> Result<HttpResponse, std::io::Error> {
    todo!(); // TODO: rename, etc
}

#[get("/{projectID}/latest")] // Include unsaved data
async fn get_latest_project() -> Result<HttpResponse, std::io::Error> {
    todo!(); // TODO: return xml string
             //Ok(HttpResponse::Ok().body(serialized_project))
}

#[get("/{projectID}/thumbnail")]
async fn get_project_thumbnail() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[derive(Deserialize)]
struct CreateRoleData {
    name: String,
    data: Option<String>,
}

#[post("/{projectID}/")]
async fn create_role(role_data: web::Json<CreateRoleData>) -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message? I am not sure
    // TODO: this shouldn't need to. It should trigger an update sent
    todo!();
}

#[get("/{projectID}/{roleID}")]
async fn get_role() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[delete("/{projectID}/{roleID}")]
async fn delete_role() -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message?
    todo!();
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RoleData {
    room_name: String,
    project_name: String,
    source_code: String,
    media: String,
    source_size: u32,
    media_size: u32,
}
#[post("/{projectID}/{roleID}")]
async fn save_role() -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message?
    todo!();
}

#[derive(Deserialize)]
struct RenameRoleData {
    name: String,
}

#[patch("/{projectID}/{roleID}")]
async fn rename_role(role_data: web::Json<RenameRoleData>) -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message?
    todo!();
}

#[get("/{projectID}/{roleID}/latest")]
async fn get_latest_role() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[get("/{projectID}/collaborators/")]
async fn list_collaborators() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[derive(Deserialize)]
struct AddCollaboratorBody {
    username: String,
}
#[post("/{projectID}/collaborators/")]
async fn add_collaborator(
    body: web::Json<AddCollaboratorBody>,
) -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[delete("/{projectID}/collaborators/{username}")]
async fn remove_collaborator() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[get("/{projectID}/occupants/")]
async fn list_occupants() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[delete("/{projectID}/occupants/{clientID}")]
async fn remove_occupant() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OccupantInvite {
    username: String,
    role_id: String,
}

#[post("/{projectID}/occupants/invite")] // TODO: add role ID
async fn invite_occupant(
    invite: web::Json<OccupantInvite>,
) -> Result<HttpResponse, std::io::Error> {
    todo!();
}

// TODO: add project management endpoints?
// - invite collaborator
// - rescind invitation
// - remove collaborator

// TODO: add (open) project management endpoints?
// - invite occupant
// - evict user

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_project)
        .service(add_collaborator)
        .service(remove_collaborator)
        .service(list_occupants)
        .service(invite_occupant);
}
