use crate::{app_data::AppData, errors::InternalError, utils};
use actix_web::HttpRequest;
use mongodb::bson::doc;
use netsblox_cloud_common::{api, Assignment, Submission};

use crate::errors::UserError;

// Permissions on groups
pub(crate) struct ViewGroup {
    pub(crate) id: api::GroupId,
    _private: (),
}

pub(crate) struct EditGroup {
    pub(crate) id: api::GroupId,
    _private: (),
}

pub(crate) struct DeleteGroup {
    pub(crate) id: api::GroupId,
    _private: (),
}

pub(crate) struct CreateAssignment {
    pub(crate) ca_data: api::CreateAssignmentData,
    pub(crate) group_id: api::GroupId,
    _private: (),
}

pub(crate) struct ViewAssignment {
    pub(crate) assignment: Assignment,
    _private: (),
}

pub(crate) struct ViewGroupAssignments {
    pub(crate) group_id: api::GroupId,
    _private: (),
}

pub(crate) struct EditAssignment {
    pub(crate) assignment: Assignment,
    _private: (),
}

pub(crate) struct DeleteAssignment {
    pub(crate) assignment: Assignment,
    _private: (),
}

pub(crate) struct CreateSubmission {
    pub(crate) assignment_id: api::AssignmentId,
    pub(crate) cs_data: api::CreateSubmissionData,
    _private: (),
}

pub(crate) struct ViewSubmission {
    pub(crate) submission: Submission,
    _private: (),
}

pub(crate) struct ViewOwnerSubmissions {
    pub(crate) owner: String,
    pub(crate) assignment_id: api::AssignmentId,
    _private: (),
}

pub(crate) struct ViewAssignmentSubmissions {
    pub(crate) assignment: Assignment,
    _private: (),
}

pub(crate) struct DeleteSubmission {
    pub(crate) submission: Submission,
    _private: (),
}

// functions to try to obtain the given permissions
pub(crate) async fn try_view_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: &api::GroupId,
) -> Result<ViewGroup, UserError> {
    // for now you can only view the group if you are allowed to edit it
    try_edit_group(app, req, group_id)
        .await
        .map(|eg| ViewGroup {
            id: eg.id,
            _private: (),
        })
}

pub(crate) async fn try_edit_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: &api::GroupId,
) -> Result<EditGroup, UserError> {
    let query = doc! {"id": group_id};
    let group = app
        .groups
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::GroupNotFoundError)?;

    let authorized = utils::get_authorized_host(&app.authorized_services, req)
        .await
        .ok()
        .flatten()
        .is_some();

    if !authorized {
        let _auth = super::try_edit_user(app, req, None, &group.owner).await?;
    }

    Ok(EditGroup {
        id: group_id.to_owned(),
        _private: (),
    })
}

/// Try to obtain permissions to delete the given group. Only group owners
/// (or those who can edit group owners) are allowed to delete the group.
pub(crate) async fn try_delete_group(
    app: &AppData,
    req: &HttpRequest,
    group_id: &api::GroupId,
) -> Result<DeleteGroup, UserError> {
    let query = doc! {"id": group_id};
    let group = app
        .groups
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::GroupNotFoundError)?;
    let _auth = super::try_edit_user(app, req, None, &group.owner).await?;

    Ok(DeleteGroup {
        id: group_id.to_owned(),
        _private: (),
    })
}

pub(crate) async fn try_create_assignment(
    app: &AppData,
    req: &HttpRequest,
    ca_data: api::CreateAssignmentData,
    group_id: api::GroupId,
) -> Result<CreateAssignment, UserError> {
    let _auth_eg = super::try_edit_group(app, req, &group_id).await?;

    Ok(CreateAssignment {
        ca_data,
        group_id,
        _private: (),
    })
}

pub(crate) async fn try_view_assignment(
    app: &AppData,
    req: &HttpRequest,
    assignment_id: api::AssignmentId,
    group_id: api::GroupId,
) -> Result<ViewAssignment, UserError> {
    let assignment = app
        .assignments
        .find_one(doc! {"id": assignment_id, "groupId": group_id}, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::AssignmentNotFoundError)?;

    let can_edit_group = super::try_edit_group(app, req, &assignment.group_id)
        .await
        .is_ok();

    let requestor = utils::get_username(req).ok_or(UserError::LoginRequiredError)?;
    let is_member = utils::is_group_member(app, &requestor, &assignment.group_id).await?;

    if !can_edit_group && !is_member {
        Err(UserError::PermissionsError)
    } else {
        Ok(ViewAssignment {
            assignment,
            _private: (),
        })
    }
}

pub(crate) async fn try_view_group_assignments(
    app: &AppData,
    req: &HttpRequest,
    group_id: api::GroupId,
) -> Result<ViewGroupAssignments, UserError> {
    let can_edit_group = super::try_edit_group(app, req, &group_id).await.is_ok();

    let requestor = utils::get_username(req).ok_or(UserError::LoginRequiredError)?;
    let is_member = utils::is_group_member(app, &requestor, &group_id).await?;

    if !can_edit_group && !is_member {
        Err(UserError::PermissionsError)
    } else {
        Ok(ViewGroupAssignments {
            group_id: group_id.clone(),
            _private: (),
        })
    }
}

pub(crate) async fn try_edit_assignment(
    app: &AppData,
    req: &HttpRequest,
    assignment_id: api::AssignmentId,
    group_id: api::GroupId,
) -> Result<EditAssignment, UserError> {
    let assignment = app
        .assignments
        .find_one(doc! {"id": assignment_id, "groupId": group_id}, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::AssignmentNotFoundError)?;

    let _can_edit_group = super::try_edit_group(app, req, &assignment.group_id).await?;

    Ok(EditAssignment {
        assignment,
        _private: (),
    })
}

pub(crate) async fn try_delete_assignment(
    app: &AppData,
    req: &HttpRequest,
    assignment_id: api::AssignmentId,
    group_id: api::GroupId,
) -> Result<DeleteAssignment, UserError> {
    let auth_ea = try_edit_assignment(app, req, assignment_id, group_id).await?;
    Ok(DeleteAssignment {
        assignment: auth_ea.assignment,
        _private: (),
    })
}

pub(crate) async fn try_create_submission(
    app: &AppData,
    req: &HttpRequest,
    cs_data: api::CreateSubmissionData,
    assignment_id: api::AssignmentId,
    group_id: api::GroupId,
) -> Result<CreateSubmission, UserError> {
    let _auth_eu = super::try_edit_user(app, req, None, &cs_data.owner).await?;
    let _auth_va = try_view_assignment(app, req, assignment_id.clone(), group_id).await?;

    Ok(CreateSubmission {
        assignment_id,
        cs_data,
        _private: (),
    })
}

pub(crate) async fn try_view_submission(
    app: &AppData,
    req: &HttpRequest,
    id: api::SubmissionId,
    assignment_id: api::AssignmentId,
) -> Result<ViewSubmission, UserError> {
    let query = doc! {"id": id, "assignmentId": assignment_id};
    let submission = app
        .submissions
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::SubmissionNotFoundError)?;

    let _auth_eu = super::try_edit_user(app, req, None, &submission.owner).await?;

    Ok(ViewSubmission {
        submission,
        _private: (),
    })
}

pub(crate) async fn try_view_owner_submissions(
    app: &AppData,
    req: &HttpRequest,
    owner: String,
    assignment_id: api::AssignmentId,
) -> Result<ViewOwnerSubmissions, UserError> {
    let _auth_eu = super::try_edit_user(app, req, None, &owner).await?;

    Ok(ViewOwnerSubmissions {
        owner,
        assignment_id,
        _private: (),
    })
}

pub(crate) async fn try_view_assignment_submissions(
    app: &AppData,
    req: &HttpRequest,
    assignment_id: api::AssignmentId,
    group_id: api::GroupId,
) -> Result<ViewAssignmentSubmissions, UserError> {
    let assignment = app
        .assignments
        .find_one(doc! {"id": assignment_id, "groupId": group_id}, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::SubmissionNotFoundError)?;

    let _auth_eg = super::try_edit_group(app, req, &assignment.group_id).await?;

    Ok(ViewAssignmentSubmissions {
        assignment,
        _private: (),
    })
}

pub(crate) async fn try_delete_submission(
    app: &AppData,
    req: &HttpRequest,
    id: api::SubmissionId,
    assignment_id: api::AssignmentId,
) -> Result<DeleteSubmission, UserError> {
    try_view_submission(app, req, id, assignment_id)
        .await
        .map(|vs| DeleteSubmission {
            submission: vs.submission,
            _private: (),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{get, http, test, web, App, HttpResponse};
    use netsblox_cloud_common::{
        api::{self, UserRole},
        Group, User,
    };

    use crate::test_utils;

    #[actix_web::test]
    async fn test_try_edit_group_owner() {
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner.username.clone(), "someGroup".into());
        test_utils::setup()
            .with_users(&[owner.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(edit_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&owner.username))
                    .uri("/test")
                    .set_json(group.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_group_other() {
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let owner: User = api::NewUser {
            username: "owner".into(),
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner.username.clone(), "someGroup".into());
        test_utils::setup()
            .with_users(&[owner, other.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(edit_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&other.username))
                    .uri("/test")
                    .set_json(group.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_try_edit_group_admin() {
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
            email: "owner@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let group = Group::new(owner.username.clone(), "someGroup".into());
        test_utils::setup()
            .with_users(&[owner, admin.clone()])
            .with_groups(&[group.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .wrap(test_utils::cookie::middleware())
                        .app_data(web::Data::new(app_data.clone()))
                        .service(edit_test),
                )
                .await;

                let req = test::TestRequest::get()
                    .cookie(test_utils::cookie::new(&admin.username))
                    .uri("/test")
                    .set_json(group.id)
                    .to_request();

                let response = test::call_service(&app, req).await;
                assert_eq!(response.status(), http::StatusCode::OK);
            })
            .await;
    }

    #[get("/test")]
    async fn view_test(
        app: web::Data<AppData>,
        req: HttpRequest,
        group: web::Json<api::GroupId>,
    ) -> Result<HttpResponse, UserError> {
        let group_id = group.into_inner();
        try_view_group(&app, &req, &group_id).await?;
        Ok(HttpResponse::Ok().finish())
    }

    #[get("/test")]
    async fn edit_test(
        app: web::Data<AppData>,
        req: HttpRequest,
        group: web::Json<api::GroupId>,
    ) -> Result<HttpResponse, UserError> {
        let group_id = group.into_inner();
        try_edit_group(&app, &req, &group_id).await?;
        Ok(HttpResponse::Ok().finish())
    }

    #[get("/test")]
    async fn delete_test(
        app: web::Data<AppData>,
        req: HttpRequest,
        group: web::Json<api::GroupId>,
    ) -> Result<HttpResponse, UserError> {
        let group_id = group.into_inner();
        try_delete_group(&app, &req, &group_id).await?;
        Ok(HttpResponse::Ok().finish())
    }
}
