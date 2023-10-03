use crate::app_data::AppData;
use crate::auth;
use crate::common::api::{CreateLibraryData, PublishState};
use crate::errors::UserError;
use crate::libraries::actions::LibraryActions;
use actix_web::{delete, get, post, HttpRequest};
use actix_web::{web, HttpResponse};

// TODO: add an endpoint for the official ones?
#[get("/community/")]
async fn list_community_libraries(app: web::Data<AppData>) -> Result<HttpResponse, UserError> {
    let actions: LibraryActions = app.to_library_actions();
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

    let actions: LibraryActions = app.to_library_actions();
    let libraries = actions.list_user_libraries(&auth_ll).await?;

    Ok(HttpResponse::Ok().json(libraries))
}

#[get("/user/{owner}/{name}")]
async fn get_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let auth_vl = auth::try_view_library(&app, &req, &owner, &name).await?;

    let actions: LibraryActions = app.to_library_actions();
    let blocks = actions.get_library_code(&auth_vl);

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

    let actions: LibraryActions = app.to_library_actions();
    let library = actions.save_library(&auth_el, &data.into_inner()).await?;

    Ok(HttpResponse::Ok().json(library))
}

#[delete("/user/{owner}/{name}")]
async fn delete_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let auth_el = auth::try_edit_library(&app, &req, &owner).await?;

    let actions: LibraryActions = app.to_library_actions();
    let library = actions.delete_library(&auth_el, &name).await?;

    Ok(HttpResponse::Ok().json(library))
}

#[post("/user/{owner}/{name}/publish")]
async fn publish_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let auth_pl = auth::try_publish_library(&app, &req, &owner).await?;

    let actions: LibraryActions = app.to_library_actions();
    let library = actions.publish(&auth_pl, &name).await?;

    Ok(HttpResponse::Ok().json(library.state))
}

#[post("/user/{owner}/{name}/unpublish")]
async fn unpublish_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let auth_pl = auth::try_publish_library(&app, &req, &owner).await?;

    let actions: LibraryActions = app.to_library_actions();
    let library = actions.unpublish(&auth_pl, &name).await?;

    Ok(HttpResponse::Ok().json(library))
}

#[get("/mod/pending")]
async fn list_pending_libraries(
    app: web::Data<AppData>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let auth_lpl = auth::try_moderate_libraries(&app, &req).await?;

    let actions: LibraryActions = app.to_library_actions();
    let libraries = actions.list_pending_libraries(&auth_lpl).await?;

    Ok(HttpResponse::Ok().json(libraries))
}

#[post("/mod/{owner}/{name}")]
async fn set_library_state(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    state: web::Json<PublishState>,
    req: HttpRequest,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let auth_ml = auth::try_moderate_libraries(&app, &req).await?;

    let actions: LibraryActions = app.to_library_actions();
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
    //use super::*;
    //use actix_web::test;

    // #[actix_web::test]
    //#[ignore]
    // async fn test_list_community_libraries() {
    //     let libraries = vec![
    //         LibraryMetadata::new("brian".into(), "public example".into(), true, None),
    //         LibraryMetadata::new("brian".into(), "private example".into(), false, None),
    //     ];
    //     let database = init_database("list_community_libs", libraries)
    //         .await
    //         .expect("Unable to initialize database");

    //     // Run the test
    //     let mut app = test::init_service(
    //         App::new()
    //             .app_data(web::Data::new(database))
    //             .configure(config),
    //     )
    //     .await;
    //     let req = test::TestRequest::get().uri("/community").to_request();
    //     let response = test::call_service(&mut app, req).await;

    //     assert_eq!(response.status(), http::StatusCode::OK);
    //     let pub_libs: std::vec::Vec<LibraryMetadata> = test::read_body_json(response).await;
    //     assert_eq!(pub_libs.len(), 1);
    //     assert_eq!(pub_libs[0].state, PublishState::Public);
    // }

    // #[actix_web::test]
    //#[ignore]
    // async fn test_list_user_libraries() {
    //     // TODO: 403 if not allowed?
    //     let libraries = vec![
    //         LibraryMetadata::new("cassie".into(), "project 1".into(), false, None),
    //         LibraryMetadata::new("brian".into(), "project 2".into(), false, None),
    //         LibraryMetadata::new("brian".into(), "project 3".into(), true, None),
    //     ];
    //     let database = init_database("list_user_libs", libraries)
    //         .await
    //         .expect("Unable to initialize database");

    //     // Run the test
    //     let mut app = test::init_service(
    //         App::new()
    //             .app_data(web::Data::new(database.clone()))
    //             .configure(config),
    //     )
    //     .await;
    //     let req = test::TestRequest::get().uri("/brian").to_request();
    //     let response = test::call_service(&mut app, req).await;

    //     assert_eq!(response.status(), http::StatusCode::OK);
    //     let libs: std::vec::Vec<LibraryMetadata> = test::read_body_json(response).await;
    //     assert_eq!(libs.len(), 2);
    //     libs.iter().for_each(|lib| {
    //         assert_eq!(lib.owner, "brian");
    //     });
    // }

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
    // async fn test_save_user_library() {
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
