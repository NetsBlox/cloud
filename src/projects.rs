use crate::app_data::AppData;
use crate::models::{Project, ProjectMetadata};
use crate::users::can_edit_user;
use actix_session::Session;
use actix_web::{delete, get, patch, post};
use actix_web::{web, HttpResponse};
use futures::stream::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime};
use mongodb::Cursor;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct CreateProjectData {
    name: String,
}

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

#[get("/user/{owner}")]
async fn list_user_projects(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (username,) = path.into_inner();
    let query = doc! {"owner": &username};
    let cursor = app
        .project_metadata
        .find(query, None)
        .await
        .expect("Could not retrieve projects");

    let projects = get_visible_projects(&app, &session, &username, cursor).await;
    Ok(HttpResponse::Ok().json(projects))
}

async fn get_visible_projects(
    app: &AppData,
    session: &Session,
    owner: &str,
    cursor: Cursor<ProjectMetadata>,
) {
    let projects = if can_edit_user(&app, &session, &owner).await {
        cursor.try_collect::<Vec<ProjectMetadata>>().await.unwrap()
    } else {
        cursor
            .try_collect::<Vec<ProjectMetadata>>()
            .await
            .unwrap()
            .into_iter()
            .filter(|p| p.public)
            .collect::<Vec<ProjectMetadata>>()
    };
}

#[get("/shared/{username}")]
async fn list_shared_projects(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (username,) = path.into_inner();
    let query = doc! {"collaborators": &username}; // FIXME
    let cursor = app
        .project_metadata
        .find(query, None)
        .await
        .expect("Could not retrieve projects");

    let projects = get_visible_projects(&app, &session, &username, cursor).await;
    Ok(HttpResponse::Ok().json(projects))
}

#[get("/user/{owner}/{name}")]
async fn get_project_named(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (owner, name) = path.into_inner();
    let query = doc! {"owner": owner, "name": name};
    match app.project_metadata.find_one(query, None).await.unwrap() {
        Some(metadata) => {
            if !can_edit_project(&app, &session, &metadata).await {
                return Ok(HttpResponse::Unauthorized().body("Not allowed."));
            }
            let project = app.fetch_project(&metadata).await;
            Ok(HttpResponse::Ok().json(project))
        }
        None => Ok(HttpResponse::NotFound().body("Project not found")),
    }
}

async fn can_view_project(app: &AppData, session: &Session, project: &ProjectMetadata) -> bool {
    if project.public {
        return true;
    }
    can_edit_project(&app, &session, &project).await
}

async fn can_edit_project(app: &AppData, session: &Session, project: &ProjectMetadata) -> bool {
    match session.get::<String>("username").unwrap_or(None) {
        Some(username) => {
            project.collaborators.contains(&username)
                || can_edit_user(&app, &session, &project.owner).await
        }
        None => false,
    }
}

#[get("/id/{projectID}")]
async fn get_project(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id,) = path.into_inner();
    let query = doc! {"_id": project_id};
    if let Some(metadata) = app.project_metadata.find_one(query, None).await.unwrap() {
        if !can_view_project(&app, &session, &metadata).await {
            return Ok(HttpResponse::Unauthorized().body("Not allowed."));
        }

        // TODO: Should this return xml? Probably not (to match the other version)
        let project = app.fetch_project(&metadata).await;
        Ok(HttpResponse::Ok().json(project))
    } else {
        Ok(HttpResponse::NotFound().body("Project not found"))
    }
}

#[delete("/id/{projectID}")]
async fn delete_project(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id,) = path.into_inner();
    let query = doc! {"_id": project_id};
    if let Some(metadata) = app.project_metadata.find_one(query, None).await.unwrap() {
        // collaborators cannot delete -> only user/admin/etc
        if !can_edit_user(&app, &session, &metadata.owner).await {
            return Ok(HttpResponse::Unauthorized().body("Not allowed."));
        }

        let deleted = app.delete_project(metadata).await;

        Ok(HttpResponse::Ok().body("Project deleted"))
    } else {
        Ok(HttpResponse::NotFound().body("Project not found"))
    }
}

#[derive(Deserialize)]
struct UpdateProjectBody {
    name: String,
}

#[patch("/id/{projectID}")]
async fn update_project(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId,)>,
    body: web::Json<UpdateProjectBody>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id,) = path.into_inner();

    let query = doc! {"_id": project_id};
    match app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        Some(metadata) => {
            if !can_edit_project(&app, &session, &metadata).await {
                return Ok(HttpResponse::Unauthorized().body("Not allowed."));
            }
            let update = doc! {"name": &body.name};
            let result = app
                .project_metadata
                .update_one(query, update, None)
                .await
                .unwrap();

            if result.matched_count > 0 {
                Ok(HttpResponse::Ok().body("Project updated."))
            } else {
                Ok(HttpResponse::NotFound().body("Project not found."))
            }
        }
        None => Ok(HttpResponse::NotFound().body("Project not found.")),
    }
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
    todo!();
    // let (project_id,) = path.into_inner();
    // match ObjectId::parse_str(project_id) {
    //     Ok(project_id) => {
    //         let collection = app.collection::<ProjectEntry>("projects");
    //         let query = doc! {"_id": project_id};
    //         let role_id = Uuid::new_v4();
    //         // FIXME: This isn't right...
    //         let role = RoleData {
    //             project_name: role_data.name,
    //             // TODO: store this using the blob
    //             source_code: role_data.data.unwrap_or("".to_owned()), // TODO: what about media?
    //             media: "".to_owned(),
    //         };
    //         //let update = doc! {format!("roles.{}", role_id): role};
    //         let update = doc! {format!("roles.{}", role_id): "FIXME"};
    //         match collection
    //             .find_one_and_update(query, update, None)
    //             .await
    //             .unwrap()
    //         {
    //             Some(project) => {
    //                 let role_names = project
    //                     .roles
    //                     .into_values()
    //                     .map(|r| r.project_name)
    //                     .collect::<HashSet<String>>();

    //                 if role_names.contains(&role_data.name) {
    //                     let mut base_name = role_data.name;
    //                     let mut role_name = base_name.clone();
    //                     let number: u32 = 2;
    //                     while role_names.contains(&role_name) {
    //                         role_name = format!("{} ({})", base_name, number);
    //                         number += 1;
    //                     }
    //                     let query = doc! {"_id": project_id};
    //                     let update =
    //                         doc! {"$set": {format!("roles.{}.ProjectName", role_id): role_name}};
    //                     collection.update_one(query, update, None).await.unwrap();
    //                 }
    //                 Ok(HttpResponse::Ok().body("Role created"))
    //             }
    //             None => Ok(HttpResponse::NotFound().body("Project not found")),
    //         }
    //     }
    //     Err(_err) => Ok(HttpResponse::NotFound().body("Project not found")),
    // }
}

#[get("/id/{projectID}/{roleID}")]
async fn get_role(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id, role_id) = path.into_inner();
    let query = doc! {"_id": project_id};
    match app.project_metadata.find_one(query, None).await.unwrap() {
        Some(metadata) => {
            if !can_view_project(&app, &session, &metadata).await {
                return Ok(HttpResponse::Unauthorized().body("Not allowed."));
            }
            match metadata.roles.get(&role_id) {
                Some(role_md) => {
                    let role = app.fetch_role(role_md).await;
                    Ok(HttpResponse::Ok().json(role))
                }
                None => Ok(HttpResponse::NotFound().body("Role not found.")),
            }
        }
        None => Ok(HttpResponse::NotFound().body("Project not found.")),
    }
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
    path: web::Path<(ObjectId, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id, role_id) = path.into_inner();
    let query = doc! {"_id": project_id};
    if let Some(metadata) = app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        if !can_edit_project(&app, &session, &metadata).await {
            return Ok(HttpResponse::Unauthorized().body("Not allowed."));
        }

        match metadata.roles.get(&role_id) {
            Some(role_md) => {
                let update =
                    doc! {"$set": {format!("roles.{}.ProjectName", role_id): &role_data.name}};
                let result = app
                    .project_metadata
                    .update_one(query, update, None)
                    .await
                    .unwrap();

                if result.modified_count > 0 {
                    Ok(HttpResponse::Ok().body("Role updated")) // TODO: send room update message?
                } else {
                    Ok(HttpResponse::NotFound().body("Role not found"))
                }
            }
            None => Ok(HttpResponse::NotFound().body("Role not found")),
        }
    } else {
        Ok(HttpResponse::NotFound().body("Role not found"))
    }
}

#[get("/id/{projectID}/{roleID}/latest")]
async fn get_latest_role() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[get("/id/{projectID}/collaborators/")]
async fn list_collaborators(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id,) = path.into_inner();
    let query = doc! {"_id": project_id};

    let result = app
        .project_metadata
        .find_one(query, None)
        .await
        .expect("Could not find project");

    if let Some(metadata) = result {
        if can_edit_project(&app, &session, &metadata).await {
            Ok(HttpResponse::Ok().json(metadata.collaborators))
        } else {
            Ok(HttpResponse::Unauthorized().body("Not allowed."))
        }
    } else {
        Ok(HttpResponse::NotFound().body("Project not found"))
    }
}

#[post("/id/{projectID}/collaborators/{username}")]
async fn add_collaborator(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id, username) = path.into_inner();
    let query = doc! {"_id": project_id};
    match app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        Some(metadata) => {
            if !can_edit_project(&app, &session, &metadata).await {
                return Ok(HttpResponse::Unauthorized().body("Not allowed."));
            }

            let update = doc! {"$push": {"collaborators": &username}};
            let result = app
                .project_metadata
                .update_one(query, update, None)
                .await
                .expect("Could not find project");

            if result.matched_count == 1 {
                Ok(HttpResponse::Ok().body("Collaborator added"))
            } else {
                Ok(HttpResponse::NotFound().body("Project not found"))
            }
        }
        None => Ok(HttpResponse::NotFound().body("Project not found")),
    }
}

#[delete("/id/{projectID}/collaborators/{username}")]
async fn remove_collaborator(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id, username) = path.into_inner();
    let query = doc! {"_id": project_id};
    match app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        Some(metadata) => {
            if !can_edit_project(&app, &session, &metadata).await {
                return Ok(HttpResponse::Unauthorized().body("Not allowed."));
            }

            let update = doc! {"$pull": {"collaborators": &username}};
            let result = app
                .project_metadata
                .update_one(query, update, None)
                .await
                .expect("Could not find project");

            if result.matched_count == 1 {
                Ok(HttpResponse::Ok().body("Collaborator added"))
            } else {
                Ok(HttpResponse::NotFound().body("Project not found"))
            }
        }
        None => Ok(HttpResponse::NotFound().body("Project not found")),
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

mod tests {
    #[actix_web::test]
    async fn test_view_shared_projects() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_view_shared_projects_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_view_shared_projects_admin() {
        unimplemented!();
    }
}
