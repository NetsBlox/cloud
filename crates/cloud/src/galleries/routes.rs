use crate::app_data::AppData;
use crate::errors::UserError;
use actix_web::post;
use actix_web::{web, HttpResponse};

#[post("/{owner}/")]
async fn create_gallery(app: web::Data<AppData>) -> Result<HttpResponse, UserError> {
    let actions = app.as_gallery_actions();
    let metadata = actions.create_gallery().await?;

    Ok(HttpResponse::Ok().json(metadata))
}

// TODO: Create endpoints for the other operations that need to be supported
// (make a function - like above - then add them to `config` - like below)

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_gallery);
}

#[cfg(test)]
mod tests {
    use crate::test_utils;
    use actix_web::{test, web, App};
    use netsblox_cloud_common::{api, Library, User};

    #[actix_web::test]
    async fn test_create_gallery() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let priv_lib = Library {
            owner: user.username.to_owned(),
            name: "private library".into(),
            notes: "my notes".into(),
            blocks: "<blocks/>".into(),
            state: api::PublishState::Private,
        };
        let pub_lib = Library {
            owner: user.username.to_owned(),
            name: "pub library".into(),
            notes: "my notes".into(),
            blocks: "<blocks/>".into(),
            state: api::PublishState::Public,
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_libraries(&[priv_lib, pub_lib])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(super::config),
                )
                .await;

                let req = test::TestRequest::get()
                    .uri(&format!("/user/{}/", &user.username))
                    .cookie(test_utils::cookie::new(&user.username))
                    .to_request();

                let libraries: Vec<api::LibraryMetadata> =
                    test::call_and_read_body_json(&app, req).await;

                assert_eq!(libraries.len(), 2);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_create_gallery_403() {
        // Check that another user cannot create a gallery for someone else
        todo!("Check that another user cannot create a gallery for someone else");
    }

    #[actix_web::test]

    async fn test_create_gallery_admin() {
        todo!("Check that an admin can create a gallery for another user");
    }
}
