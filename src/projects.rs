use crate::app_data::AppData;
use actix_web::{delete, get, patch, post};
use actix_web::{web, HttpResponse};
use futures::stream::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

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

#[derive(Deserialize, Serialize)]
struct ProjectEntry {
    _id: ObjectId,
    owner: String,
    name: String,
    updated: DateTime,
    thumbnail: String,
    public: bool,
    collaborators: std::vec::Vec<String>,
    origin_time: DateTime, // FIXME: set the case
    roles: HashMap<String, RoleData>,
    // TODO: add the rest of the fields
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
struct RoleData {
    project_name: String,
    source_code: String,
    media: String,
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
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: Should this include metadata?
    // TODO: authenticate!
    let (owner, name) = path.into_inner();
    let collection = app.collection::<ProjectEntry>("projects");
    let query = doc! {"owner": owner, "name": name};
    match collection.find_one(query, None).await.unwrap() {
        Some(project) => {
            //project.source_code; // TODO: fetch this using the blob client
            // TODO: serialize the project
            Ok(HttpResponse::Ok().json(project))
        }
        None => Ok(HttpResponse::NotFound().body("Project not found")),
    }
}

#[get("/id/{projectID}")]
async fn get_project() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[delete("/id/{projectID}")]
async fn delete_project(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: authenticate! is admin or owner (is group owner?))
    let collection = app.collection::<ProjectMetadata>("projects");
    let (project_id,) = path.into_inner();
    match ObjectId::parse_str(project_id) {
        Ok(id) => {
            let query = doc! {"_id": id}; // FIXME
            let result = collection
                .delete_one(query, None)
                .await
                .expect("Could not delete project");

            if result.deleted_count > 0 {
                Ok(HttpResponse::Ok().body("Project deleted"))
            } else {
                Ok(HttpResponse::NotFound().body("Project not found"))
            }
        }
        Err(_) => Ok(HttpResponse::NotFound().body("Project not found")),
    }
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
async fn get_project_thumbnail(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, std::io::Error> {
    let collection = app.collection::<ProjectMetadata>("projects");
    let (project_id,) = path.into_inner();
    match ObjectId::parse_str(project_id) {
        Ok(id) => {
            let query = doc! {"_id": id};
            let result = collection
                .find_one(query, None)
                .await
                .expect("Could not delete project");

            if let Some(metadata) = result {
                Ok(HttpResponse::Ok().body(metadata.thumbnail))
            } else {
                Ok(HttpResponse::NotFound().body("Project not found"))
            }
        }
        Err(_) => Ok(HttpResponse::NotFound().body("Project not found")),
    }
}

#[derive(Deserialize)]
struct CreateRoleData {
    name: String,
    data: Option<String>,
}

#[post("/id/{projectID}/")]
async fn create_role(
    app: web::Data<AppData>,
    role_data: web::Json<CreateRoleData>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message? I am not sure
    // TODO: this shouldn't need to. It should trigger an update sent
    let (project_id,) = path.into_inner();
    match ObjectId::parse_str(project_id) {
        Ok(project_id) => {
            let collection = app.collection::<ProjectEntry>("projects");
            let query = doc! {"_id": project_id};
            let role_id = Uuid::new_v4();
            // FIXME: This isn't right...
            let role = RoleData {
                project_name: role_data.name,
                // TODO: store this using the blob
                source_code: role_data.data.unwrap_or("".to_owned()), // TODO: what about media?
                media: "".to_owned(),
            };
            let update = doc! {format!("roles.{}", role_id): role};
            match collection
                .find_one_and_update(query, update, None)
                .await
                .unwrap()
            {
                Some(project) => {
                    let role_names = project
                        .roles
                        .into_values()
                        .map(|r| r.project_name)
                        .collect::<HashSet<String>>();

                    if role_names.contains(&role_data.name) {
                        let mut base_name = role_data.name;
                        let mut role_name = base_name.clone();
                        let number: u32 = 2;
                        while role_names.contains(&role_name) {
                            role_name = format!("{} ({})", base_name, number);
                            number += 1;
                        }
                        let query = doc! {"_id": project_id};
                        let update =
                            doc! {"$set": {format!("roles.{}.ProjectName", role_id): role_name}};
                        collection.update_one(query, update, None).await.unwrap();
                    }
                    Ok(HttpResponse::Ok().body("Role created"))
                }
                None => Ok(HttpResponse::NotFound().body("Project not found")),
            }
        }
        Err(_err) => Ok(HttpResponse::NotFound().body("Project not found")),
    }
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
async fn rename_role(
    app: web::Data<AppData>,
    role_data: web::Json<RenameRoleData>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id, role_id) = path.into_inner();
    match ObjectId::parse_str(project_id) {
        Ok(project_id) => {
            let query = doc! {"_id": project_id};
            let update = doc! {"$set": {format!("roles.{}.ProjectName", role_id): &role_data.name}};
            let collection = app.collection::<ProjectEntry>("projects");
            let result = collection.update_one(query, update, None).await.unwrap();

            if result.modified_count > 0 {
                Ok(HttpResponse::Ok().body("Role updated")) // TODO: send room update message?
            } else {
                Ok(HttpResponse::NotFound().body("Project not found"))
            }
        }
        Err(_err) => Ok(HttpResponse::NotFound().body("Project not found")),
    }
}

#[get("/id/{projectID}/{roleID}/latest")]
async fn get_latest_role() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[get("/id/{projectID}/collaborators/")]
async fn list_collaborators(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: authenticate
    let collection = app.collection::<ProjectEntry>("projects");
    let (project_id,) = path.into_inner();
    match ObjectId::parse_str(project_id) {
        Ok(id) => {
            let query = doc! {"_id": id};
            let result = collection
                .find_one(query, None)
                .await
                .expect("Could not find project");

            if let Some(project) = result {
                Ok(HttpResponse::Ok().json(project.collaborators))
            } else {
                Ok(HttpResponse::NotFound().body("Project not found"))
            }
        }
        Err(_) => Ok(HttpResponse::NotFound().body("Project not found")),
    }
}

#[derive(Deserialize)]
struct AddCollaboratorBody {
    username: String,
}
#[post("/id/{projectID}/collaborators/")]
async fn add_collaborator(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    body: web::Json<AddCollaboratorBody>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: authenticate
    let collection = app.collection::<ProjectEntry>("projects");
    let (project_id,) = path.into_inner();
    match ObjectId::parse_str(project_id) {
        Ok(id) => {
            let query = doc! {"_id": id};
            let update = doc! {"$push": {"collaborators": &body.username}};
            let result = collection
                .update_one(query, update, None)
                .await
                .expect("Could not find project");

            if result.matched_count == 1 {
                Ok(HttpResponse::Ok().body("Collaborator added"))
            } else {
                Ok(HttpResponse::NotFound().body("Project not found"))
            }
        }
        Err(_) => Ok(HttpResponse::NotFound().body("Project not found")),
    }
}

#[delete("/id/{projectID}/collaborators/{username}")]
async fn remove_collaborator(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: authenticate
    let collection = app.collection::<ProjectEntry>("projects");
    let (project_id, username) = path.into_inner();
    match ObjectId::parse_str(project_id) {
        Ok(id) => {
            let query = doc! {"_id": id};
            let update = doc! {"$pull": {"collaborators": &username}};
            let result = collection
                .update_one(query, update, None)
                .await
                .expect("Could not find project");

            if result.matched_count == 1 {
                Ok(HttpResponse::Ok().body("Collaborator removed"))
            } else {
                Ok(HttpResponse::NotFound().body("Project not found"))
            }
        }
        Err(_) => Ok(HttpResponse::NotFound().body("Project not found")),
    }
}

#[get("/id/{projectID}/occupants/")]
async fn list_occupants() -> Result<HttpResponse, std::io::Error> {
    // TODO: should this go to the network category?
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

mod tests {}
