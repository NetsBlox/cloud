use crate::app_data::AppData;
use actix_web::{delete, get, patch, post};
use actix_web::{web, HttpResponse};
use futures::stream::TryStreamExt;
use mongodb::bson::{doc, DateTime};
use serde::{Deserialize, Serialize};

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

#[derive(Deserialize, Serialize)]
struct ProjectMetadata {
    id: String,
    name: String,
    updated: DateTime,
    thumbnail: String,
    public: bool,
}

#[get("/user/{owner}")]
async fn list_user_projects(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, std::io::Error> {
    let collection = app.collection::<ProjectMetadata>("projects");
    let (username,) = path.into_inner();
    let query = doc! {"owner": username};
    let mut cursor = collection
        .find(query, None)
        .await
        .expect("Could not retrieve projects");

    let mut projects = Vec::new();
    while let Some(project) = cursor.try_next().await.expect("Could not fetch project") {
        // TODO: should I stream this back?
        projects.push(project);
    }
    Ok(HttpResponse::Ok().json(projects))
}

#[get("/shared/{username}")]
async fn list_shared_projects(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, std::io::Error> {
    let collection = app.collection::<ProjectMetadata>("projects");
    let (username,) = path.into_inner();
    let query = doc! {"collaborators": username}; // FIXME
    let mut cursor = collection
        .find(query, None)
        .await
        .expect("Could not retrieve projects");

    let mut projects = Vec::new();
    while let Some(project) = cursor.try_next().await.expect("Could not fetch project") {
        // TODO: should I stream this back?
        projects.push(project);
    }
    Ok(HttpResponse::Ok().json(projects))
}

#[get("/user/{owner}/{name}")]
async fn get_project_named(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, std::io::Error> {
    let (owner, name) = path.into_inner();
    // TODO: Should this include metadata?
    todo!();
}

#[get("/id/{projectID}")]
async fn get_project() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[delete("/id/{projectID}")]
async fn delete_project() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[patch("/id/{projectID}")]
async fn update_project() -> Result<HttpResponse, std::io::Error> {
    todo!(); // TODO: rename, etc
}

#[get("/id/{projectID}/latest")] // Include unsaved data
async fn get_latest_project() -> Result<HttpResponse, std::io::Error> {
    todo!(); // TODO: return xml string
             //Ok(HttpResponse::Ok().body(serialized_project))
}

#[get("/id/{projectID}/thumbnail")]
async fn get_project_thumbnail() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[derive(Deserialize)]
struct CreateRoleData {
    name: String,
    data: Option<String>,
}

#[post("/id/{projectID}/")]
async fn create_role(role_data: web::Json<CreateRoleData>) -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message? I am not sure
    // TODO: this shouldn't need to. It should trigger an update sent
    todo!();
}

#[get("/id/{projectID}/{roleID}")]
async fn get_role() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[delete("/id/{projectID}/{roleID}")]
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
#[post("/id/{projectID}/{roleID}")]
async fn save_role() -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message?
    todo!();
}

#[derive(Deserialize)]
struct RenameRoleData {
    name: String,
}

#[patch("/id/{projectID}/{roleID}")]
async fn rename_role(role_data: web::Json<RenameRoleData>) -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message?
    todo!();
}

#[get("/id/{projectID}/{roleID}/latest")]
async fn get_latest_role() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[get("/id/{projectID}/collaborators/")]
async fn list_collaborators() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[derive(Deserialize)]
struct AddCollaboratorBody {
    username: String,
}
#[post("/id/{projectID}/collaborators/")]
async fn add_collaborator(
    body: web::Json<AddCollaboratorBody>,
) -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[delete("/id/{projectID}/collaborators/{username}")]
async fn remove_collaborator() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[get("/id/{projectID}/occupants/")]
async fn list_occupants() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[delete("/id/{projectID}/occupants/{clientID}")]
async fn remove_occupant() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OccupantInvite {
    username: String,
    role_id: String,
}

#[post("/id/{projectID}/occupants/invite")] // TODO: add role ID
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
