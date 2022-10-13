use std::collections::HashMap;
use std::io::BufWriter;

use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::libraries;
use crate::models::{ProjectMetadata, RoleData};
use crate::network::topology::{self, BrowserClientState};
use crate::users::{can_edit_user, ensure_can_edit_user};
use actix_session::Session;
use actix_web::{delete, get, patch, post};
use actix_web::{web, HttpResponse};
use futures::stream::{FuturesUnordered, TryStreamExt};
use image::{
    codecs::png::PngEncoder, ColorType, EncodableLayout, GenericImageView, ImageEncoder,
    ImageFormat, RgbaImage,
};
use mongodb::bson::doc;
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument};
use mongodb::Cursor;
use netsblox_core::{
    ClientID, CreateProjectData, Project, ProjectId, PublishState, RoleId, SaveState,
    UpdateProjectData, UpdateRoleData,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[post("/")]
async fn create_project(
    app: web::Data<AppData>,
    body: web::Json<CreateProjectData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let current_user = session.get::<String>("username").unwrap_or(None);
    let project_data = body.into_inner();
    let owner_name = project_data.owner.or_else(|| current_user);
    // FIXME: what if the owner is a client ID?
    if let Some(username) = &owner_name {
        ensure_can_edit_user(&app, &session, username).await?;
    }

    let owner = owner_name
        .or_else(|| project_data.client_id.map(|id| id.as_str().to_string()))
        .ok_or_else(|| UserError::LoginRequiredError)?;

    let name = project_data.name.to_owned();
    let metadata = app
        .import_project(&owner, &name, project_data.roles, project_data.save_state)
        .await?;

    let role_id = metadata.roles.keys().next().unwrap();
    let role_name = &metadata.roles.get(role_id).unwrap().name;
    Ok(HttpResponse::Ok().json(metadata))
    // TODO: add allow_rename query string parameter?
}

#[get("/user/{owner}")]
async fn list_user_projects(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let query = doc! {"owner": &username, "saveState": SaveState::SAVED};
    println!("query is: {:?}", query);
    let cursor = app
        .project_metadata
        .find(query, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;

    let projects = get_visible_projects(&app, &session, &username, cursor).await;
    println!("Found {} projects for {}", projects.len(), username);
    Ok(HttpResponse::Ok().json(projects))
}

async fn get_visible_projects(
    app: &AppData,
    session: &Session,
    owner: &str,
    cursor: Cursor<ProjectMetadata>,
) -> Vec<netsblox_core::ProjectMetadata> {
    let projects = if can_edit_user(app, session, owner).await.unwrap_or(false) {
        cursor.try_collect::<Vec<_>>().await.unwrap()
    } else {
        cursor
            .try_collect::<Vec<_>>()
            .await
            .unwrap()
            .into_iter()
            .filter(|p| match p.state {
                PublishState::Public => true,
                _ => false,
            })
            .collect::<Vec<_>>()
    };
    projects.into_iter().map(|project| project.into()).collect()
}

#[get("/shared/{username}")]
async fn list_shared_projects(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &username).await?;

    let query = doc! {"collaborators": &username, "saveState": SaveState::SAVED};
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
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    // TODO: Do I need to have edit permissions?
    ensure_can_view_project_metadata(&app, &session, None, &metadata).await?;

    let project = app.fetch_project(&metadata).await?;
    Ok(HttpResponse::Ok().json(project)) // TODO: Update this to a responder?
}

#[get("/user/{owner}/{name}/metadata")]
async fn get_project_metadata(
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
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_view_project_metadata(&app, &session, None, &metadata).await?;

    let metadata: netsblox_core::ProjectMetadata = metadata.into();
    Ok(HttpResponse::Ok().json(metadata))
}

#[get("/id/{id}/metadata")]
async fn get_project_id_metadata(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let metadata = ensure_can_view_project(&app, &session, None, &project_id).await?;

    let metadata: netsblox_core::ProjectMetadata = metadata.into();
    Ok(HttpResponse::Ok().json(metadata))
}

pub async fn ensure_can_view_project(
    app: &AppData,
    session: &Session,
    client_id: Option<ClientID>,
    project_id: &ProjectId,
) -> Result<ProjectMetadata, UserError> {
    let metadata = app.get_project_metadatum(&project_id).await?;
    ensure_can_view_project_metadata(&app, &session, client_id, &metadata).await?;
    Ok(metadata)
}

async fn ensure_can_view_project_metadata(
    app: &AppData,
    session: &Session,
    client_id: Option<ClientID>,
    project: &ProjectMetadata,
) -> Result<(), UserError> {
    if !can_view_project(app, session, client_id, project).await? {
        Err(UserError::PermissionsError)
    } else {
        Ok(())
    }
}

fn flatten<T>(nested: Option<Option<T>>) -> Option<T> {
    match nested {
        Some(x) => x,
        None => None,
    }
}

async fn can_view_project(
    app: &AppData,
    session: &Session,
    client_id: Option<ClientID>,
    project: &ProjectMetadata,
) -> Result<bool, UserError> {
    match project.state {
        PublishState::Private => {
            if let Some(username) = session.get::<String>("username").unwrap_or(None) {
                let query = doc! {"username": username};
                let invite = flatten(app.occupant_invites.find_one(query, None).await.ok());
                if invite.is_some() {
                    return Ok(true);
                }
            }

            can_edit_project(app, session, client_id, project).await
        }
        // Allow viewing projects pending approval. Disclaimer should be on client side
        // Client can also disable JS or simply prompt the user if he/she would still like
        // to open the project
        _ => Ok(true),
    }
}

pub async fn ensure_can_edit_project(
    app: &AppData,
    session: &Session,
    client_id: Option<ClientID>,
    project_id: &ProjectId,
) -> Result<ProjectMetadata, UserError> {
    let metadata = app.get_project_metadatum(project_id).await?;

    if can_edit_project(app, session, client_id, &metadata).await? {
        Ok(metadata)
    } else {
        Err(UserError::PermissionsError)
    }
}

async fn can_edit_project(
    app: &AppData,
    session: &Session,
    client_id: Option<ClientID>,
    project: &ProjectMetadata,
) -> Result<bool, UserError> {
    println!("Can {:?} edit the project? ({})", client_id, project.owner);
    let is_owner = client_id
        .map(|id| id.as_str() == &project.owner)
        .unwrap_or(false);

    if is_owner {
        Ok(true)
    } else {
        match session.get::<String>("username").unwrap_or(None) {
            Some(username) => {
                if project.collaborators.contains(&username) {
                    Ok(true)
                } else {
                    can_edit_user(app, session, &project.owner).await
                }
            }
            None => Err(UserError::LoginRequiredError),
        }
    }
}

#[get("/id/{projectID}")]
async fn get_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let metadata = ensure_can_view_project(&app, &session, None, &project_id).await?;

    let project: netsblox_core::Project = app.fetch_project(&metadata).await?.into();
    Ok(HttpResponse::Ok().json(project)) // TODO: Update this to a responder?
}

#[post("/id/{projectID}/publish")]
async fn publish_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let query = doc! {"id": &project_id};
    let metadata = ensure_can_edit_project(&app, &session, None, &project_id).await?;
    let state = if is_approval_required(&app, &metadata).await? {
        PublishState::PendingApproval
    } else {
        PublishState::Public
    };

    let update = doc! {"$set": {"state": &state}};
    app.project_metadata
        .update_one(query, update, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;

    Ok(HttpResponse::Ok().json(PublishState::Public))
}

async fn is_approval_required(
    app: &AppData,
    metadata: &ProjectMetadata,
) -> Result<bool, UserError> {
    for role_md in metadata.roles.values() {
        let role = app.fetch_role(&role_md).await?;
        if libraries::is_approval_required(&role.code) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[post("/id/{projectID}/unpublish")]
async fn unpublish_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let query = doc! {"id": &project_id};
    ensure_can_edit_project(&app, &session, None, &project_id).await?;

    let update = doc! {"$set": {"state": PublishState::Private}};
    let result = app
        .project_metadata
        .update_one(query, update, None)
        .await
        .map_err(|_err| UserError::InternalError)?;

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
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    // collaborators cannot delete -> only user/admin/etc
    ensure_can_edit_user(&app, &session, &metadata.owner).await?;
    app.delete_project(metadata).await?; // TODO:
    Ok(HttpResponse::Ok().body("Project deleted"))
}

#[patch("/id/{projectID}")]
async fn update_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    body: web::Json<UpdateProjectData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();

    // TODO: validate the name. Or make it a type?
    let query = doc! {"id": &project_id};
    let body = body.into_inner();
    let metadata = ensure_can_edit_project(&app, &session, body.client_id, &project_id).await?;

    println!("Changing name from {} to {}", &metadata.name, &body.name);
    let update = doc! {"$set": {"name": &body.name}};
    let options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    // TODO: How do we know there isn't a name collision?
    let updated_metadata = app
        .project_metadata
        .find_one_and_update(query, update, options)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    app.on_room_changed(updated_metadata);
    Ok(HttpResponse::Ok().body("Project updated."))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetProjectRoleParams {
    // FIXME: this isn't really secure since it is easy to spoof the client ID
    client_id: Option<ClientID>,
}

#[get("/id/{projectID}/latest")]
async fn get_latest_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    params: web::Query<GetProjectRoleParams>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let client_id = params.into_inner().client_id;
    let metadata = ensure_can_view_project(&app, &session, client_id, &project_id).await?;

    let roles = metadata
        .roles
        .keys()
        .map(|role_id| fetch_role_data(&app, &metadata, role_id.to_owned()))
        .collect::<FuturesUnordered<_>>()
        .try_collect::<HashMap<RoleId, RoleData>>()
        .await
        .unwrap(); // TODO: handle errors

    let project = Project {
        id: metadata.id.to_owned(),
        name: metadata.name.to_owned(),
        owner: metadata.owner.to_owned(),
        updated: metadata.updated.to_system_time(),
        state: metadata.state.to_owned(),
        collaborators: metadata.collaborators.to_owned(),
        origin_time: metadata.origin_time.to_system_time(),
        save_state: metadata.save_state.to_owned(),
        roles,
    };
    Ok(HttpResponse::Ok().json(project))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThumbnailParams {
    aspect_ratio: Option<f32>,
}

#[get("/id/{projectID}/thumbnail")]
async fn get_project_thumbnail(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    params: web::Query<ThumbnailParams>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let metadata = ensure_can_view_project(&app, &session, None, &project_id).await?;

    let role_metadata = metadata
        .roles
        .values()
        .max_by_key(|md| md.updated)
        .ok_or_else(|| UserError::ThumbnailNotFoundError)?;
    let role = app.fetch_role(&role_metadata).await?;
    let thumbnail = role
        .code
        .split("<thumbnail>data:image/png;base64,")
        .skip(1)
        .next()
        .and_then(|text| text.split("</thumbnail>").next())
        .ok_or_else(|| UserError::ThumbnailNotFoundError)
        .and_then(|thumbnail_str| {
            base64::decode(thumbnail_str)
                .map_err(|err| InternalError::Base64DecodeError(err).into())
        })
        .and_then(|image_data| {
            image::load_from_memory_with_format(&image_data, ImageFormat::Png)
                .map_err(|err| InternalError::ThumbnailDecodeError(err).into())
        })?;

    let image_content = if let Some(aspect_ratio) = params.aspect_ratio {
        let (width, height) = thumbnail.dimensions();
        let current_ratio = (width as f32) / (height as f32);
        let (resized_width, resized_height) = if current_ratio < aspect_ratio {
            let new_width = (aspect_ratio * (height as f32)) as u32;
            (new_width, height)
        } else {
            let new_height = ((width as f32) / aspect_ratio) as u32;
            (width, new_height)
        };

        let top_offset: u32 = (resized_height - height) / 2;
        let left_offset: u32 = (resized_width - width) / 2;
        let mut image = RgbaImage::new(resized_width, resized_height);
        for x in 0..width {
            for y in 0..height {
                let pixel = thumbnail.get_pixel(x, y);
                image.put_pixel(x + left_offset, y + top_offset, pixel);
            }
        }
        // encode the bytes as a png
        let mut png_bytes = BufWriter::new(Vec::new());
        let encoder = PngEncoder::new(&mut png_bytes);
        let color = ColorType::Rgba8;
        encoder
            .write_image(image.as_bytes(), resized_width, resized_height, color)
            .map_err(|err| InternalError::ThumbnailEncodeError(err))?;
        actix_web::web::Bytes::copy_from_slice(&png_bytes.into_inner().unwrap())
    } else {
        let (width, height) = thumbnail.dimensions();
        let mut png_bytes = BufWriter::new(Vec::new());
        let encoder = PngEncoder::new(&mut png_bytes);
        let color = ColorType::Rgba8;
        encoder
            .write_image(thumbnail.as_bytes(), width, height, color)
            .map_err(|err| InternalError::ThumbnailEncodeError(err))?;
        actix_web::web::Bytes::copy_from_slice(&png_bytes.into_inner().unwrap())
    };

    Ok(HttpResponse::Ok()
        .content_type("image/png")
        .body(image_content))
}

#[derive(Deserialize)]
struct CreateRoleData {
    name: String,
    code: Option<String>,
    media: Option<String>,
}

impl From<CreateRoleData> for RoleData {
    fn from(data: CreateRoleData) -> RoleData {
        RoleData {
            name: data.name,
            code: data.code.unwrap_or_else(String::new),
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
    let metadata = ensure_can_edit_project(&app, &session, None, &project_id).await?;

    let updated_metadata = app.create_role(metadata, body.into_inner().into()).await?;
    app.on_room_changed(updated_metadata);
    Ok(HttpResponse::Ok().body("Role created"))
}

#[get("/id/{projectID}/{roleID}")]
async fn get_role(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, RoleId)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();

    let metadata = ensure_can_view_project(&app, &session, None, &project_id).await?;
    let role_md = metadata
        .roles
        .get(&role_id)
        .ok_or_else(|| UserError::RoleNotFoundError)?;

    let role = app.fetch_role(role_md).await?;
    Ok(HttpResponse::Ok().json(role))
}

#[delete("/id/{projectID}/{roleID}")]
async fn delete_role(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, RoleId)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();
    let query = doc! {"id": &project_id};

    let metadata = ensure_can_edit_project(&app, &session, None, &project_id).await?;
    if metadata.roles.keys().count() == 1 {
        return Err(UserError::CannotDeleteLastRoleError);
    }

    let update = doc! {"$unset": {format!("roles.{}", role_id): &""}};
    let options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let updated_metadata = app
        .project_metadata
        .find_one_and_update(query, update, options)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    app.on_room_changed(updated_metadata);
    Ok(HttpResponse::Ok().body("Deleted!"))
}

#[post("/id/{projectID}/{roleID}")]
async fn save_role(
    app: web::Data<AppData>,
    body: web::Json<RoleData>,
    path: web::Path<(ProjectId, RoleId)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();
    let metadata = ensure_can_edit_project(&app, &session, None, &project_id).await?;
    let updated_metdata = app
        .save_role(&metadata, &role_id, body.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(updated_metdata.state))
}

#[patch("/id/{projectID}/{roleID}")]
async fn rename_role(
    app: web::Data<AppData>,
    body: web::Json<UpdateRoleData>,
    path: web::Path<(ProjectId, RoleId)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();

    let query = doc! {"id": &project_id};
    let body = body.into_inner();
    let metadata = ensure_can_edit_project(&app, &session, body.client_id, &project_id).await?;

    if metadata.roles.contains_key(&role_id) {
        let update = doc! {"$set": {format!("roles.{}.name", role_id): &body.name}};
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let updated_metadata = app
            .project_metadata
            .find_one_and_update(query, update, options)
            .await
            .map_err(|err| InternalError::DatabaseConnectionError(err))?
            .ok_or_else(|| UserError::ProjectNotFoundError)?;

        app.on_room_changed(updated_metadata);
        Ok(HttpResponse::Ok().body("Role updated"))
    } else {
        Err(UserError::RoleNotFoundError)
    }
}

#[get("/id/{projectID}/{roleID}/latest")]
async fn get_latest_role(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, RoleId)>,
    params: web::Query<GetProjectRoleParams>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();
    let metadata =
        ensure_can_view_project(&app, &session, params.into_inner().client_id, &project_id).await?;

    let (_, role_data) = fetch_role_data(&app, &metadata, role_id).await?;
    Ok(HttpResponse::Ok().json(role_data))
}

async fn fetch_role_data(
    app: &AppData,
    metadata: &ProjectMetadata,
    role_id: RoleId,
) -> Result<(RoleId, RoleData), UserError> {
    let role_md = metadata
        .roles
        .get(&role_id)
        .ok_or_else(|| UserError::RoleNotFoundError)?;

    // Try to fetch the role data from the current occupants
    let state = BrowserClientState {
        project_id: metadata.id.clone(),
        role_id: role_id.clone(),
    };
    let request_opt = app
        .network
        .send(topology::GetRoleRequest { state })
        .await
        .map_err(|_err| UserError::InternalError)
        .and_then(|result| result.0.ok_or_else(|| UserError::InternalError));

    let active_role = if let Ok(request) = request_opt {
        request.send().await.ok()
    } else {
        None
    };

    // If unable to retrieve role data from current occupants (unoccupied or error),
    // fetch the latest from the database
    let role_data = match active_role {
        Some(role_data) => role_data,
        None => app.fetch_role(role_md).await?,
    };
    Ok((role_id, role_data))
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RoleDataResponse {
    id: String,
    pub data: RoleData,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportRoleParams {
    client_id: Option<ClientID>,
}

#[post("/id/{projectID}/{roleID}/latest")]
async fn report_latest_role(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, RoleId)>,
    body: web::Json<RoleDataResponse>,
    params: web::Query<ReportRoleParams>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();
    let id = Uuid::parse_str(&body.id).map_err(|_err| UserError::ProjectNotFoundError)?;
    let client_id = params.into_inner().client_id;
    let metadata = ensure_can_edit_project(&app, &session, client_id, &project_id).await?;

    if !metadata.roles.contains_key(&role_id) {
        return Err(UserError::RoleNotFoundError);
    }

    app.network.do_send(topology::RoleDataResponse {
        id,
        data: body.into_inner().data,
    });
    Ok(HttpResponse::Ok().finish())
}

#[get("/id/{projectID}/collaborators/")]
async fn list_collaborators(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let metadata = ensure_can_edit_project(&app, &session, None, &project_id).await?;

    Ok(HttpResponse::Ok().json(metadata.collaborators))
}

#[delete("/id/{projectID}/collaborators/{username}")]
async fn remove_collaborator(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (project_id, username) = path.into_inner();
    let query = doc! {"id": &project_id};
    ensure_can_edit_project(&app, &session, None, &project_id).await?;

    let update = doc! {"$pull": {"collaborators": &username}};
    let options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let metadata = app
        .project_metadata
        .find_one_and_update(query, update, options)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    app.on_room_changed(metadata);
    Ok(HttpResponse::Ok().body("Collaborator removed"))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_project)
        .service(update_project)
        .service(delete_project)
        .service(list_user_projects)
        .service(list_shared_projects)
        .service(get_project)
        .service(get_project_named)
        .service(get_project_metadata)
        .service(get_project_id_metadata)
        .service(publish_project)
        .service(unpublish_project)
        .service(get_latest_project)
        .service(get_project_thumbnail)
        .service(get_role)
        .service(get_latest_role)
        .service(report_latest_role)
        .service(create_role)
        .service(save_role)
        .service(rename_role)
        .service(delete_role)
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
        // TODO: retrieves unsaved changes
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
