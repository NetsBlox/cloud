use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};

use crate::common::api;
use crate::groups::actions::GroupActions;
use crate::services::settings::actions::SettingsActions;
use crate::users::actions::UserActions;
use crate::{app_data::AppData, errors::UserError};
use crate::{auth, utils};

#[get("/user/{username}/")]
async fn list_user_hosts_with_settings(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_vu = auth::try_view_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.as_user_actions();
    let settings = actions.get_user_settings(&auth_vu).await?;
    let hosts: Vec<_> = settings.keys().collect();

    Ok(HttpResponse::Ok().json(hosts))
}

#[get("/user/{username}/{host}")]
async fn get_user_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, api::ServiceHostId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    let auth_vu = auth::try_view_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.as_user_actions();
    let mut user_settings = actions.get_user_settings(&auth_vu).await?;
    let mut default_settings = api::ServiceHostSettings::default();
    let settings = user_settings
        .get_mut(&host)
        .unwrap_or(&mut default_settings);
    let redacted = utils::redact_service_setting_secrets(&app, &req, &host, settings).await?;

    Ok(HttpResponse::Ok().json(redacted))
    //
}

#[get("/user/{username}/{host}/all")]
async fn get_all_settings(
    req: HttpRequest,
    app: web::Data<AppData>,
    path: web::Path<(String, api::ServiceHostId)>,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();

    let auth_vu = auth::try_view_user(&app, &req, None, &username).await?;
    let actions: SettingsActions = app.as_settings_actions();
    let mut all_settings = actions.get_settings(&auth_vu, &host).await?;
    if let Some(user_settings) = all_settings.user.as_mut() {
        utils::redact_service_setting_secrets(&app, &req, &host, user_settings).await?;
    }
    if let Some(member_settings) = all_settings.member.as_mut() {
        utils::redact_service_setting_secrets(&app, &req, &host, member_settings).await?;
    }
    Ok(HttpResponse::Ok().json(all_settings))
}

#[post("/user/{username}/{host}")]
async fn set_user_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, api::ServiceHostId)>,
    body: web::Json<api::ServiceHostSettings>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &username).await?;

    let settings = body.into_inner();

    let actions: UserActions = app.as_user_actions();
    actions
        .set_user_settings(&auth_eu, &host, &settings)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("/user/{username}/{host}")]
async fn delete_user_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, api::ServiceHostId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.as_user_actions();
    actions.delete_user_settings(&auth_eu, &host).await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("/user/{username}/{host}/{service}")]
async fn delete_user_service_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, api::ServiceHostId, api::ServiceName)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username, host, service_name) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &username).await?;

    let actions: UserActions = app.as_user_actions();
    actions.delete_user_service_settings(&auth_eu, &host, &service_name).await?;

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
    path: web::Path<(api::GroupId, api::ServiceHostId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, host) = path.into_inner();
    let auth_vg = auth::try_view_group(&app, &req, &group_id).await?;

    let actions: GroupActions = app.as_group_actions();

    let mut group_settings = actions.get_service_settings(&auth_vg).await?;
    let mut default_settings = api::ServiceHostSettings::default();

    let settings = group_settings
        .get_mut(&host)
        .unwrap_or(&mut default_settings);
    let redacted = utils::redact_service_setting_secrets(&app, &req, &host, settings).await?;

    Ok(HttpResponse::Ok().json(redacted))
}

#[post("/group/{group_id}/{host}")]
async fn set_group_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::ServiceHostId)>,
    body: web::Json<api::ServiceHostSettings>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, host) = path.into_inner();
    let settings = body.into_inner();

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
    path: web::Path<(api::GroupId, api::ServiceHostId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, host) = path.into_inner();

    let auth_eg = auth::try_edit_group(&app, &req, &group_id).await?;

    let actions: GroupActions = app.as_group_actions();
    actions.delete_group_settings(&auth_eg, &host).await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("/group/{group_id}/{host}/{service}")]
async fn delete_group_service_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::ServiceHostId, api::ServiceName)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, host, service_name) = path.into_inner();
    let auth_eg = auth::try_edit_group(&app, &req, &group_id).await?;

    let actions: GroupActions = app.as_group_actions();
    actions.delete_group_service_settings(&auth_eg, &host, &service_name).await?;

    Ok(HttpResponse::Ok().finish())
}


pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(get_user_settings)
        .service(set_user_settings)
        .service(list_user_hosts_with_settings)
        .service(get_all_settings)
        .service(delete_user_settings)
        .service(delete_user_service_settings)
        .service(get_group_settings)
        .service(set_group_settings)
        .service(list_group_hosts_with_settings)
        .service(delete_group_settings)
        .service(delete_group_service_settings);
}

#[cfg(test)]
mod tests {
    #[actix_web::test]
    async fn test_parse() {}
}

// TODO: add test for setting service settings from authorized host
// TODO: only can edit it's own settings, right?
// TODO: add test for delete service settings from authorized host
