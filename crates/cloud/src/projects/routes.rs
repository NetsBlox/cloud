use crate::app_data::AppData;
use crate::common::api;
use crate::common::api::{
    ClientId, CreateProjectData, ProjectId, RoleData, RoleId, UpdateProjectData, UpdateRoleData,
};
use crate::errors::{InternalError, UserError};
use crate::projects::actions::ProjectActions;
use crate::{auth, utils};
use actix_web::{delete, get, patch, post, HttpRequest};
use actix_web::{web, HttpResponse};
use mongodb::bson::doc;
use serde::Deserialize;

#[post("/")]
async fn create_project(
    app: web::Data<AppData>,
    body: web::Json<CreateProjectData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let project_data = body.into_inner();

    let current_user = utils::get_username(&req);
    let client_id = project_data.client_id.clone();
    let owner = project_data
        .owner
        .clone()
        .or(current_user)
        .or_else(|| client_id.clone().map(|id| id.as_str().to_string()))
        .ok_or(UserError::LoginRequiredError)?;

    let auth_eu = auth::try_edit_user(&app, &req, client_id.as_ref(), &owner).await?;
    let actions: ProjectActions = app.as_project_actions();
    let metadata = actions.create_project(&auth_eu, project_data).await?;

    Ok(HttpResponse::Ok().json(metadata))
}

#[get("/user/{owner}")]
async fn list_user_projects(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_lp = auth::try_list_projects(&app, &req, &username).await?;

    let actions: ProjectActions = app.as_project_actions();
    let projects = actions.list_projects(&auth_lp).await?;

    Ok(HttpResponse::Ok().json(projects))
}

#[get("/shared/{username}")]
async fn list_shared_projects(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_lp = auth::try_list_projects(&app, &req, &username).await?;

    let actions: ProjectActions = app.as_project_actions();
    let projects = actions.list_shared_projects(&auth_lp).await?;

    Ok(HttpResponse::Ok().json(projects))
}

#[get("/user/{owner}/{name}")]
async fn get_project_named(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let query = doc! {"owner": owner, "name": name};
    let metadata = app
        .project_metadata
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::ProjectNotFoundError)?;

    let auth_vp = auth::try_view_project(&app, &req, None, &metadata.id).await?;
    let actions: ProjectActions = app.as_project_actions();

    let project = actions.get_project(&auth_vp).await?;
    Ok(HttpResponse::Ok().json(project))
}

#[get("/user/{owner}/{name}/metadata")]
async fn get_project_metadata(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let query = doc! {"owner": owner, "name": name};
    let metadata = app
        .project_metadata
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::ProjectNotFoundError)?;

    let auth_vp = auth::try_view_project(&app, &req, None, &metadata.id).await?;

    let actions: ProjectActions = app.as_project_actions();
    let metadata = actions.get_project_metadata(&auth_vp);

    Ok(HttpResponse::Ok().json(metadata))
}

#[get("/id/{id}/metadata")]
async fn get_project_id_metadata(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let auth_vp = auth::try_view_project(&app, &req, None, &project_id).await?;

    let actions: ProjectActions = app.as_project_actions();
    let metadata = actions.get_project_metadata(&auth_vp);

    Ok(HttpResponse::Ok().json(metadata))
}

#[get("/id/{projectID}")]
async fn get_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let auth_vp = auth::try_view_project(&app, &req, None, &project_id).await?;

    let actions: ProjectActions = app.as_project_actions();
    let project = actions.get_project(&auth_vp).await?;

    Ok(HttpResponse::Ok().json(project))
}

#[post("/id/{projectID}/publish")]
async fn publish_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let auth_ep = auth::try_edit_project(&app, &req, None, &project_id).await?;
    let actions: ProjectActions = app.as_project_actions();
    let state = actions.publish_project(&auth_ep).await?;
    Ok(HttpResponse::Ok().json(state))
}

#[post("/id/{projectID}/unpublish")]
async fn unpublish_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let auth_ep = auth::try_edit_project(&app, &req, None, &project_id).await?;
    let actions: ProjectActions = app.as_project_actions();
    let state = actions.unpublish_project(&auth_ep).await?;
    Ok(HttpResponse::Ok().json(state))
}

#[delete("/id/{projectID}")]
async fn delete_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let auth_dp = auth::try_delete_project(&app, &req, None, &project_id).await?;
    let actions: ProjectActions = app.as_project_actions();
    let project = actions.delete_project(&auth_dp).await?;

    Ok(HttpResponse::Ok().json(project))
}

#[patch("/id/{projectID}")]
async fn update_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    body: web::Json<UpdateProjectData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();

    let body = body.into_inner();
    let auth_ep = auth::try_edit_project(&app, &req, body.client_id, &project_id).await?;

    let actions: ProjectActions = app.as_project_actions();
    let metadata = actions.rename_project(&auth_ep, &body.name).await?;
    Ok(HttpResponse::Ok().json(metadata))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetProjectRoleParams {
    // FIXME: this isn't really secure since it is easy to spoof the client ID
    client_id: Option<ClientId>,
}

#[get("/id/{projectID}/latest")]
async fn get_latest_project(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    params: web::Query<GetProjectRoleParams>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let params = params.into_inner();
    let auth_vp =
        auth::try_view_project(&app, &req, params.client_id.as_ref(), &project_id).await?;
    let actions: ProjectActions = app.as_project_actions();
    let project = actions.get_latest_project(&auth_vp).await?;
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
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let auth_vp = auth::try_view_project(&app, &req, None, &project_id).await?;

    let actions: ProjectActions = app.as_project_actions();
    let thumbnail = actions
        .get_project_thumbnail(&auth_vp, params.aspect_ratio)
        .await?;

    Ok(HttpResponse::Ok().content_type("image/png").body(thumbnail))
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
            code: data.code.unwrap_or_default(),
            media: data.media.unwrap_or_default(),
        }
    }
}

#[post("/id/{projectID}/")]
async fn create_role(
    app: web::Data<AppData>,
    body: web::Json<CreateRoleData>,
    path: web::Path<(ProjectId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let auth_ep = auth::try_edit_project(&app, &req, None, &project_id).await?;

    let actions: ProjectActions = app.as_project_actions();
    let updated_metadata = actions
        .create_role(&auth_ep, body.into_inner().into())
        .await?;

    Ok(HttpResponse::Ok().json(updated_metadata))
}

#[get("/id/{projectID}/{roleID}")]
async fn get_role(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, RoleId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();
    let auth_vp = auth::try_view_project(&app, &req, None, &project_id).await?;

    let actions: ProjectActions = app.as_project_actions();
    let role = actions.get_role(&auth_vp, role_id).await?;

    Ok(HttpResponse::Ok().json(role))
}

#[delete("/id/{projectID}/{roleID}")]
async fn delete_role(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, RoleId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();

    let auth_ep = auth::try_edit_project(&app, &req, None, &project_id).await?;

    let actions: ProjectActions = app.as_project_actions();
    let metadata = actions.delete_role(&auth_ep, role_id).await?;

    Ok(HttpResponse::Ok().json(metadata))
}

#[post("/id/{projectID}/{roleID}")]
async fn save_role(
    app: web::Data<AppData>,
    body: web::Json<RoleData>,
    path: web::Path<(ProjectId, RoleId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();
    let auth_ep = auth::try_edit_project(&app, &req, None, &project_id).await?;
    let actions: ProjectActions = app.as_project_actions();
    let metadata = actions
        .save_role(&auth_ep, &role_id, body.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(metadata))
}

#[patch("/id/{projectID}/{roleID}")]
async fn rename_role(
    app: web::Data<AppData>,
    body: web::Json<UpdateRoleData>,
    path: web::Path<(ProjectId, RoleId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();
    let body = body.into_inner();
    let auth_ep = auth::try_edit_project(&app, &req, body.client_id, &project_id).await?;

    let actions: ProjectActions = app.as_project_actions();
    let metadata = actions.rename_role(&auth_ep, role_id, &body.name).await?;
    Ok(HttpResponse::Ok().json(metadata))
}

#[get("/id/{projectID}/{roleID}/latest")]
async fn get_latest_role(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, RoleId)>,
    params: web::Query<GetProjectRoleParams>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();
    let params = params.into_inner();
    let auth_vp =
        auth::try_view_project(&app, &req, params.client_id.as_ref(), &project_id).await?;

    let actions: ProjectActions = app.as_project_actions();
    let (_, role_data) = actions.fetch_role_data(&auth_vp, role_id).await?;

    Ok(HttpResponse::Ok().json(role_data))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportRoleParams {
    client_id: Option<ClientId>,
}

#[post("/id/{projectID}/{roleID}/latest")]
async fn report_latest_role(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, RoleId)>,
    body: web::Json<api::RoleDataResponse>,
    params: web::Query<ReportRoleParams>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id, role_id) = path.into_inner();
    let client_id = params.into_inner().client_id;
    let auth_ep = auth::try_edit_project(&app, &req, client_id, &project_id).await?;
    let actions: ProjectActions = app.as_project_actions();
    let resp = body.into_inner();
    actions
        .set_latest_role(&auth_ep, &role_id, &resp.id, resp.data)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[get("/id/{projectID}/collaborators/")]
async fn list_collaborators(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id,) = path.into_inner();
    let metadata = auth::try_view_project(&app, &req, None, &project_id).await?;

    let actions: ProjectActions = app.as_project_actions();
    let collaborators = actions.get_collaborators(&metadata);

    Ok(HttpResponse::Ok().json(collaborators))
}

#[delete("/id/{projectID}/collaborators/{username}")]
async fn remove_collaborator(
    app: web::Data<AppData>,
    path: web::Path<(ProjectId, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (project_id, username) = path.into_inner();
    let edit_proj = auth::try_edit_project(&app, &req, None, &project_id).await?;
    let actions: ProjectActions = app.as_project_actions();
    let metadata = actions.remove_collaborator(&edit_proj, &username).await?;

    Ok(HttpResponse::Ok().json(metadata))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use actix_web::{http, test, App};
    use netsblox_cloud_common::{api::UserRole, User};

    #[actix_web::test]
    #[ignore]
    async fn test_create_project() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_create_project_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_create_project_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_get_project() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let project = test_utils::project::builder()
            .with_owner(user.username.clone())
            .with_name("project name".into())
            .build();

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new(&user.username);
                let req = test::TestRequest::post()
                    .uri(&format!("/id/{}", &project.id))
                    .cookie(cookie)
                    .to_request();

                let data: api::Project = test::call_and_read_body_json(&app, req).await;
                assert_eq!(data.id, project.id);
                assert_eq!(data.name, project.name);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_get_project_403() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let other: User = api::NewUser {
            username: "other".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let project = test_utils::project::builder()
            .with_owner(user.username.clone())
            .with_name("project name".into())
            .build();

        test_utils::setup()
            .with_users(&[user.clone(), other.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new(&other.username);
                let req = test::TestRequest::post()
                    .uri(&format!("/id/{}", &project.id))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_get_project_admin() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::User),
        }
        .into();
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let project = test_utils::project::builder()
            .with_owner(user.username.clone())
            .with_name("project name".into())
            .build();

        test_utils::setup()
            .with_users(&[user.clone(), admin.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let cookie = test_utils::cookie::new(&admin.username);
                let req = test::TestRequest::post()
                    .uri(&format!("/id/{}", &project.id))
                    .cookie(cookie)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_project_named() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_project_named_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_project_named_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_latest_project() {
        // TODO: retrieves unsaved changes
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_latest_project_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_latest_project_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_project_thumbnail() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_project_thumbnail_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_project_thumbnail_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_update_project() {
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();

        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let project = test_utils::project::builder()
            .with_owner(owner.username.clone())
            .with_name("initial name".into())
            .build();
        let other_project = test_utils::project::builder()
            .with_owner("admin".into())
            .with_name("new name".into())
            .build();

        test_utils::setup()
            .with_projects(&[project.clone(), other_project])
            .with_users(&[admin.clone(), owner])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let update_data = api::UpdateProjectData {
                    name: "new name".into(),
                    client_id: None,
                };
                let req = test::TestRequest::patch()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri(&format!("/id/{}", &project.id))
                    .set_json(&update_data)
                    .to_request();

                let metadata: api::ProjectMetadata = test::call_and_read_body_json(&app, req).await;
                assert_eq!(metadata.name, update_data.name);

                // TODO: check the database is updated, too
            })
            .await;
    }

    #[actix_web::test]
    async fn test_update_project_collision() {
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();

        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let project = test_utils::project::builder()
            .with_owner(owner.username.clone())
            .with_name("initial name".into())
            .build();

        let existing = test_utils::project::builder()
            .with_owner(owner.username.clone())
            .with_name("new name".into())
            .build();

        test_utils::setup()
            .with_projects(&[project.clone(), existing.clone()])
            .with_users(&[admin.clone(), owner])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let update_data = api::UpdateProjectData {
                    name: existing.name.clone(),
                    client_id: None,
                };
                let req = test::TestRequest::patch()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri(&format!("/id/{}", &project.id))
                    .set_json(&update_data)
                    .to_request();

                let metadata: api::ProjectMetadata = test::call_and_read_body_json(&app, req).await;
                assert_ne!(metadata.name, existing.name);
                assert!(metadata.name.starts_with(&update_data.name));

                // TODO: check the database is updated, too
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_update_project_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_update_project_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_publish_project() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_publish_project_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_publish_project_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_unpublish_project() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_unpublish_project_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_unpublish_project_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_delete_project() {
        // TODO: Should the client be notified?
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_delete_project_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_delete_project_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_rename_project_owner() {
        let username = "user1";
        let project = test_utils::project::builder()
            .with_name("old_name".into())
            .with_owner(username.to_string())
            .build();
        let id = project.id.clone();
        let new_name = "new project";
        let project_update = UpdateProjectData {
            name: new_name.into(),
            client_id: None,
        };

        test_utils::setup()
            .with_projects(&[project])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::patch()
                    .cookie(test_utils::cookie::new(username))
                    .uri(&format!("/id/{}", id))
                    .set_json(&project_update)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);

                let query = doc! {"id": id};
                let project = app_data
                    .project_metadata
                    .find_one(query, None)
                    .await
                    .unwrap()
                    .unwrap();

                assert_eq!(project.name, new_name);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_rename_project_invalid_name() {
        let username = "user1";
        let project = test_utils::project::builder()
            .with_name("old name".into())
            .with_owner(username.to_string())
            .build();
        let id = project.id.clone();
        let new_name = "shit";
        let project_update = UpdateProjectData {
            name: new_name.into(),
            client_id: None,
        };

        test_utils::setup()
            .with_projects(&[project])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::patch()
                    .cookie(test_utils::cookie::new(username))
                    .uri(&format!("/id/{}", id))
                    .set_json(&project_update)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);

                let query = doc! {"id": id};
                let project = app_data
                    .project_metadata
                    .find_one(query, None)
                    .await
                    .unwrap()
                    .unwrap();

                assert_eq!(project.name, "old name".to_string());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_rename_project_401() {
        let new_name = "some new name";
        let project_update = UpdateProjectData {
            name: new_name.into(),
            client_id: None,
        };
        let id = "abc123";
        let project = test_utils::project::builder()
            .with_name("old_name".into())
            .with_id(ProjectId::new(id.to_string()))
            .build();

        test_utils::setup()
            .with_projects(&[project])
            .run(|app_data| async {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::patch()
                    .uri(&format!("/id/{}", &id))
                    .set_json(&project_update)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_rename_project_admin() {
        let owner: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let admin: User = api::NewUser {
            username: "admin".to_string(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let project = test_utils::project::builder()
            .with_name("old_name".into())
            .with_owner(owner.username.clone())
            .build();
        let id = project.id.clone();
        let new_name = "new project";
        let project_update = UpdateProjectData {
            name: new_name.into(),
            client_id: None,
        };

        test_utils::setup()
            .with_projects(&[project])
            .with_users(&[owner, admin.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::patch()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri(&format!("/id/{}", id))
                    .set_json(&project_update)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_list_user_projects() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_list_user_projects_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_list_user_projects_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_view_shared_projects() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_view_shared_projects_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_view_shared_projects_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_create_role() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_create_role_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_create_role_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_create_role_room_update() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_role() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_role_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_role_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_latest_role() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_latest_role_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_get_latest_role_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_rename_role() {
        let user: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let role_id = api::RoleId::new("someRole".into());
        let role_data = api::RoleData {
            name: "role".into(),
            code: "<code/>".into(),
            media: "<media/>".into(),
        };
        let project = test_utils::project::builder()
            .with_owner(user.username.to_string())
            .with_roles([(role_id.clone(), role_data)].into_iter().collect())
            .build();

        test_utils::setup()
            .with_projects(&[project.clone()])
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let data = UpdateRoleData {
                    name: "new_name".into(),
                    client_id: None,
                };
                let req = test::TestRequest::patch()
                    .cookie(test_utils::cookie::new(&user.username))
                    .uri(&format!("/id/{}/{}", &project.id, &role_id))
                    .set_json(&data)
                    .to_request();

                let project: api::ProjectMetadata = test::call_and_read_body_json(&app, req).await;
                let role = project.roles.get(&role_id).unwrap();
                assert_eq!(role.name, data.name);

                let project = app_data.get_project_metadatum(&project.id).await.unwrap();
                let role = project.roles.get(&role_id).unwrap();
                assert_eq!(role.name, data.name);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_rename_role_invalid_name() {
        let username = "user1";
        let role_id = api::RoleId::new("someRole".into());
        let role_data = api::RoleData {
            name: "role".into(),
            code: "<code/>".into(),
            media: "<media/>".into(),
        };
        let project = test_utils::project::builder()
            .with_owner(username.to_string())
            .with_collaborators(&["user2", "user3"])
            .with_roles([(role_id.clone(), role_data)].into_iter().collect())
            .build();

        test_utils::setup()
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let data = UpdateRoleData {
                    name: "$ .1 damn".into(),
                    client_id: None,
                };
                let req = test::TestRequest::patch()
                    .cookie(test_utils::cookie::new(username))
                    .uri(&format!("/id/{}/{}", &project.id, &role_id))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);

                let project = app_data.get_project_metadatum(&project.id).await.unwrap();
                let role = project.roles.get(&role_id).unwrap();
                assert_eq!(role.name, "role".to_string());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_rename_role_no_perms() {
        let username = "user1";
        let role_id = api::RoleId::new("someRole".into());
        let role_data = api::RoleData {
            name: "role".into(),
            code: "<code/>".into(),
            media: "<media/>".into(),
        };
        let project = test_utils::project::builder()
            .with_owner("owner".to_string())
            .with_collaborators(&["user2", "user3"])
            .with_roles([(role_id.clone(), role_data)].into_iter().collect())
            .build();

        test_utils::setup()
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let data = UpdateRoleData {
                    name: "X".into(),
                    client_id: None,
                };
                let req = test::TestRequest::patch()
                    .cookie(test_utils::cookie::new(username))
                    .uri(&format!("/id/{}/{}", &project.id, &role_id))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);

                let project = app_data.get_project_metadatum(&project.id).await.unwrap();
                let role = project.roles.get(&role_id).unwrap();
                assert_eq!(role.name, "role".to_string());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_rename_role_admin() {
        let admin: User = api::NewUser {
            username: "admin".to_string(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();

        let role_id = api::RoleId::new("someRole".into());
        let role_data = api::RoleData {
            name: "role".into(),
            code: "<code/>".into(),
            media: "<media/>".into(),
        };
        let project = test_utils::project::builder()
            .with_owner("owner".to_string())
            .with_roles([(role_id.clone(), role_data)].into_iter().collect())
            .build();

        test_utils::setup()
            .with_users(&[admin.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let data = UpdateRoleData {
                    name: "new_name".into(),
                    client_id: None,
                };
                let req = test::TestRequest::patch()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri(&format!("/id/{}/{}", &project.id, &role_id))
                    .set_json(&data)
                    .to_request();

                let project: api::ProjectMetadata = test::call_and_read_body_json(&app, req).await;
                let role = project.roles.get(&role_id).unwrap();
                assert_eq!(role.name, data.name);

                let project = app_data.get_project_metadatum(&project.id).await.unwrap();
                let role = project.roles.get(&role_id).unwrap();
                assert_eq!(role.name, "new_name".to_string());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_save_role() {
        let user: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let role_id = api::RoleId::new("someRole".into());
        let role_data = api::RoleData {
            name: "role".into(),
            code: "<code/>".into(),
            media: "<media/>".into(),
        };
        let project = test_utils::project::builder()
            .with_owner(user.username.to_string())
            .with_roles([(role_id.clone(), role_data)].into_iter().collect())
            .build();

        test_utils::setup()
            .with_projects(&[project.clone()])
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let data = api::RoleData {
                    name: "new name".into(),
                    code: "<new code/>".into(),
                    media: "<new media/>".into(),
                };
                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&user.username))
                    .uri(&format!("/id/{}/{}", &project.id, &role_id))
                    .set_json(&data)
                    .to_request();

                let project: api::ProjectMetadata = test::call_and_read_body_json(&app, req).await;
                let role = project.roles.get(&role_id).unwrap();
                assert_eq!(&role.name, "new name");
                assert_eq!(&role.code, "<new code/>");
                assert_eq!(&role.media, "<new media/>");
            })
            .await;
    }

    #[actix_web::test]
    async fn test_save_role_403() {
        let user: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let other: User = api::NewUser {
            username: "other".to_string(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let role_id = api::RoleId::new("someRole".into());
        let role_data = api::RoleData {
            name: "role".into(),
            code: "<code/>".into(),
            media: "<media/>".into(),
        };
        let project = test_utils::project::builder()
            .with_owner(user.username.to_string())
            .with_roles([(role_id.clone(), role_data)].into_iter().collect())
            .build();

        test_utils::setup()
            .with_projects(&[project.clone()])
            .with_users(&[user.clone(), other.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let data = api::RoleData {
                    name: "new name".into(),
                    code: "<new code/>".into(),
                    media: "<new media/>".into(),
                };
                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&other.username))
                    .uri(&format!("/id/{}/{}", &project.id, &role_id))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_save_role_admin() {
        let user: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let admin: User = api::NewUser {
            username: "admin".to_string(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let role_id = api::RoleId::new("someRole".into());
        let role_data = api::RoleData {
            name: "role".into(),
            code: "<code/>".into(),
            media: "<media/>".into(),
        };
        let project = test_utils::project::builder()
            .with_owner(user.username.to_string())
            .with_roles([(role_id.clone(), role_data)].into_iter().collect())
            .build();

        test_utils::setup()
            .with_projects(&[project.clone()])
            .with_users(&[user.clone(), admin.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let data = api::RoleData {
                    name: "new name".into(),
                    code: "<new code/>".into(),
                    media: "<new media/>".into(),
                };
                let req = test::TestRequest::post()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri(&format!("/id/{}/{}", &project.id, &role_id))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_role() {
        let user: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let role_id = api::RoleId::new("someRole".into());
        let role_data = api::RoleData {
            name: "role".into(),
            code: "<code/>".into(),
            media: "<media/>".into(),
        };
        let role2_id = api::RoleId::new("role2Id".into());
        let role2_data = api::RoleData {
            name: "role2".into(),
            code: "<code/>".into(),
            media: "<media/>".into(),
        };
        let project = test_utils::project::builder()
            .with_owner(user.username.to_string())
            .with_roles(
                [(role_id.clone(), role_data), (role2_id, role2_data)]
                    .into_iter()
                    .collect(),
            )
            .build();

        test_utils::setup()
            .with_projects(&[project.clone()])
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .cookie(test_utils::cookie::new(&user.username))
                    .uri(&format!("/id/{}/{}", &project.id, &role_id))
                    .to_request();

                let project: api::ProjectMetadata = test::call_and_read_body_json(&app, req).await;
                let role_opt = project.roles.get(&role_id);
                assert!(role_opt.is_none(), "Role not deleted");

                let role2_opt = project.roles.get(&role2_id);
                assert!(role2_opt.is_some(), "Other role deleted!");
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_delete_role_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_delete_role_admin() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_delete_role_room_update() {
        todo!();
    }

    #[actix_web::test]
    async fn test_remove_collaborator() {
        let username = "user1";
        let project = test_utils::project::builder()
            .with_owner(username.to_string())
            .with_collaborators(&["user2", "user3"])
            .build();

        test_utils::setup()
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .cookie(test_utils::cookie::new(username))
                    .uri(&format!("/id/{}/collaborators/user2", &project.id))
                    .to_request();

                let project: api::ProjectMetadata = test::call_and_read_body_json(&app, req).await;
                let expected = ["user3"];
                project
                    .collaborators
                    .into_iter()
                    .enumerate()
                    .for_each(|(i, name)| assert_eq!(name, expected[i]));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_remove_collaborator_invalid_name() {
        let owner: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let project = test_utils::project::builder()
            .with_owner(owner.username.clone())
            .with_collaborators(&["user2", "user3"])
            .build();

        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri(&format!(
                        "/id/{}/collaborators/notACollaborator",
                        &project.id
                    ))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_remove_collaborator_403() {
        let owner: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let other: User = api::NewUser {
            username: "other".to_string(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        let project = test_utils::project::builder()
            .with_owner(owner.username.clone())
            .with_collaborators(&["user2", "user3"])
            .build();

        test_utils::setup()
            .with_users(&[owner, other.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .cookie(test_utils::cookie::new(&other.username))
                    .uri(&format!("/id/{}/collaborators/user2", &project.id))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_remove_collaborator_admin() {
        let owner: User = api::NewUser {
            username: "owner".to_string(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let admin: User = api::NewUser {
            username: "admin".to_string(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();

        let project = test_utils::project::builder()
            .with_owner(owner.username.clone())
            .with_collaborators(&["user2", "user3"])
            .build();

        test_utils::setup()
            .with_users(&[owner, admin.clone()])
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri(&format!("/id/{}/collaborators/user2", &project.id))
                    .to_request();

                let project: api::ProjectMetadata = test::call_and_read_body_json(&app, req).await;
                let expected = ["user3"];
                project
                    .collaborators
                    .into_iter()
                    .enumerate()
                    .for_each(|(i, name)| assert_eq!(name, expected[i]));
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_remove_collaborator_room_update() {
        todo!();
    }

    #[actix_web::test]
    async fn test_list_collaborators() {
        let username = "user1";
        let project = test_utils::project::builder()
            .with_owner(username.to_string())
            .with_collaborators(&["user2", "user3"])
            .build();

        test_utils::setup()
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(username))
                    .uri(&format!("/id/{}/collaborators/", &project.id))
                    .to_request();

                let collaborators: Vec<String> = test::call_and_read_body_json(&app, req).await;
                collaborators
                    .into_iter()
                    .enumerate()
                    .for_each(|(i, name)| assert_eq!(name, project.collaborators[i]));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_list_collaborators_403() {
        let username = "user1";
        let project = test_utils::project::builder()
            .with_owner(username.to_string())
            .with_collaborators(&["user2", "user3"])
            .build();

        test_utils::setup()
            .with_projects(&[project.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/id/{}/collaborators/", &project.id))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_list_collaborators_admin() {
        let admin: User = api::NewUser {
            username: "admin".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();

        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "admin@netsblox.org".into(),
            password: None,
            group_id: None,
            role: Some(UserRole::Admin),
        }
        .into();
        let project = test_utils::project::builder()
            .with_owner(owner.username.clone())
            .with_collaborators(&["user2", "user3"])
            .build();

        test_utils::setup()
            .with_projects(&[project.clone()])
            .with_users(&[admin.clone(), owner])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri(&format!("/id/{}/collaborators/", &project.id))
                    .to_request();

                let collaborators: Vec<String> = test::call_and_read_body_json(&app, req).await;
                collaborators
                    .into_iter()
                    .enumerate()
                    .for_each(|(i, name)| assert_eq!(name, project.collaborators[i]));
            })
            .await;
    }
}
