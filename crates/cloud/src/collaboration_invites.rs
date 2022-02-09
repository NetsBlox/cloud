use actix_session::Session;
use actix_web::{get, post};
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::doc;

use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::models::{CollaborationInvite, InvitationState};
use crate::users::ensure_can_edit_user;

#[get("/user/{recipient}/")]
async fn list_invites(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (recipient,) = path.into_inner();
    ensure_can_edit_user(&app, &session, &recipient).await?;

    let query = doc! {"recipient": recipient};
    let cursor = app.collab_invites.find(query, None).await.unwrap();
    let invites = cursor.try_collect::<Vec<_>>().await.unwrap();

    Ok(HttpResponse::Ok().json(invites))
}

#[post("/{project_id}/invite/{recipient}")]
async fn send_invite(
    app: web::Data<AppData>,
    session: Session,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, UserError> {
    let (project_id, recipient) = path.into_inner();

    let query = doc! {"id": &project_id};
    let metadata = app
        .project_metadata
        .find_one(query, None)
        .await
        .map_err(|_err| InternalError::DatabaseConnectionError)? // TODO: wrap the error?
        .ok_or_else(|| UserError::ProjectNotFoundError)?;

    ensure_can_edit_user(&app, &session, &metadata.owner).await?;
    let sender = session
        .get::<String>("username")
        .unwrap_or(None)
        .ok_or_else(|| UserError::PermissionsError)?;

    let invitation = CollaborationInvite::new(sender.clone(), recipient.clone(), project_id);

    let query = doc! {
        "sender": &sender,
        "recipient": &recipient,
        "projectId": &invitation.project_id
    };
    let update = doc! {
        "$setOnInsert": invitation
    };
    let options = mongodb::options::UpdateOptions::builder()
        .upsert(true)
        .build();

    let result = app
        .collab_invites
        .update_one(query, update, Some(options))
        .await
        .unwrap();

    if result.matched_count == 1 {
        Ok(HttpResponse::Conflict().body("Invitation already exists."))
    } else {
        Ok(HttpResponse::Ok().body("Invitation sent!"))
    }
}

#[post("/id/{id}")]
async fn respond_to_invite(
    app: web::Data<AppData>,
    state: web::Json<InvitationState>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (recipient, id) = path.into_inner();
    let query = doc! {"_id": id};
    ensure_can_edit_user(&app, &session, &recipient).await?;

    if let Some(invite) = app
        .collab_invites
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        // TODO: set the invites to collaborator list?
        if app
            .project_metadata
            .find_one(doc! {"id": &invite.project_id}, None)
            .await
            .unwrap()
            .is_none()
        {
            app.collab_invites.delete_one(query, None).await.unwrap();
            return Ok(HttpResponse::NotFound().body("Project no longer exists."));
        }

        let update = doc! {
            "$set": {
                "state": state.into_inner()
            }
        };
        // TODO: Update the project
        app.collab_invites
            .update_one(query, update, None)
            .await
            .unwrap();

        // TODO: Should this send something else? Maybe a token or something to use to join the project?
        Ok(HttpResponse::Ok().body("Invitation updated!"))
    } else {
        Ok(HttpResponse::NotFound().body("Invitation not found."))
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_invites)
        .service(send_invite)
        .service(respond_to_invite);
}

#[cfg(test)]
mod tests {

    #[actix_web::test]
    async fn test_view_invites() {
        todo!();
    }

    #[actix_web::test]
    async fn test_view_invites_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_view_invites_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_send_invite() {
        todo!();
    }

    #[actix_web::test]
    async fn test_send_invite_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_send_invite_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_respond_to_invite() {
        todo!();
    }

    #[actix_web::test]
    async fn test_respond_to_invite_403() {
        todo!();
    }

    #[actix_web::test]
    async fn test_respond_to_invite_admin() {
        todo!();
    }

    #[actix_web::test]
    async fn test_respond_to_invite_project_deleted() {
        todo!();
    }
}
