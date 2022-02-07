use actix_session::Session;
use actix_web::{get, post};
use actix_web::{web, HttpResponse};
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use serde::Deserialize;

use crate::app_data::AppData;
use crate::models::{CollaborationInvitation, InvitationState};
use crate::users::can_edit_user;

#[get("/{recipient}/")]
async fn list_invites(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (recipient,) = path.into_inner();
    if !can_edit_user(&app, &session, &recipient).await {
        return Ok(HttpResponse::Unauthorized().body("Not allowed"));
    }

    let query = doc! {"recipient": recipient};
    let cursor = app.collab_invites.find(query, None).await.unwrap();
    let invites = cursor.try_collect::<Vec<_>>().await.unwrap();

    Ok(HttpResponse::Ok().json(invites))
}

#[derive(Deserialize)]
struct CollaborateRequestBody {
    sender: Option<String>,
    project_id: ObjectId,
}

#[post("/{recipient}/")]
async fn send_invite(
    app: web::Data<AppData>,
    session: Session,
    path: web::Path<(String,)>,
    body: web::Json<CollaborateRequestBody>,
) -> Result<HttpResponse, std::io::Error> {
    let (recipient,) = path.into_inner();
    if let Some(username) = session.get::<String>("username").unwrap_or(None) {
        let body = body.into_inner();
        let sender = body.sender.unwrap_or(username);
        let invitation =
            CollaborationInvitation::new(sender.clone(), recipient.clone(), body.project_id);
        if !can_edit_user(&app, &session, &sender).await {
            return Ok(HttpResponse::Unauthorized().body("Not allowed"));
        }
        let query =
            doc! {"sender": &sender, "recipient": &recipient, "projectId": &invitation.project_id};
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
    } else {
        Ok(HttpResponse::Unauthorized().body("Not allowed"))
    }
}

#[derive(Deserialize)]
struct CollaborateResponse {
    state: InvitationState,
}

#[post("/{recipient}/{id}")]
async fn respond_to_invite(
    app: web::Data<AppData>,
    body: web::Json<CollaborateResponse>,
    path: web::Path<(String, ObjectId)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (recipient, id) = path.into_inner();
    let query = doc! {"_id": id};
    if !can_edit_user(&app, &session, &recipient).await {
        return Ok(HttpResponse::Unauthorized().body("Not allowed."));
    }

    if let Some(invite) = app
        .collab_invites
        .find_one(query.clone(), None)
        .await
        .unwrap()
    {
        if app
            .project_metadata
            .find_one(doc! {"_id": &invite.project_id}, None)
            .await
            .unwrap()
            .is_none()
        {
            app.collab_invites.delete_one(query, None).await.unwrap();
            return Ok(HttpResponse::NotFound().body("Project no longer exists."));
        }

        let update = doc! {
            "$set": {
                "state": body.state.to_owned()
            }
        };
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
