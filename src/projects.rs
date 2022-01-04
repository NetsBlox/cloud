use crate::app_data::AppData;
use crate::models::{ProjectMetadata, RoleData};
use crate::network::topology;
use crate::users::can_edit_user;
use actix_session::Session;
use actix_web::{delete, get, patch, post};
use actix_web::{web, HttpResponse};
use futures::stream::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument};
use mongodb::Cursor;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectData {
    owner: Option<String>,
    name: String,
    roles: Option<Vec<RoleData>>,
    client_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatedRole<'a> {
    project_id: String,
    role_id: &'a str,
    name: String,
    role_name: &'a str,
}

#[post("/")]
async fn create_project(
    app: web::Data<AppData>,
    body: web::Json<CreateProjectData>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let owner = session
        .get::<String>("username")
        .unwrap_or(None)
        .unwrap_or(body.client_id.to_owned());

    let name = body.name.to_owned();
    let metadata = app
        .import_project(&owner, &name, body.into_inner().roles)
        .await;

    let role_id = metadata.roles.keys().next().unwrap();
    let role_name = &metadata.roles.get(role_id).unwrap().project_name;
    Ok(HttpResponse::Ok().json(CreatedRole {
        project_id: metadata.id.to_string(),
        role_id,
        name: metadata.name,
        role_name,
    }))
    // TODO: should we automatically set the client to the role?
    // TODO: how should we determine the role to open?
    // TODO: add allow_rename query string parameter?
    // TODO: return the project name/id, role name/id
}

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
            if !can_edit_project(&app, &session, None, &metadata).await {
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
    can_edit_project(&app, &session, None, &project).await
}

async fn can_edit_project(
    app: &AppData,
    session: &Session,
    client_id: Option<String>,
    project: &ProjectMetadata,
) -> bool {
    println!(
        "Can {} edit the project? ({})",
        client_id.clone().unwrap_or("None".to_owned()),
        project.owner
    );
    let is_owner = client_id.map(|id| id == project.owner).unwrap_or(false);

    is_owner
        || match session.get::<String>("username").unwrap_or(None) {
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
    let query = doc! {"id": project_id};
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

#[post("/id/{projectID}/publish")] // TODO: Will this collide with role
async fn publish_project(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id,) = path.into_inner();
    let query = doc! {"id": project_id};
    if let Some(metadata) = app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        if !can_edit_project(&app, &session, None, &metadata).await {
            return Ok(HttpResponse::Unauthorized().body("Not allowed."));
        }

        let update = doc! {"public": true};
        app.project_metadata
            .update_one(query, update, None)
            .await
            .unwrap();

        Ok(HttpResponse::Ok().body("Project published!"))
    } else {
        Ok(HttpResponse::NotFound().body("Project not found"))
    }
}

#[post("/id/{projectID}/unpublish")] // TODO: Will this collide with role
async fn unpublish_project(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id,) = path.into_inner();
    let query = doc! {"id": project_id};
    if let Some(metadata) = app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        if !can_edit_project(&app, &session, None, &metadata).await {
            return Ok(HttpResponse::Unauthorized().body("Not allowed."));
        }

        let update = doc! {"public": false};
        app.project_metadata
            .update_one(query, update, None)
            .await
            .unwrap();

        Ok(HttpResponse::Ok().body("Project published!"))
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
    let query = doc! {"id": project_id};
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
#[serde(rename_all = "camelCase")]
struct UpdateProjectBody {
    name: String,
    client_id: Option<String>,
}

#[patch("/id/{projectID}")]
async fn update_project(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    body: web::Json<UpdateProjectBody>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (id_str,) = path.into_inner();
    // TODO: make a ProjectID type
    let project_id = match ObjectId::parse_str(id_str) {
        Ok(id) => id,
        Err(_) => return Ok(HttpResponse::BadRequest().body("Invalid project ID.")),
    };

    // TODO: validate the name. Or make it a type?
    let query = doc! {"id": project_id};
    match app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        Some(metadata) => {
            let body = body.into_inner();
            if !can_edit_project(&app, &session, body.client_id, &metadata).await {
                return Ok(HttpResponse::Unauthorized().body("Not allowed."));
            }
            println!("Changing name from {} to {}", &metadata.name, &body.name);
            let update = doc! {"$set": {"name": &body.name}};
            let options = FindOneAndUpdateOptions::builder()
                .return_document(ReturnDocument::After)
                .build();
            let updated_metadata = app
                .project_metadata
                .find_one_and_update(query, update, options)
                .await
                .unwrap();

            if updated_metadata.is_some() {
                println!(
                    "New project name is {:?}",
                    updated_metadata.as_ref().unwrap().name
                );
                app.network.do_send(topology::SendRoomState {
                    project: updated_metadata.unwrap(),
                });
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
            let query = doc! {"id": id};
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
    source_code: Option<String>,
    media: Option<String>,
}

#[post("/id/{projectID}/")]
async fn create_role(
    app: web::Data<AppData>,
    body: web::Json<CreateRoleData>,
    path: web::Path<(ObjectId,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: send room update message? I am not sure
    // TODO: this shouldn't need to. It should trigger an update sent
    let (project_id,) = path.into_inner();
    let query = doc! {"id": project_id};
    if let Some(metadata) = app.project_metadata.find_one(query, None).await.unwrap() {
        if !can_edit_project(&app, &session, None, &metadata).await {
            return Ok(HttpResponse::Unauthorized().body("Not allowed."));
        }

        let updated_metadata = app
            .create_role(
                metadata,
                &body.name,
                body.source_code.to_owned(),
                body.media.to_owned(),
            )
            .await;

        if updated_metadata.is_ok() {
            app.network.do_send(topology::SendRoomState {
                project: updated_metadata.unwrap(),
            });
        }

        Ok(HttpResponse::Ok().body("Role created"))
    } else {
        Ok(HttpResponse::NotFound().body("Project not found"))
    }
}

#[get("/id/{projectID}/{roleID}")]
async fn get_role(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id, role_id) = path.into_inner();
    let query = doc! {"id": project_id};
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
async fn delete_role(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id, role_id) = path.into_inner();
    let query = doc! {"id": project_id};
    match app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        Some(metadata) => {
            if !can_edit_project(&app, &session, None, &metadata).await {
                return Ok(HttpResponse::Unauthorized().body("Not allowed."));
            }
            let update = doc! {"$unset": {format!("roles.{}", role_id): &""}};
            let options = FindOneAndUpdateOptions::builder()
                .return_document(ReturnDocument::After)
                .build();

            let updated_metadata = app
                .project_metadata
                .find_one_and_update(query, update, options)
                .await
                .unwrap();

            if updated_metadata.is_some() {
                app.network.do_send(topology::SendRoomState {
                    project: updated_metadata.unwrap(),
                });
            }

            Ok(HttpResponse::Ok().body("Deleted!"))
        }
        None => Ok(HttpResponse::NotFound().body("Project not found.")),
    }
}

#[derive(Deserialize)]
struct SaveRoleBody {
    source_code: String,
    media: String,
}

#[post("/id/{projectID}/{roleID}")]
async fn save_role(
    app: web::Data<AppData>,
    body: web::Json<SaveRoleBody>,
    path: web::Path<(ObjectId, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id, role_id) = path.into_inner();
    let query = doc! {"id": project_id};
    match app.project_metadata.find_one(query, None).await.unwrap() {
        Some(metadata) => {
            if !can_edit_project(&app, &session, None, &metadata).await {
                return Ok(HttpResponse::Unauthorized().body("Not allowed."));
            }
            app.save_role(&metadata, &role_id, &body.source_code, &body.media)
                .await;

            Ok(HttpResponse::Ok().body("Saved!"))
        }
        None => Ok(HttpResponse::NotFound().body("Project not found.")),
    }
    // TODO: send room update message?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameRoleData {
    name: String,
    client_id: Option<String>,
}

#[patch("/id/{projectID}/{roleID}")]
async fn rename_role(
    app: web::Data<AppData>,
    body: web::Json<RenameRoleData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id_str, role_id) = path.into_inner();
    let project_id = match ObjectId::parse_str(project_id_str) {
        Ok(id) => id,
        Err(_) => return Ok(HttpResponse::BadRequest().body("Invalid project ID.")),
    };

    let query = doc! {"id": project_id};
    let body = body.into_inner();
    if let Some(metadata) = app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        if !can_edit_project(&app, &session, body.client_id, &metadata).await {
            return Ok(HttpResponse::Unauthorized().body("Not allowed."));
        }

        if metadata.roles.contains_key(&role_id) {
            let update = doc! {"$set": {format!("roles.{}.ProjectName", role_id): &body.name}};
            let options = FindOneAndUpdateOptions::builder()
                .return_document(ReturnDocument::After)
                .build();

            let updated_metadata = app
                .project_metadata
                .find_one_and_update(query, update, options)
                .await
                .unwrap();

            if updated_metadata.is_some() {
                app.network.do_send(topology::SendRoomState {
                    project: updated_metadata.unwrap(),
                });
                Ok(HttpResponse::Ok().body("Role updated"))
            } else {
                Ok(HttpResponse::NotFound().body("Project not found"))
            }
        } else {
            Ok(HttpResponse::NotFound().body("Role not found"))
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
    let query = doc! {"id": project_id};

    let result = app
        .project_metadata
        .find_one(query, None)
        .await
        .expect("Could not find project");

    if let Some(metadata) = result {
        if can_edit_project(&app, &session, None, &metadata).await {
            Ok(HttpResponse::Ok().json(metadata.collaborators))
        } else {
            Ok(HttpResponse::Unauthorized().body("Not allowed."))
        }
    } else {
        Ok(HttpResponse::NotFound().body("Project not found"))
    }
}

// TODO: Should we use this or the invite endpoints?
#[post("/id/{projectID}/collaborators/{username}")]
async fn add_collaborator(
    app: web::Data<AppData>,
    path: web::Path<(ObjectId, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (project_id, username) = path.into_inner();
    let query = doc! {"id": project_id};
    match app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        Some(metadata) => {
            if !can_edit_project(&app, &session, None, &metadata).await {
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
    let query = doc! {"id": project_id};
    match app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        Some(metadata) => {
            if !can_edit_project(&app, &session, None, &metadata).await {
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

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_project)
        .service(update_project)
        .service(delete_project)
        .service(list_user_projects)
        .service(list_shared_projects)
        .service(get_project)
        .service(get_project_named)
        .service(publish_project)
        .service(unpublish_project)
        .service(get_latest_project)
        .service(get_project_thumbnail)
        .service(get_role)
        .service(get_latest_role)
        .service(create_role)
        .service(save_role)
        .service(rename_role)
        .service(delete_role)
        .service(add_collaborator)
        .service(list_collaborators)
        .service(remove_collaborator);
}

mod tests {
    #[actix_web::test]
    async fn test_create_project() {
        todo!();
    }

    #[actix_web::test]
    async fn test_create_project_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_create_project_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_project() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_project_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_project_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_project_named() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_project_named_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_project_named_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_latest_project() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_latest_project_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_latest_project_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_project_thumbnail() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_project_thumbnail_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_project_thumbnail_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_update_project() {
        todo!();
    }

    #[actix_web::test]
    async fn test_update_project_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_update_project_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_publish_project() {
        todo!();
    }

    #[actix_web::test]
    async fn test_publish_project_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_publish_project_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_unpublish_project() {
        todo!();
    }

    #[actix_web::test]
    async fn test_unpublish_project_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_unpublish_project_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_delete_project() {
        // TODO: Should the client be notified?
        todo!();
    }

    #[actix_web::test]
    async fn test_delete_project_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_delete_project_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_rename_project() {
        todo!();
    }

    #[actix_web::test]
    async fn test_rename_project_invalid_name() {
        todo!();
    }

    #[actix_web::test]
    async fn test_rename_project_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_rename_project_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_rename_project_room_update() {
        todo!();
    }

    #[actix_web::test]
    async fn test_list_user_projects() {
        todo!();
    }

    #[actix_web::test]
    async fn test_list_user_projects_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_list_user_projects_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_view_shared_projects() {
        todo!();
    }

    #[actix_web::test]
    async fn test_view_shared_projects_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_view_shared_projects_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_create_role() {
        todo!();
    }

    #[actix_web::test]
    async fn test_create_role_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_create_role_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_create_role_room_update() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_role() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_role_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_role_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_latest_role() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_latest_role_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_latest_role_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_rename_role() {
        todo!();
    }

    #[actix_web::test]
    async fn test_rename_role_invalid_name() {
        todo!();
    }

    #[actix_web::test]
    async fn test_rename_role_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_rename_role_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_rename_role_room_update() {
        todo!();
    }

    #[actix_web::test]
    async fn test_save_role() {
        todo!();
    }

    #[actix_web::test]
    async fn test_save_role_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_save_role_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_delete_role() {
        todo!();
    }

    #[actix_web::test]
    async fn test_delete_role_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_delete_role_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_delete_role_room_update() {
        todo!();
    }

    #[actix_web::test]
    async fn test_add_collaborator() {
        todo!();
    }

    #[actix_web::test]
    async fn test_add_collaborator_invalid_name() {
        todo!();
    }

    #[actix_web::test]
    async fn test_add_collaborator_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_add_collaborator_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_add_collaborator_room_update() {
        todo!();
    }

    #[actix_web::test]
    async fn test_remove_collaborator() {
        todo!();
    }

    #[actix_web::test]
    async fn test_remove_collaborator_invalid_name() {
        todo!();
    }

    #[actix_web::test]
    async fn test_remove_collaborator_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_remove_collaborator_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_remove_collaborator_room_update() {
        todo!();
    }

    #[actix_web::test]
    async fn test_list_collaborators() {
        todo!();
    }

    #[actix_web::test]
    async fn test_list_collaborators_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_list_collaborators_admin() {
        todo!();
    }
}
