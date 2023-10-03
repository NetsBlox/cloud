use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};

use crate::auth;
use crate::common::api;
use crate::groups::actions::GroupActions;
use crate::services::settings::actions::SettingsActions;
use crate::users::actions::UserActions;
use crate::{app_data::AppData, errors::UserError};

#[get("/user/{username}/")]
async fn list_user_hosts_with_settings(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_vu = auth::try_view_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.as_user_actions();
    let settings = actions.get_service_settings(&auth_vu).await?;
    let hosts: Vec<_> = settings.keys().collect();

    Ok(HttpResponse::Ok().json(hosts))
}

#[get("/user/{username}/{host}")]
async fn get_user_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    let auth_vu = auth::try_view_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.as_user_actions();
    let settings = actions.get_service_settings(&auth_vu).await?;
    let empty_string = String::default();
    let settings = settings.get(&host).unwrap_or(&empty_string).to_owned();

    Ok(HttpResponse::Ok().body(settings))
}

#[get("/user/{username}/{host}/all")]
async fn get_all_settings(
    req: HttpRequest,
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();

    let auth_vu = auth::try_view_user(&app, &req, None, &username).await?;
    let actions: SettingsActions = app.as_settings_actions();
    let all_settings = actions.get_settings(&auth_vu, &host).await?;

    Ok(HttpResponse::Ok().json(all_settings))
}

#[post("/user/{username}/{host}")]
async fn set_user_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    body: web::Bytes,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &username).await?;

    let settings = std::str::from_utf8(&body).map_err(|_err| UserError::InternalError)?;

    let actions: UserActions = app.as_user_actions();
    actions.set_user_settings(&auth_eu, &host, settings).await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("/user/{username}/{host}")]
async fn delete_user_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.as_user_actions();
    actions.delete_user_settings(&auth_eu, &host).await?;

    Ok(HttpResponse::Ok().finish())
}

#[get("/group/{group_id}/")]
async fn list_group_hosts_with_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id,) = path.into_inner();

    let auth_vg = auth::try_view_group(&app, &req, &group_id).await?;

    let actions: GroupActions = app.as_group_actions();
    let settings = actions.get_service_settings(&auth_vg).await?;
    let hosts: Vec<_> = settings.keys().collect();

    Ok(HttpResponse::Ok().json(hosts))
}

#[get("/group/{group_id}/{host}")]
async fn get_group_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, host) = path.into_inner();
    let auth_vg = auth::try_view_group(&app, &req, &group_id).await?;

    let actions: GroupActions = app.as_group_actions();
    let settings = actions.get_service_settings(&auth_vg).await?;

    let empty_string = String::default();
    let settings = settings.get(&host).unwrap_or(&empty_string).to_owned();

    Ok(HttpResponse::Ok().body(settings))
}

#[post("/group/{group_id}/{host}")]
async fn set_group_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, String)>,
    body: web::Bytes,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, host) = path.into_inner();
    let settings = std::str::from_utf8(&body).map_err(|_err| UserError::InternalError)?;

    let auth_eg = auth::try_edit_group(&app, &req, &group_id).await?;

    let actions: GroupActions = app.as_group_actions();
    actions
        .set_service_settings(&auth_eg, &host, &settings)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("/group/{group_id}/{host}")]
async fn delete_group_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, host) = path.into_inner();

    let auth_eg = auth::try_edit_group(&app, &req, &group_id).await?;

    let actions: GroupActions = app.as_group_actions();
    actions.delete_service_settings(&auth_eg, &host).await?;

    Ok(HttpResponse::Ok().finish())
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(get_user_settings)
        .service(set_user_settings)
        .service(list_user_hosts_with_settings)
        .service(get_all_settings)
        .service(delete_user_settings)
        .service(get_group_settings)
        .service(set_group_settings)
        .service(list_group_hosts_with_settings)
        .service(delete_group_settings);
}

// TODO: add test for setting service settings from authorized host
// TODO: only can edit it's own settings, right?
// TODO: add test for delete service settings from authorized host
