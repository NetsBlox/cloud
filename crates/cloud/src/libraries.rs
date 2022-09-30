use crate::app_data::AppData;
use crate::errors::{InternalError, UserError};
use crate::users::{can_edit_user, ensure_is_moderator, is_moderator};
use actix_session::Session;
use actix_web::{delete, get, post};
use actix_web::{web, HttpResponse};
use futures::stream::TryStreamExt;
use lazy_static::lazy_static;
use mongodb::bson::doc;
use mongodb::options::{FindOneAndUpdateOptions, FindOptions};
use netsblox_core::{CreateLibraryData, LibraryMetadata, PublishState};
use regex::Regex;
use rustrict::CensorStr;

// TODO: add an endpoint for the official ones?
#[get("/community")]
async fn list_community_libraries(app: web::Data<AppData>) -> Result<HttpResponse, UserError> {
    let options = FindOptions::builder().sort(doc! {"name": 1}).build();
    let public_filter = doc! {"state": PublishState::Public};
    let cursor = app
        .library_metadata
        .find(public_filter, options)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;

    let libraries = cursor
        .try_collect::<Vec<_>>()
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;
    Ok(HttpResponse::Ok().json(libraries))
}

#[get("/user/{owner}/")]
async fn list_user_libraries(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (username,) = path.into_inner();
    let query = doc! {"owner": username};
    let options = FindOptions::builder().sort(doc! {"name": 1}).build();
    let mut cursor = app
        .library_metadata
        .find(query, options)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;

    let mut libraries = Vec::new();
    while let Some(library) = cursor.try_next().await.expect("Could not fetch library") {
        if can_view_library(&app, &session, &library)
            .await
            .unwrap_or(false)
        {
            // TODO: do this in the outer loop
            libraries.push(library);
        }
    }

    Ok(HttpResponse::Ok().json(libraries))
}

#[get("/user/{owner}/{name}")]
async fn get_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    let query = doc! {"owner": owner, "name": name};
    let library = app
        .libraries
        .find_one(query, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .ok_or_else(|| UserError::LibraryNotFoundError)?;

    let blocks = library.blocks.to_owned();
    ensure_can_view_library(&app, &session, &library.into()).await?;
    Ok(HttpResponse::Ok().body(blocks))
}

#[post("/user/{owner}/")]
async fn save_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    data: web::Json<CreateLibraryData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner,) = path.into_inner();
    if !is_valid_name(&data.name) {
        return Ok(HttpResponse::BadRequest().body("Invalid library name"));
    }
    ensure_can_edit_library(&app, &session, &owner).await?;

    let query = doc! {"owner": &owner, "name": &data.name};
    let update = doc! {
        "$set": {
            "notes": &data.notes,
            "blocks": &data.blocks,
        },
        "$setOnInsert": {
            "owner": &owner,
            "name": &data.name,
            "state": PublishState::Private,
        }
    };
    let options = FindOneAndUpdateOptions::builder().upsert(true).build();
    match app
        .libraries
        .find_one_and_update(query.clone(), update, options)
        .await
        .unwrap()
    {
        Some(library) => {
            let needs_approval = match library.state {
                PublishState::Private => false,
                _ => is_approval_required(&data.blocks),
            };

            let publish_state = if needs_approval {
                let update = doc! {"state": PublishState::PendingApproval};
                app.libraries.update_one(query, update, None).await.unwrap();
                PublishState::PendingApproval
            } else {
                library.state
            };

            Ok(HttpResponse::Ok().json(publish_state))
        }
        None => Ok(HttpResponse::Created().json(PublishState::Private)),
    }
}

fn is_valid_name(name: &str) -> bool {
    lazy_static! {
        static ref LIBRARY_NAME: Regex = Regex::new(r"^[A-zÀ-ÿ0-9 \(\)_-]+$").unwrap();
    }
    LIBRARY_NAME.is_match(name) && !name.is_inappropriate()
}

fn is_approval_required(text: &str) -> bool {
    text.contains("reportJSFunction") || text.is_inappropriate()
}

#[delete("/user/{owner}/{name}")]
async fn delete_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    ensure_can_edit_library(&app, &session, &owner).await?;

    let query = doc! {"owner": owner, "name": name};
    let result = app
        .library_metadata
        .delete_one(query, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;

    if result.deleted_count == 0 {
        Err(UserError::LibraryNotFoundError)
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

#[post("/user/{owner}/{name}/publish")]
async fn publish_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    ensure_can_edit_library(&app, &session, &owner).await?;

    let query = doc! {"owner": owner, "name": name};
    let update = doc! {"$set": {"state": PublishState::PendingApproval}};

    let library = app
        .libraries
        .find_one_and_update(query.clone(), update, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?
        .ok_or_else(|| UserError::LibraryNotFoundError)?;

    if !is_approval_required(&library.blocks) || is_moderator(&app, &session).await? {
        let update = doc! {"$set": {"state": PublishState::Public}};
        app.library_metadata
            .update_one(query, update, None)
            .await
            .unwrap();
        Ok(HttpResponse::Ok().json(PublishState::Public))
    } else {
        Ok(HttpResponse::Ok().json(PublishState::PendingApproval))
    }
}

#[post("/user/{owner}/{name}/unpublish")]
async fn unpublish_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    let (owner, name) = path.into_inner();
    ensure_can_edit_library(&app, &session, &owner).await?;

    let query = doc! {"owner": owner, "name": name};
    let update = doc! {"$set": {"state": PublishState::Private}};
    let result = app
        .library_metadata
        .update_one(query, update, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;

    if result.matched_count == 0 {
        Err(UserError::LibraryNotFoundError)
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

async fn ensure_can_edit_library(
    app: &AppData,
    session: &Session,
    owner: &str,
) -> Result<(), UserError> {
    if !can_edit_library(app, session, owner).await? {
        Err(UserError::PermissionsError)
    } else {
        Ok(())
    }
}

async fn can_edit_library(
    app: &AppData,
    session: &Session,
    owner: &str,
) -> Result<bool, UserError> {
    match session.get::<String>("username").unwrap_or(None) {
        Some(_username) => can_edit_user(app, session, owner).await,
        None => Ok(false),
    }
}

async fn ensure_can_view_library(
    app: &AppData,
    session: &Session,
    library: &LibraryMetadata,
) -> Result<(), UserError> {
    if !can_view_library(app, session, library).await? {
        Err(UserError::PermissionsError)
    } else {
        Ok(())
    }
}

async fn can_view_library(
    app: &AppData,
    session: &Session,
    library: &LibraryMetadata,
) -> Result<bool, UserError> {
    match library.state {
        PublishState::Public => Ok(true),
        _ => can_edit_library(app, session, &library.owner).await,
    }
}

#[get("/mod/pending")]
async fn list_pending_libraries(
    app: web::Data<AppData>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    ensure_is_moderator(&app, &session).await?;

    let cursor = app
        .library_metadata
        .find(doc! {"state": PublishState::PendingApproval}, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;

    let libraries = cursor
        .try_collect::<Vec<_>>()
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;

    Ok(HttpResponse::Ok().json(libraries))
}

#[post("/mod/{owner}/{name}")]
async fn set_library_state(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    state: web::Json<PublishState>,
    session: Session,
) -> Result<HttpResponse, UserError> {
    ensure_is_moderator(&app, &session).await?;

    let (owner, name) = path.into_inner();
    let query = doc! {"owner": owner, "name": name};
    let update = doc! {"$set": {"state": state.into_inner()}};
    app.library_metadata
        .update_one(query, update, None)
        .await
        .map_err(|err| InternalError::DatabaseConnectionError(err))?;

    Ok(HttpResponse::Ok().finish())
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
    use super::*;
    use actix_web::{http, test, App};
    use mongodb::{Client, Database};

    // async fn init_database(
    //     name: &str,
    //     libraries: std::vec::Vec<LibraryMetadata>,
    // ) -> Result<Database, UserError> {
    //     let library_count = libraries.len();
    //     let client = Client::with_uri_str("mongodb://127.0.0.1:27017/")
    //         .await
    //         .expect("Unable to connect to database");

    //     // Seed the database
    //     let database_name = &format!("netsblox-tests-{}", name);
    //     let database = client.database(database_name);
    //     let collection = database.collection::<LibraryMetadata>("libraries"); // FIXME: rename collection - not database
    //     collection
    //         .delete_many(doc! {}, None)
    //         .await
    //         .expect("Unable to empty database");
    //     collection
    //         .insert_many(libraries, None)
    //         .await
    //         .expect("Unable to seed database");

    //     let count = collection
    //         .count_documents(doc! {}, None)
    //         .await
    //         .expect("Unable to count docs");
    //     assert_eq!(
    //         count, library_count as u64,
    //         "Expected {} docs but found {}",
    //         library_count, count
    //     );

    //     Ok(database)
    // }

    // #[actix_web::test]
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
    // async fn test_list_user_libraries_403() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    // async fn test_list_user_libraries_404() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    // async fn test_get_user_library() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    // async fn test_get_user_library_public() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    // async fn test_get_user_library_403() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    // async fn test_get_user_library_404() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    // async fn test_save_user_library() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    // async fn test_save_user_library_403() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    // async fn test_save_user_library_approval_req() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    // async fn test_delete_user_library() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    // async fn test_delete_user_library_403() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
    // async fn test_delete_user_library_404() {
    //     unimplemented!();
    // }

    // #[actix_web::test]
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
    // async fn test_publish_user_library_approval_req() {
    //     unimplemented!();
    // }

    #[actix_web::test]
    async fn test_publish_user_library_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_unpublish_user_library() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_unpublish_user_library_403() {
        unimplemented!();
    }

    #[test]
    async fn test_is_valid_name() {
        assert!(is_valid_name("hello library"));
    }

    #[test]
    async fn test_is_valid_name_diacritic() {
        assert!(is_valid_name("hola libré"));
    }

    #[test]
    async fn test_is_valid_name_weird_symbol() {
        assert_eq!(is_valid_name("<hola libré>"), false);
    }
}
