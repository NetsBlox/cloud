use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::models::{ProjectId, ProjectMetadata, RoleData};
use crate::network::topology;
use crate::users::{can_edit_user, ensure_can_edit_user};
use actix_session::Session;
use actix_web::{delete, get, patch, post};
use actix_web::{web, HttpResponse};
use futures::stream::TryStreamExt;
use mongodb::bson::doc;
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
        .unwrap_or_else(|| body.client_id.to_owned());

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
) -> Vec<ProjectMetadata> {
    let projects = if can_edit_user(app, session, owner).await {
        cursor.try_collect::<Vec<_>>().await.unwrap()
    } else {
        cursor
            .try_collect::<Vec<_>>()
            .await
            .unwrap()
            .into_iter()
            .filter(|p| p.public)
            .collect::<Vec<_>>()
    };
    projects
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
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let query = doc! {"owner": owner, "name": name};
    let metadata = app
        .project_metadata
        .find_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    // TODO: Do I need to have edit permissions?
    ensure_can_view_project(&app, &session, &metadata).await?;

    let project = app.fetch_project(&metadata).await?;
    Ok(HttpResponse::Ok().json(project)) // TODO: Update this to a responder?
}

async fn ensure_can_view_project(
    app: &AppData,
    session: &Session,
    project: &ProjectMetadata,
) -> Result<(), UserError> {
    if !can_view_project(app, session, project).await {
        Err(UserError::PermissionsError)
    } else {
        Ok(())
    }
}

async fn can_view_project(app: &AppData, session: &Session, project: &ProjectMetadata) -> bool {
    if project.public {
        return true;
    }
    can_edit_project(app, session, None, project).await
}

async fn ensure_can_edit_project(
    app: &AppData,
    session: &Session,
    client_id: Option<String>,
    project: &ProjectMetadata,
) -> Result<(), UserError> {
    if !can_edit_project(app, session, client_id, project).await {
        Err(UserError::PermissionsError)
    } else {
        Ok(())
    }
}

async fn can_edit_project(
    app: &AppData,
    session: &Session,
    client_id: Option<String>,
    project: &ProjectMetadata,
) -> bool {
    println!(
        "Can {} edit the project? ({})",
        client_id.as_deref().unwrap_or("None"),
        project.owner
    );
    let is_owner = client_id.map(|id| id == project.owner).unwrap_or(false);

    is_owner
        || match session.get::<String>("username").unwrap_or(None) {
            Some(username) => {
                project.collaborators.contains(&username)
                    || can_edit_user(app, session, &project.owner).await
            }
            None => false,
        }
}

#[get("/id/{projectID}")]
async fn get_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let query = doc! {"id": project_id};
    // TODO: better error handling
    let metadata = app
        .project_metadata
        .find_one(query, None)
        .await
        .map_err(|_err| UserError::InternalError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_view_project(&app, &session, &metadata).await?;

    let project = app.fetch_project(&metadata).await?;
    Ok(HttpResponse::Ok().json(project)) // TODO: Update this to a responder?
}

#[post("/id/{projectID}/publish")] // TODO: Will this collide with role?
async fn publish_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let query = doc! {"id": project_id};
    let metadata = app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .map_err(|_err| UserError::InternalError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_edit_project(&app, &session, None, &metadata).await?;

    let update = doc! {"public": true};
    app.project_metadata
        .update_one(query, update, None)
        .await
        .map_err(|_err| UserError::InternalError)?; // TODO: wrap the error?

    Ok(HttpResponse::Ok().body("Project published!"))
}

#[post("/id/{projectID}/unpublish")]
async fn unpublish_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let query = doc! {"id": project_id};
    let metadata = app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .map_err(|_err| UserError::InternalError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_edit_project(&app, &session, None, &metadata).await?;

    let update = doc! {"public": false};
    let result = app
        .project_metadata
        .update_one(query, update, None)
        .await
        .map_err(|_err| UserError::InternalError)?; // TODO: wrap the error?

    if result.matched_count > 0 {
        Ok(HttpResponse::Ok().body("Project published!"))
    } else {
        Err(UserError::ProjectNotFoundError)
    }
}

#[delete("/id/{projectID}")]
async fn delete_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let query = doc! {"id": project_id};
    let metadata = app
        .project_metadata
        .find_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    // collaborators cannot delete -> only user/admin/etc

    ensure_can_edit_user(&app, &session, &metadata.owner).await?;
    app.delete_project(metadata).await?; // TODO:
    Ok(HttpResponse::Ok().body("Project deleted"))
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
    path: web::Path<(ProjectId,)>,
    body: web::Json<UpdateProjectBody>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();

    // TODO: validate the name. Or make it a type?
    let query = doc! {"id": project_id};
    let metadata = app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    let body = body.into_inner();
    ensure_can_edit_project(&app, &session, body.client_id, &metadata).await?;

    println!("Changing name from {} to {}", &metadata.name, &body.name);
    let update = doc! {"$set": {"name": &body.name}};
    let options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let updated_metadata = app
        .project_metadata
        .find_one_and_update(query, update, options)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    println!("New project name is {:?}", updated_metadata.name);
    app.network.do_send(topology::SendRoomState {
        project: updated_metadata,
    });

    Ok(HttpResponse::Ok().body("Project updated."))
}

#[get("/id/{projectID}/latest")] // Include unsaved data
async fn get_latest_project() -> Result<HttpResponse, std::io::Error> {
    todo!(); // TODO: return xml string
             //Ok(HttpResponse::Ok().body(serialized_project))
}

#[get("/id/{projectID}/thumbnail")]
async fn get_project_thumbnail(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();

    let query = doc! {"id": project_id};
    let metadata = app
        .project_metadata
        .find_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_view_project(&app, &session, &metadata).await?;

    Ok(HttpResponse::Ok().body(metadata.thumbnail))
}

#[derive(Deserialize)]
struct CreateRoleData {
    name: String,
    source_code: Option<String>,
    media: Option<String>,
}

impl From<CreateRoleData> for RoleData {
    fn from(data: CreateRoleData) -> RoleData {
        RoleData {
            project_name: data.name,
            source_code: data.source_code.unwrap_or_else(String::new),
            media: data.media.unwrap_or_else(String::new),
        }
    }
}

#[post("/id/{projectID}/")]
async fn create_role(
    app: web::Data<AppData>,
    body: web::Json<CreateRoleData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let query = doc! {"id": project_id};
    let metadata = app
        .project_metadata
        .find_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_edit_project(&app, &session, None, &metadata).await?;

    app.create_role(metadata, body.into_inner().into()).await;
    Ok(HttpResponse::Ok().body("Role created"))
}

#[get("/id/{projectID}/{roleID}")]
async fn get_role(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();
    let query = doc! {"id": project_id};
    let metadata = app
        .project_metadata
        .find_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_view_project(&app, &session, &metadata).await?;
    let role_md = metadata
        .roles
        .get(&role_id)
        .ok_or_else(|| UserError::RoleNotFoundError)?;

    let role = app.fetch_role(role_md).await;
    Ok(HttpResponse::Ok().json(role))
}

#[delete("/id/{projectID}/{roleID}")]
async fn delete_role(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();
    let query = doc! {"id": project_id};
    let metadata = app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    // TODO: Move this to AppData
    ensure_can_edit_project(&app, &session, None, &metadata).await?;
    let update = doc! {"$unset": {format!("roles.{}", role_id): &""}};
    let options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let updated_metadata = app
        .project_metadata
        .find_one_and_update(query, update, options)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?; // TODO: wrap the error?

    if updated_metadata.is_some() {
        app.network.do_send(topology::SendRoomState {
            project: updated_metadata.unwrap(),
        });
    }

    Ok(HttpResponse::Ok().body("Deleted!"))
}

#[post("/id/{projectID}/{roleID}")]
async fn save_role(
    app: web::Data<AppData>,
    body: web::Json<RoleData>,
    path: web::Path<(ProjectId, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();
    let query = doc! {"id": project_id};
    let metadata = app
        .project_metadata
        .find_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_edit_project(&app, &session, None, &metadata).await?;
    app.save_role(&metadata, &role_id, body.into_inner()).await;

    Ok(HttpResponse::Ok().body("Saved!"))
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
    path: web::Path<(ProjectId, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();

    let query = doc! {"id": project_id};
    let body = body.into_inner();
    let metadata = app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_edit_project(&app, &session, body.client_id, &metadata).await?;

    if metadata.roles.contains_key(&role_id) {
        let update = doc! {"$set": {format!("roles.{}.ProjectName", role_id): &body.name}};
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let updated_metadata = app
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
            .ok_or_else(|| UserError::ProjectNotFoundError)?;

        app.network.do_send(topology::SendRoomState {
            project: updated_metadata,
        });
        Ok(HttpResponse::Ok().body("Role updated"))
    } else {
        Err(UserError::RoleNotFoundError)
    }
}

#[get("/id/{projectID}/{roleID}/latest")]
async fn get_latest_role() -> Result<HttpResponse, std::io::Error> {
    todo!();
}

#[get("/id/{projectID}/collaborators/")]
async fn list_collaborators(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let query = doc! {"id": project_id};

    let metadata = app
        .project_metadata
        .find_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_edit_project(&app, &session, None, &metadata).await?;
    Ok(HttpResponse::Ok().json(metadata.collaborators))
}

// TODO: Should we use this or the invite endpoints?
#[post("/id/{projectID}/collaborators/{username}")]
async fn add_collaborator(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, username) = path.into_inner();
    let query = doc! {"id": project_id};
    let metadata = app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_edit_project(&app, &session, None, &metadata).await?;

    let update = doc! {"$push": {"collaborators": &username}};
    let result = app
        .project_metadata
        .update_one(query, update, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?; // TODO: wrap the error?

    if result.matched_count == 1 {
        Ok(HttpResponse::Ok().body("Collaborator added"))
    } else {
        Err(UserError::ProjectNotFoundError)
    }
}

#[delete("/id/{projectID}/collaborators/{username}")]
async fn remove_collaborator(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, username) = path.into_inner();
    let query = doc! {"id": project_id};
    let metadata = app
        .project_metadata
        .find_one(query.clone(), None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;
    ensure_can_edit_project(&app, &session, None, &metadata).await?;

    let update = doc! {"$pull": {"collaborators": &username}};
    let result = app
        .project_metadata
        .update_one(query, update, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)?; // TODO: wrap the error?

    if result.matched_count == 1 {
        Ok(HttpResponse::Ok().body("Collaborator added"))
    } else {
        Err(UserError::ProjectNotFoundError)
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
