use crate::app_data::AppData;
use crate::auth;
use crate::errors::UserError;
use crate::groups::actions::GroupActions;
use actix_web::{delete, get, patch, post, HttpRequest};
use actix_web::{web, HttpResponse};

use crate::common::api;

#[get("/user/{owner}/")]
async fn list_groups(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    let auth_vu = auth::try_view_user(&app, &req, None, &owner).await?;

    let actions: GroupActions = app.as_group_actions();
    let groups = actions.list_groups(&auth_vu).await?;

    Ok(HttpResponse::Ok().json(groups))
}

#[get("/id/{id}")]
async fn view_group(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let auth_vg = auth::try_view_group(&app, &req, &id).await?;

    let actions: GroupActions = app.as_group_actions();
    let group = actions.view_group(&auth_vg).await?;

    Ok(HttpResponse::Ok().json(group))
}

#[get("/id/{id}/members")]
async fn list_members(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();

    let auth_vg = auth::try_view_group(&app, &req, &id).await?;

    let actions: GroupActions = app.as_group_actions();
    let members = actions.list_members(&auth_vg).await?;

    Ok(HttpResponse::Ok().json(members))
}

#[post("/user/{owner}/")]
async fn create_group(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
    body: web::Json<api::CreateGroupData>,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    let auth_eu = auth::try_edit_user(&app, &req, None, &owner).await?;

    let actions: GroupActions = app.as_group_actions();
    let group = actions.create_group(&auth_eu, body.into_inner()).await?;

    Ok(HttpResponse::Ok().json(group))
}

#[patch("/id/{id}")]
async fn update_group(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    data: web::Json<api::UpdateGroupData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let auth_eg = auth::try_edit_group(&app, &req, &id).await?;

    let actions: GroupActions = app.as_group_actions();
    let group = actions.rename_group(&auth_eg, &data.name).await?;

    Ok(HttpResponse::Ok().json(group))
}

#[delete("/id/{id}")]
async fn delete_group(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();

    let auth_dg = auth::try_delete_group(&app, &req, &id).await?;

    let actions: GroupActions = app.as_group_actions();
    let group = actions.delete_group(&auth_dg).await?;

    Ok(HttpResponse::Ok().json(group))
}

#[post("/id/{id}/assignments/")]
async fn create_assignment(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    body: web::Json<api::CreateAssignmentData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let data = body.into_inner();
    let auth_ca = auth::try_create_assignment(&app, &req, data, id).await?;

    let actions: GroupActions = app.as_group_actions();
    let assignment = actions.create_assignment(&auth_ca).await?;

    Ok(HttpResponse::Ok().json(assignment))
}

#[get("/id/{id}/assignments/")]
async fn view_group_assignments(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (id,) = path.into_inner();
    let auth_vga = auth::try_view_group_assignments(&app, &req, id).await?;

    let actions: GroupActions = app.as_group_actions();
    let assignments = actions.view_group_assignments(&auth_vga).await?;

    Ok(HttpResponse::Ok().json(assignments))
}

#[get("/id/{group_id}/assignments/id/{id}/")]
async fn view_assignment(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::AssignmentId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, id) = path.into_inner();
    let auth_va = auth::try_view_assignment(&app, &req, id, group_id).await?;

    let assignment = api::Assignment::from(auth_va.assignment);

    Ok(HttpResponse::Ok().json(assignment))
}

#[patch("/id/{group_id}/assignments/id/{id}/")]
async fn edit_assignment(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::AssignmentId)>,
    body: web::Json<api::UpdateAssignmentData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, id) = path.into_inner();
    let update = body.into_inner();
    let auth_ea = auth::try_edit_assignment(&app, &req, id, group_id).await?;

    let actions: GroupActions = app.as_group_actions();
    let assignment = actions.edit_assignment(&auth_ea, update).await?;

    Ok(HttpResponse::Ok().json(assignment))
}

#[delete("/id/{group_id}/assignments/id/{id}/")]
async fn delete_assignment(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::AssignmentId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, id) = path.into_inner();
    let auth_da = auth::try_delete_assignment(&app, &req, id, group_id).await?;

    let actions: GroupActions = app.as_group_actions();
    let assignment = actions.delete_assignment(&auth_da).await?;

    Ok(HttpResponse::Ok().json(assignment))
}

#[post("/id/{group_id}/assignments/id/{assignment_id}/submissions/")]
async fn create_submission(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::AssignmentId)>,
    body: web::Json<api::CreateSubmissionData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, assignment_id) = path.into_inner();
    let data = body.into_inner();
    let auth_cs = auth::try_create_submission(&app, &req, data, assignment_id, group_id).await?;

    let actions: GroupActions = app.as_group_actions();
    let submission = actions.create_submission(&auth_cs).await?;

    Ok(HttpResponse::Ok().json(submission))
}

#[get("/id/{group_id}/assignments/id/{assignment_id}/submissions/id/{id}/")]
async fn view_submission(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::AssignmentId, api::SubmissionId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, assignment_id, id) = path.into_inner();
    let auth_vs = auth::try_view_submission(&app, &req, id, assignment_id).await?;

    let submission = api::Submission::from(auth_vs.submission);

    Ok(HttpResponse::Ok().json(submission))
}

#[get("/id/{group_id}/assignments/id/{assignment_id}/submissions/")]
async fn view_assignment_submissions(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::AssignmentId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, assignment_id) = path.into_inner();
    let auth_vas =
        auth::try_view_assignment_submissions(&app, &req, assignment_id, group_id).await?;

    let actions: GroupActions = app.as_group_actions();
    let submissions = actions.view_assignment_submissions(&auth_vas).await?;

    Ok(HttpResponse::Ok().json(submissions))
}

#[get("/id/{group_id}/assignments/id/{assignment_id}/submissions/user/{owner}/")]
async fn view_user_submissions(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::AssignmentId, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, assignment_id, owner) = path.into_inner();
    let auth_vus = auth::try_view_owner_submissions(&app, &req, owner, assignment_id).await?;

    let actions: GroupActions = app.as_group_actions();
    let submissions = actions.view_user_submissions(&auth_vus).await?;

    Ok(HttpResponse::Ok().json(submissions))
}

#[get("/id/{group_id}/assignments/id/{assignment_id}/submissions/id/{id}/xml/")]
async fn view_submission_xml(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::AssignmentId, api::SubmissionId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, assignment_id, id) = path.into_inner();
    let auth_vs = auth::try_view_submission(&app, &req, id, assignment_id).await?;

    let actions: GroupActions = app.as_group_actions();
    let xml = actions.view_submission_xml(&auth_vs).await?;

    Ok(HttpResponse::Ok().content_type("text/xml").body(xml))
}

#[delete("/id/{group_id}/assignments/id/{assignment_id}/submissions/id/{id}/")]
async fn delete_submission(
    app: web::Data<AppData>,
    path: web::Path<(api::GroupId, api::AssignmentId, api::SubmissionId)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (group_id, assignment_id, id) = path.into_inner();
    let auth_vs = auth::try_delete_submission(&app, &req, id, assignment_id).await?;

    let actions: GroupActions = app.as_group_actions();
    let submission = actions.delete_submission(&auth_vs).await?;

    Ok(HttpResponse::Ok().json(submission))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_groups)
        .service(view_group)
        .service(list_members)
        .service(create_group)
        .service(update_group)
        .service(delete_group)
        .service(create_assignment)
        .service(view_group_assignments)
        .service(view_assignment)
        .service(edit_assignment)
        .service(delete_assignment)
        .service(create_submission)
        .service(view_submission)
        .service(view_assignment_submissions)
        .service(view_user_submissions)
        .service(view_submission_xml)
        .service(delete_submission);
}

#[cfg(test)]
mod tests {
    use actix_web::{body::MessageBody, http, test, App};
    use mongodb::bson::doc;
    use mongodb::bson::DateTime;
    use netsblox_cloud_common::{Assignment, Group, User};

    use super::*;
    use crate::test_utils;

    #[actix_web::test]
    async fn test_update_group() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(user.username.clone(), "some_group".into());

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let data = api::UpdateGroupData {
                    name: "new_name".into(),
                };
                let req = test::TestRequest::patch()
                    .uri(&format!("/id/{}", &group.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;

                // Check that the group is updated in the db
                let query = doc! {"id": &group.id};
                let group = app_data
                    .groups
                    .find_one(query, None)
                    .await
                    .expect("Could not query DB")
                    .ok_or(UserError::GroupNotFoundError)
                    .expect("Group not found in db.");

                assert_eq!(group.name, "new_name".to_string());

                // Check response
                assert_eq!(response.status(), http::StatusCode::OK);
                let bytes = response.into_body().try_into_bytes().unwrap();
                let group: api::Group = serde_json::from_slice(&bytes).unwrap();

                assert_eq!(group.name, "new_name".to_string());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_update_group_no_perms() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new("other_user".into(), "some_group".into());

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let data = api::UpdateGroupData {
                    name: "new_name".into(),
                };
                let req = test::TestRequest::patch()
                    .uri(&format!("/id/{}", &group.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&data)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    #[ignore]
    async fn test_update_group_404() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_group() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(user.username.clone(), "some_group".into());

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .uri(&format!("/id/{}", &group.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let _group: api::Group = test::call_and_read_body_json(&app, req).await;
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_group_403() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new("other_user".into(), "some_group".into());

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .uri(&format!("/id/{}", &group.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_ne!(response.status(), http::StatusCode::OK);

                let query = doc! {"id": group.id};
                let group = app_data.groups.find_one(query, None).await.unwrap();
                assert!(group.is_some());
            })
            .await;
    }

    #[actix_web::test]
    async fn test_delete_group_404() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new("other_user".into(), "some_group".into());

        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::delete()
                    .uri(&format!("/id/{}", &group.id))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
            })
            .await;
    }
    // TODO: How does it handle malformed IDs?

    #[actix_web::test]
    async fn test_get_assignment() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(user.username.clone(), "some_group".into());
        let assignment = Assignment::new(
            "assignment_1".to_string(),
            group.id.clone(),
            DateTime::now(),
        );

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_groups(&[group.clone()])
            .with_assignments(&[assignment.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data.clone()))
                        .wrap(test_utils::cookie::middleware())
                        .configure(config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!(
                        "/id/{}/assignments/id/{}/",
                        &group.id, &assignment.id
                    ))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let response = test::call_service(&app, req).await;

                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }
}
