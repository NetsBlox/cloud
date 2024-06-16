use crate::app_data::AppData;
use crate::auth;
use crate::common::api::{self, CreateLibraryData, PublishState};
use crate::errors::UserError;
use crate::libraries::actions::LibraryActions;
use actix_web::{delete, get, post, HttpRequest};
use actix_web::{web, HttpResponse};

// TODO: add an endpoint for the official ones?
#[get("/community/")]
async fn list_community_libraries(app: web::Data<AppData>) -> Result<HttpResponse, UserError> {
    let actions: LibraryActions = app.as_library_actions();
    let libraries = actions.list_community_libraries().await?;

    Ok(HttpResponse::Ok().json(libraries))
}

#[get("/user/{owner}/")]
async fn list_user_libraries(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let auth_ll = auth::try_list_libraries(&app, &req, &username).await?;

    let actions: LibraryActions = app.as_library_actions();
    let libraries = actions.list_user_libraries(&auth_ll).await?;

    Ok(HttpResponse::Ok().json(libraries))
}

#[get("/user/{owner}/{name}")]
async fn get_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, api::LibraryName)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let auth_vl = auth::try_view_library(&app, &req, &owner, &name).await?;

    let blocks = LibraryActions::get_library_code(&auth_vl);

    Ok(HttpResponse::Ok().body(blocks))
}

#[post("/user/{owner}/")]
async fn save_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    data: web::Json<CreateLibraryData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();

    let auth_el = auth::try_edit_library(&app, &req, &owner).await?;

    let actions: LibraryActions = app.as_library_actions();
    let library = actions.save_library(&auth_el, &data.into_inner()).await?;

    Ok(HttpResponse::Ok().json(library))
}

#[delete("/user/{owner}/{name}")]
async fn delete_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, api::LibraryName)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let auth_el = auth::try_edit_library(&app, &req, &owner).await?;

    let actions: LibraryActions = app.as_library_actions();
    let library = actions.delete_library(&auth_el, &name).await?;

    Ok(HttpResponse::Ok().json(library))
}

#[post("/user/{owner}/{name}/publish")]
async fn publish_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, api::LibraryName)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let auth_pl = auth::try_publish_library(&app, &req, &owner).await?;

    let actions: LibraryActions = app.as_library_actions();
    let library = actions.publish(&auth_pl, &name).await?;

    Ok(HttpResponse::Ok().json(library.state))
}

#[post("/user/{owner}/{name}/unpublish")]
async fn unpublish_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, api::LibraryName)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let auth_pl = auth::try_publish_library(&app, &req, &owner).await?;

    let actions: LibraryActions = app.as_library_actions();
    let library = actions.unpublish(&auth_pl, &name).await?;

    Ok(HttpResponse::Ok().json(library))
}

#[get("/mod/pending")]
async fn list_pending_libraries(
    app: web::Data<AppData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let auth_lpl = auth::try_moderate_libraries(&app, &req).await?;

    let actions: LibraryActions = app.as_library_actions();
    let libraries = actions.list_pending_libraries(&auth_lpl).await?;

    Ok(HttpResponse::Ok().json(libraries))
}

#[post("/mod/{owner}/{name}")]
async fn set_library_state(
    app: web::Data<AppData>,
    path: web::Path<(String, api::LibraryName)>,
    state: web::Json<PublishState>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let auth_ml = auth::try_moderate_libraries(&app, &req).await?;

    let actions: LibraryActions = app.as_library_actions();
    let library = actions
        .set_library_state(&auth_ml, &owner, &name, state.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(library))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_community_libraries)
        .service(list_user_libraries)
        .service(get_user_library)
        .service(save_user_library)
        .service(delete_user_library)
        .service(publish_user_library)
        .service(unpublish_user_library)
        .service(list_pending_libraries)
        .service(set_library_state);
}

#[cfg(test)]
mod tests {
    use crate::test_utils;
    use actix_web::{test, web, App};
    use netsblox_cloud_common::{api, Library, User};

    #[actix_web::test]
    async fn test_list_user_libraries() {
        let user: User = api::NewUser {
            username: api::Username::new("user"),
            email: api::Email::new("user@netsblox.org"),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let priv_lib = Library {
            owner: user.username.clone(),
            name: api::LibraryName::new("private library"),
            notes: "my notes".into(),
            blocks: "<blocks/>".into(),
            state: api::PublishState::Private,
        };
        let pub_lib = Library {
            owner: user.username.clone(),
            name: api::LibraryName::new("pub library"),
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
    #[ignore]
    async fn test_list_community_libraries() {
        todo!()
    }

    #[actix_web::test]
    async fn test_save_public_library_with_approval() {
        let user: User = api::NewUser {
            username: api::Username::new("user"),
            email: api::Email::new("user@netsblox.org"),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let pub_lib = Library {
            owner: user.username.clone(),
            name: api::LibraryName::new("pub library"),
            notes: "my notes".into(),
            blocks: "<blocks/>".into(),
            state: api::PublishState::Public,
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_libraries(&[pub_lib.clone()])
            .run(|app_data| async move {
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(app_data))
                        .wrap(test_utils::cookie::middleware())
                        .configure(super::config),
                )
                .await;

                let lib_data = api::CreateLibraryData {
                    name: api::LibraryName::new("pub library"),
                    notes: "my notes".into(),
                    blocks: "<blocks><reportJSFunction/></blocks>".into(),
                };
                let req = test::TestRequest::post()
                    .uri(&format!("/user/{}/", &user.username))
                    .cookie(test_utils::cookie::new(&user.username))
                    .set_json(&lib_data)
                    .to_request();

                let metadata: api::LibraryMetadata = test::call_and_read_body_json(&app, req).await;
                assert!(matches!(metadata.state, api::PublishState::PendingApproval));
            })
            .await;
    }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_list_user_libraries_403() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_list_user_libraries_404() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_get_user_library() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_get_user_library_public() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_get_user_library_403() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_get_user_library_404() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_save_user_library_403() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_save_user_library_approval_req() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_delete_user_library() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_delete_user_library_403() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_delete_user_library_404() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_publish_user_library() {
    //     let publish_name = "to-publish-example";
    //     let libraries = vec![
    //         LibraryMetadata::new("brian".into(), publish_name.into(), false, None),
    //         LibraryMetadata::new("brian".into(), "private example".into(), false, None),
    //     ];
    //     let database = init_database("publish_user_lib", libraries)
    //         .await
    //         .expect("Unable to initialize database");

    //     // Run the test
    //     let mut app = test::init_service(
    //         App::new()
    //             .app_data(web::Data::new(database.clone()))
    //             .configure(config),
    //     )
    //     .await;
    //     let req = test::TestRequest::post()
    //         .uri(&format!("/brian/{}/publish", publish_name))
    //         .to_request();
    //     let response = test::call_service(&mut app, req).await;

    //     assert_eq!(response.status(), http::StatusCode::OK);
    //     let collection = database.collection::<LibraryMetadata>("libraries");
    //     let mut cursor = collection
    //         .find(doc! {}, None)
    //         .await
    //         .expect("Could not retrieve docs after publish");

    //     let mut count = 0;

    //     let libraries = cursor.try_collect::<Vec<_>>().await.unwrap();
    //     libraries.into_iter().for_each(|library| {
    //         let expected_public = if library.name == publish_name {
    //             PublishState::Public
    //         } else {
    //             PublishState::Private
    //         };
    //         assert_eq!(
    //             library.state, expected_public,
    //             "Expected \"{}\" to have public value of {}",
    //             library.name, expected_public
    //         );
    //         count += 1;
    //     });
    //     assert_eq!(count, 2);
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_publish_user_library_approval_req() {
    //     unimplemented!();
    // }

    #[actix_web::test]
    #[ignore]
    async fn test_publish_user_library() {
        // TODO: auto-publish if not needed
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_publish_user_library_mod() {
        // TODO: auto-publish if mod
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_publish_user_library_admin() {
        // TODO: auto-publish if mod
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_publish_user_library_403() {
        todo!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_unpublish_user_library() {
        unimplemented!();
    }

    #[actix_web::test]
    #[ignore]
    async fn test_unpublish_user_library_403() {
        unimplemented!();
    }
}
