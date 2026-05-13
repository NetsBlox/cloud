use actix_web::{delete, get, patch, web, HttpRequest, HttpResponse};
use itertools::Itertools;

use crate::auth;
use crate::common::api;
use crate::{app_data::AppData, errors::UserError};

#[patch("/user/{username}/host/{host}")]
async fn update_user_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, api::ServiceHostId)>,
    body: web::Json<api::ServiceHostSettings>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    let settings = body.into_inner();

    let auth_us = auth::try_update_user_settings(&app, &req, username, host, settings).await?;

    app.as_settings_actions()
        .update_user_settings(&auth_us)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[get("/user/{username}")]
async fn list_user_hosts_with_settings(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_vs = auth::try_view_user_settings(&app, &req, username).await?;

    let hosts = app
        .as_settings_actions()
        .get_user_settings(&auth_vs)
        .await?
        .into_keys()
        .collect_vec();

    Ok(HttpResponse::Ok().json(hosts))
}

#[get("/user/{username}/host/{host}")]
async fn get_user_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, api::ServiceHostId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    let auth_vs = auth::try_view_user_settings(&app, &req, username).await?;

    let host_settings = app
        .as_settings_actions()
        .get_user_host_settings(&auth_vs, &host)
        .await?;

    Ok(HttpResponse::Ok().json(host_settings))
}

#[get("/user/{username}/host/{host}/all")]
async fn get_all_settings(
    req: HttpRequest,
    app: web::Data<AppData>,
    path: web::Path<(String, api::ServiceHostId)>,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();

    let auth_vs = auth::try_view_user_settings(&app, &req, username).await?;
    let all_settings = app
        .as_settings_actions()
        .get_all_settings(&auth_vs, &host)
        .await?;
    Ok(HttpResponse::Ok().json(all_settings))
}

#[delete("/user/{username}/host/{host}")]
async fn delete_user_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, api::ServiceHostId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username, host) = path.into_inner();
    let auth_ds = auth::try_delete_user_settings(&app, &req, username, host).await?;

    app.as_settings_actions()
        .delete_user_settings(&auth_ds)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("/user/{username}/host/{host}/service/{service}")]
async fn delete_all_user_service_settings(
    app: web::Data<AppData>,
    path: web::Path<(String, api::ServiceHostId, api::ServiceName)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username, host, service_name) = path.into_inner();
    let auth_ds = auth::try_delete_user_settings(&app, &req, username, host).await?;

    app.as_settings_actions()
        .delete_user_service_settings(&auth_ds, &service_name)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("/user/{username}/host/{host}/service/{service}/setting/{setting}")]
async fn delete_user_service_setting(
    app: web::Data<AppData>,
    path: web::Path<(
        String,
        api::ServiceHostId,
        api::ServiceName,
        api::SettingName,
    )>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username, host, service_name, setting_name) = path.into_inner();
    let auth_ds = auth::try_delete_user_settings(&app, &req, username, host).await?;

    app.as_settings_actions()
        .delete_user_service_setting(&auth_ds, &service_name, &setting_name)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[get("/group/{group_id}")]
async fn list_group_hosts_with_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id,) = path.into_inner();

    let auth_vgs = auth::try_view_group_settings(&app, &req, group_id).await?;

    let hosts = app
        .as_settings_actions()
        .get_group_settings(&auth_vgs)
        .await?
        .into_keys()
        .collect_vec();

    Ok(HttpResponse::Ok().json(hosts))
}

#[get("/group/{group_id}/host/{host}")]
async fn get_group_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::ServiceHostId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, host) = path.into_inner();
    let auth_vgs = auth::try_view_group_settings(&app, &req, group_id).await?;

    let group_settings = app
        .as_settings_actions()
        .get_group_host_settings(&auth_vgs, &host)
        .await?;

    Ok(HttpResponse::Ok().json(group_settings))
}

#[patch("/group/{group_id}/host/{host}")]
async fn set_group_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::ServiceHostId)>,
    body: web::Json<api::ServiceHostSettings>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, host) = path.into_inner();
    let update = body.into_inner();

    let auth_ugs = auth::try_update_group_settings(&app, &req, group_id, host, update).await?;

    app.as_settings_actions()
        .update_group_settings(&auth_ugs)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("/group/{group_id}/host/{host}")]
async fn delete_group_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::ServiceHostId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, host) = path.into_inner();

    let auth_dgs = auth::try_delete_group_settings(&app, &req, group_id, host).await?;

    app.as_settings_actions()
        .delete_group_settings(&auth_dgs)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("/group/{group_id}/host/{host}/service/{service}")]
async fn delete_all_group_service_settings(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::ServiceHostId, api::ServiceName)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, host, service_name) = path.into_inner();
    let auth_dgs = auth::try_delete_group_settings(&app, &req, group_id, host).await?;

    app.as_settings_actions()
        .delete_group_service_settings(&auth_dgs, &service_name)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[delete("/group/{group_id}/host/{host}/service/{service}/setting/{setting}")]
async fn delete_group_service_setting(
    app: web::Data<AppData>,
    path: web::Path<(
        api::GroupId,
        api::ServiceHostId,
        api::ServiceName,
        api::SettingName,
    )>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, host, service_name, setting_name) = path.into_inner();
    let auth_dgs = auth::try_delete_group_settings(&app, &req, group_id, host).await?;

    app.as_settings_actions()
        .delete_group_service_setting(&auth_dgs, &service_name, &setting_name)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(get_user_settings)
        .service(update_user_settings)
        .service(list_user_hosts_with_settings)
        .service(get_all_settings)
        .service(delete_user_settings)
        .service(delete_all_user_service_settings)
        .service(delete_user_service_setting)
        .service(get_group_settings)
        .service(set_group_settings)
        .service(list_group_hosts_with_settings)
        .service(delete_group_settings)
        .service(delete_all_group_service_settings)
        .service(delete_group_service_setting);
}

#[cfg(test)]
mod tests {
    use super::config;
    use crate::test_utils;
    use crate::test_utils::service_settings::test_settings;
    use crate::test_utils::test_defaults as __default;
    use actix_web::web;
    use actix_web::{http, test, App};
    use netsblox_cloud_common::api;
    use netsblox_cloud_common::api::test_utils::TestFrom;
    use netsblox_cloud_common::api::ServiceHostSettings;
    use netsblox_cloud_common::{AuthorizedServiceHost, User};
    use std::slice;

    #[actix_web::test]
    async fn test_view_host_list() {
        let username = "username";
        let mut user: User = __default::default_user(username);

        let host = api::ServiceHostId::__from("host");
        let service = api::ServiceName::__from("service");
        let setting = api::SettingName::__from("setting");
        let value = api::SettingValue::new("not_super_secret", api::SettingVisiblity::Public);
        user.service_settings = Some(test_settings(&host, &service, &setting, &value));

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let endpoint = format!("/user/{username}");
                let req = test::TestRequest::get()
                    .uri(&endpoint)
                    .cookie(test_utils::cookie::new(username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
                let data: Vec<api::ServiceHostId> = test::read_body_json(response).await;

                assert_eq!(data, vec![host]);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_host_list_other_403() {
        let othername = "other";
        let other: User = __default::default_user(othername);
        let username = "username";
        let mut user: User = __default::default_user(username);

        let host = api::ServiceHostId::__from("host");
        let service = api::ServiceName::__from("service");
        let setting = api::SettingName::__from("setting");
        let value = api::SettingValue::new("not_super_secret", api::SettingVisiblity::Public);
        user.service_settings = Some(test_settings(&host, &service, &setting, &value));

        test_utils::setup()
            .with_users(&[user.clone(), other.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let endpoint = format!("/user/{username}");
                let req = test::TestRequest::get()
                    .uri(&endpoint)
                    .cookie(test_utils::cookie::new(othername))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_host_list_admin() {
        let admin_name = "admin";
        let admin = __default::default_admin(admin_name);

        let username = "username";
        let mut user: User = __default::default_user(username);

        let host = api::ServiceHostId::__from("host");
        let service = api::ServiceName::__from("service");
        let setting = api::SettingName::__from("setting");
        let value = api::SettingValue::new("not_super_secret", api::SettingVisiblity::Public);
        user.service_settings = Some(test_settings(&host, &service, &setting, &value));

        test_utils::setup()
            .with_users(&[user.clone(), admin.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let endpoint = format!("/user/{username}");
                let req = test::TestRequest::get()
                    .uri(&endpoint)
                    .cookie(test_utils::cookie::new(admin_name))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
                let data: Vec<api::ServiceHostId> = test::read_body_json(response).await;

                assert_eq!(data, vec![host]);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_restricted_settings() {
        let username = "username";
        let mut user: User = __default::default_user(username);

        let host = api::ServiceHostId::__from("host");
        let service = api::ServiceName::__from("service");
        let setting = api::SettingName::__from("setting");
        let value = api::SettingValue::new("super_secret", api::SettingVisiblity::Restricted);
        user.service_settings = Some(test_settings(&host, &service, &setting, &value));

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let endpoint = format!("/user/{username}/host/{host}");
                let req = test::TestRequest::get()
                    .uri(&endpoint)
                    .cookie(test_utils::cookie::new(username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
                let mut data: ServiceHostSettings = test::read_body_json(response).await;

                let mut settings = data
                    .as_mut()
                    .remove(&service)
                    .expect("Service not found in services Map");
                let setting_value = settings
                    .remove(&setting)
                    .expect("Setting not found in settings Map")
                    .value;

                assert_eq!(setting_value, value.redacted());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_restricted_settings_as_host() {
        let username = "username";
        let mut user: User = __default::default_user(username);

        let host_id = api::ServiceHostId::__from("host");
        let service = api::ServiceName::__from("service");
        let setting = api::SettingName::__from("setting");
        let value = api::SettingValue::new("super_secret", api::SettingVisiblity::Restricted);
        user.service_settings = Some(test_settings(&host_id, &service, &setting, &value));

        let visibility = api::ServiceHostScope::Public(Vec::new());
        let host = AuthorizedServiceHost::new(String::from("url"), host_id.clone(), visibility);

        test_utils::setup()
            .with_users(slice::from_ref(&user))
            .with_authorized_services(slice::from_ref(&host))
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let endpoint = format!("/user/{username}/host/{host_id}");
                let req = test::TestRequest::get()
                    .uri(&endpoint)
                    .insert_header(host.auth_header())
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
                let mut data: ServiceHostSettings = test::read_body_json(response).await;

                let mut settings = data
                    .as_mut()
                    .remove(&service)
                    .expect("Service not found in services Map");
                let setting_value = settings
                    .remove(&setting)
                    .expect("Setting not found in settings Map")
                    .value;

                assert_eq!(setting_value, value.value);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_restricted_settings_as_other_host_redacted() {
        let username = "username";
        let mut user: User = __default::default_user(username);

        let host_id = api::ServiceHostId::__from("host");
        let service = api::ServiceName::__from("service");
        let setting = api::SettingName::__from("setting");
        let value = api::SettingValue::new("super_secret", api::SettingVisiblity::Restricted);
        user.service_settings = Some(test_settings(&host_id, &service, &setting, &value));

        let visibility = api::ServiceHostScope::Public(Vec::new());
        let host = AuthorizedServiceHost::new(String::from("url"), host_id.clone(), visibility.clone());

        let other_host_id = api::ServiceHostId::__from("other_host");
        let other_host = AuthorizedServiceHost::new(String::from("url"), other_host_id.clone(), visibility);

        test_utils::setup()
            .with_users(slice::from_ref(&user))
            .with_authorized_services(&[host.clone(), other_host.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let endpoint = format!("/user/{username}/host/{host_id}");
                let req = test::TestRequest::get()
                    .uri(&endpoint)
                    .insert_header(other_host.auth_header())
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
                let mut data: ServiceHostSettings = test::read_body_json(response).await;

                let mut settings = data
                    .as_mut()
                    .remove(&service)
                    .expect("Service not found in services Map");
                let setting_value = settings
                    .remove(&setting)
                    .expect("Setting not found in settings Map")
                    .value;

                assert_eq!(setting_value, value.redacted());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_view_public_settings() {
        let username = "username";
        let mut user: User = __default::default_user(username);

        let host = api::ServiceHostId::__from("host");
        let service = api::ServiceName::__from("service");
        let setting = api::SettingName::__from("setting");
        let value = api::SettingValue::new("not_super_secret", api::SettingVisiblity::Public);
        user.service_settings = Some(test_settings(&host, &service, &setting, &value));

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .configure(config),
                )
                .await;

                let endpoint = format!("/user/{username}/host/{host}");
                let req = test::TestRequest::get()
                    .uri(&endpoint)
                    .cookie(test_utils::cookie::new(username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
                let mut data: ServiceHostSettings = test::read_body_json(response).await;

                let mut settings = data
                    .as_mut()
                    .remove(&service)
                    .expect("Service not found in services Map");
                let setting_value = settings
                    .remove(&setting)
                    .expect("Setting not found in settings Map")
                    .value;
                assert_eq!(setting_value, value.value);
            })
            .await;
    }
}
