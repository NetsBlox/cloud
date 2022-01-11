use crate::app_data::AppData;
use crate::users::{can_edit_user, is_super_user};
use actix_session::Session;
use actix_web::{delete, get, post};
use actix_web::{web, HttpResponse};
use futures::stream::TryStreamExt;
use lazy_static::lazy_static;
use mongodb::bson::doc;
use mongodb::options::{FindOneAndUpdateOptions, FindOptions};
use regex::Regex;
use rustrict::CensorStr;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct LibraryMetadata {
    owner: String,
    name: String,
    notes: String,
    public: bool,
}

impl LibraryMetadata {
    pub fn new(
        owner: String,
        name: String,
        public: bool,
        notes: Option<String>,
    ) -> LibraryMetadata {
        LibraryMetadata {
            owner,
            name,
            notes: notes.unwrap_or_else(String::new),
            public,
        }
    }
    pub fn from_lib(library: &Library) -> LibraryMetadata {
        LibraryMetadata::new(
            library.owner.clone(),
            library.name.clone(),
            library.public,
            Some(library.notes.clone()),
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Library {
    owner: String,
    name: String,
    notes: String,
    public: bool,
    blocks: String,
    needs_approval: bool,
}

// TODO: add an endpoint for the official ones?
#[get("/community")]
async fn list_community_libraries(db: web::Data<AppData>) -> Result<HttpResponse, std::io::Error> {
    let collection = db.collection::<LibraryMetadata>("libraries");

    let options = FindOptions::builder().sort(doc! {"name": 1}).build();
    let public_filter = doc! {"public": true};
    let cursor = collection
        .find(public_filter, options)
        .await
        .expect("Library list query failed");

    let libraries = cursor.try_collect::<Vec<_>>().await.unwrap();
    Ok(HttpResponse::Ok().json(libraries))
}

#[get("/user/{owner}")]
async fn list_user_libraries(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (username,) = path.into_inner();
    let query = doc! {"owner": username};
    let collection = app.collection::<LibraryMetadata>("libraries");
    let options = FindOptions::builder().sort(doc! {"name": 1}).build();
    let mut cursor = collection
        .find(query, options)
        .await
        .expect("Library list query failed");

    let mut libraries = Vec::new();
    while let Some(library) = cursor.try_next().await.expect("Could not fetch library") {
        if can_view_library(&app, &session, &library).await {
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
) -> Result<HttpResponse, std::io::Error> {
    let (owner, name) = path.into_inner();
    let collection = app.collection::<Library>("libraries");
    let query = doc! {"owner": owner, "name": name};
    if let Some(library) = collection
        .find_one(query, None)
        .await
        .expect("Unable to retrieve from database")
    {
        if can_view_library(&app, &session, &LibraryMetadata::from_lib(&library)).await {
            Ok(HttpResponse::Ok().body(library.blocks))
        } else {
            Ok(HttpResponse::Unauthorized().body("Not allowed."))
        }
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[derive(Serialize, Deserialize)]
struct CreateLibraryData {
    notes: String,
    blocks: String,
}
#[post("/user/{owner}/{name}")]
async fn save_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    data: web::Json<CreateLibraryData>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (owner, name) = path.into_inner();
    if !is_valid_name(&name) {
        return Ok(HttpResponse::BadRequest().body("Invalid library name"));
    }
    if !can_edit_library(&app, &session, &owner).await {
        return Ok(HttpResponse::Unauthorized().body("Not allowed."));
    }

    let collection = app.collection::<Library>("libraries");
    let query = doc! {"owner": &owner, "name": &name};
    let update = doc! {
        "notes": &data.notes,
        "blocks": &data.blocks,
        "$setOnInsert": {
            "owner": &owner,
            "name": &name,
            "public": false,
            "needsApproval": false,
        }
    };
    let options = FindOneAndUpdateOptions::builder().upsert(true).build();
    match collection
        .find_one_and_update(query.clone(), update, options)
        .await
        .unwrap()
    {
        Some(library) => {
            if library.public && is_approval_required(&data.blocks) {
                let update = doc! {"public": false, "needsApproval": true};
                collection.update_one(query, update, None).await.unwrap();
            }
            Ok(HttpResponse::Ok().body("Library saved."))
        }
        None => Ok(HttpResponse::Created().body("Library saved.")),
    }
}

fn is_valid_name(name: &str) -> bool {
    lazy_static! {
        static ref LIBRARY_NAME: Regex = Regex::new(r"^[A-zÀ-ÿ0-9 _-]+$").unwrap();
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
) -> Result<HttpResponse, std::io::Error> {
    let (owner, name) = path.into_inner();
    if !can_edit_library(&app, &session, &owner).await {
        return Ok(HttpResponse::Unauthorized().body("Not allowed."));
    }
    let collection = app.collection::<LibraryMetadata>("libraries");
    let query = doc! {"owner": owner, "name": name};
    let result = collection
        .delete_one(query, None)
        .await
        .expect("Unable to delete from database");
    if result.deleted_count == 0 {
        Ok(HttpResponse::NotFound().finish())
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

#[post("/user/{owner}/{name}/publish")]
async fn publish_user_library(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (owner, name) = path.into_inner();
    if !can_edit_library(&app, &session, &owner).await {
        return Ok(HttpResponse::Unauthorized().body("Not allowed."));
    }

    let collection = app.collection::<Library>("libraries");
    let query = doc! {"owner": owner, "name": name};
    let update = doc! {"$set": {"needsApproval": true}};

    match collection
        .find_one_and_update(query.clone(), update, None)
        .await
        .expect("Library publish operation failed")
    {
        Some(library) => {
            if !is_approval_required(&library.blocks) {
                let update = doc! {"$set": {"public": true, "needsApproval": false}};
                collection.update_one(query, update, None).await.unwrap();
                Ok(HttpResponse::Ok().body("Library published!"))
            } else {
                Ok(HttpResponse::Ok().body("Library marked to publish (approval required)."))
            }
        }
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

#[post("/user/{owner}/{name}/unpublish")]
async fn unpublish_user_library(
    db: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    let (owner, name) = path.into_inner();
    if !can_edit_library(&db, &session, &owner).await {
        return Ok(HttpResponse::Unauthorized().body("Not allowed."));
    }

    let collection = db.collection::<LibraryMetadata>("libraries");

    let query = doc! {"owner": owner, "name": name};
    let update = doc! {"$set": {"public": false}};
    let result = collection
        .update_one(query, update, None)
        .await
        .expect("Library unpublish operation failed");
    if result.matched_count == 0 {
        Ok(HttpResponse::NotFound().finish())
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

async fn can_edit_library(app: &AppData, session: &Session, owner: &str) -> bool {
    match session.get::<String>("username").unwrap_or(None) {
        Some(username) => can_edit_user(app, session, owner).await,
        None => false,
    }
}

async fn can_view_library(app: &AppData, session: &Session, library: &LibraryMetadata) -> bool {
    if library.public {
        return true;
    }

    match session.get::<String>("username").unwrap_or(None) {
        Some(username) => can_edit_user(app, session, &library.owner).await,
        None => false,
    }
}

#[get("/admin/approval_needed")]
async fn list_approval_needed(
    app: web::Data<AppData>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    if !is_super_user(&app, &session).await {
        return Ok(HttpResponse::Unauthorized().body("Not allowed."));
    }

    let collection = app.collection::<LibraryMetadata>("libraries");
    let cursor = collection
        .find(doc! {"needs_approval": true}, None)
        .await
        .expect("Could not retrieve libraries");

    let libraries = cursor.try_collect::<Vec<_>>().await.unwrap();

    Ok(HttpResponse::Ok().json(libraries))
}

#[post("/admin/{owner}/{name}/approve")]
async fn approve_library(
    app: web::Data<AppData>,
    path: web::Path<(String, String)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    if !is_super_user(&app, &session).await {
        return Ok(HttpResponse::Unauthorized().body("Not allowed."));
    }

    let collection = app.collection::<LibraryMetadata>("libraries");
    let (owner, name) = path.into_inner();
    let query = doc! {"owner": owner, "name": name};
    let update = doc! {"$set": {"public": true, "needsApproval": false}};
    collection
        .update_one(query, update, None)
        .await
        .expect("Unable to update library");
    Ok(HttpResponse::Ok().finish())
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_community_libraries)
        .service(list_user_libraries)
        .service(get_user_library)
        .service(save_user_library)
        .service(delete_user_library)
        .service(publish_user_library)
        .service(unpublish_user_library);
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http, test, App};
    use mongodb::{Client, Database};

    async fn init_database(
        name: &str,
        libraries: std::vec::Vec<LibraryMetadata>,
    ) -> Result<Database, std::io::Error> {
        let library_count = libraries.len();
        let client = Client::with_uri_str("mongodb://127.0.0.1:27017/")
            .await
            .expect("Unable to connect to database");

        // Seed the database
        let database_name = &format!("netsblox-tests-{}", name);
        let database = client.database(database_name);
        let collection = database.collection::<LibraryMetadata>("libraries"); // FIXME: rename collection - not database
        collection
            .delete_many(doc! {}, None)
            .await
            .expect("Unable to empty database");
        collection
            .insert_many(libraries, None)
            .await
            .expect("Unable to seed database");

        let count = collection
            .count_documents(doc! {}, None)
            .await
            .expect("Unable to count docs");
        assert_eq!(
            count, library_count as u64,
            "Expected {} docs but found {}",
            library_count, count
        );

        Ok(database)
    }

    #[actix_web::test]
    async fn test_list_community_libraries() {
        let libraries = vec![
            LibraryMetadata::new(
                "brian".into(),
                "public example".into(),
                true,
                None,
            ),
            LibraryMetadata::new(
                "brian".into(),
                "private example".into(),
                false,
                None,
            ),
        ];
        let database = init_database("list_community_libs", libraries)
            .await
            .expect("Unable to initialize database");

        // Run the test
        let mut app = test::init_service(
            App::new()
                .app_data(web::Data::new(database))
                .configure(config),
        )
        .await;
        let req = test::TestRequest::get().uri("/community").to_request();
        let response = test::call_service(&mut app, req).await;

        assert_eq!(response.status(), http::StatusCode::OK);
        let pub_libs: std::vec::Vec<LibraryMetadata> = test::read_body_json(response).await;
        assert_eq!(pub_libs.len(), 1);
        assert_eq!(pub_libs[0].public, true);
    }

    #[actix_web::test]
    async fn test_list_user_libraries() {
        // TODO: 403 if not allowed?
        let libraries = vec![
            LibraryMetadata::new("cassie".into(), "project 1".into(), false, None),
            LibraryMetadata::new("brian".into(), "project 2".into(), false, None),
            LibraryMetadata::new("brian".into(), "project 3".into(), true, None),
        ];
        let database = init_database("list_user_libs", libraries)
            .await
            .expect("Unable to initialize database");

        // Run the test
        let mut app = test::init_service(
            App::new()
                .app_data(web::Data::new(database.clone()))
                .configure(config),
        )
        .await;
        let req = test::TestRequest::get().uri("/brian").to_request();
        let response = test::call_service(&mut app, req).await;

        assert_eq!(response.status(), http::StatusCode::OK);
        let libs: std::vec::Vec<LibraryMetadata> = test::read_body_json(response).await;
        assert_eq!(libs.len(), 2);
        libs.iter().for_each(|lib| {
            assert_eq!(lib.owner, "brian");
        });
    }

    #[actix_web::test]
    async fn test_list_user_libraries_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_list_user_libraries_404() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_get_user_library() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_get_user_library_public() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_get_user_library_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_get_user_library_404() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_save_user_library() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_save_user_library_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_save_user_library_approval_req() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_user_library() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_user_library_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_user_library_404() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_publish_user_library() {
        let publish_name = "to-publish-example";
        let libraries = vec![
            LibraryMetadata::new("brian".into(), publish_name.into(), false, None),
            LibraryMetadata::new(
                "brian".into(),
                "private example".into(),
                false,
                None,
            ),
        ];
        let database = init_database("publish_user_lib", libraries)
            .await
            .expect("Unable to initialize database");

        // Run the test
        let mut app = test::init_service(
            App::new()
                .app_data(web::Data::new(database.clone()))
                .configure(config),
        )
        .await;
        let req = test::TestRequest::post()
            .uri(&format!("/brian/{}/publish", publish_name))
            .to_request();
        let response = test::call_service(&mut app, req).await;

        assert_eq!(response.status(), http::StatusCode::OK);
        let collection = database.collection::<LibraryMetadata>("libraries");
        let mut cursor = collection
            .find(doc! {}, None)
            .await
            .expect("Could not retrieve docs after publish");

        let mut count = 0;

        let libraries = cursor.try_collect::<Vec<_>>().await.unwrap();
        libraries.into_iter().for_each(|library| {
            let expected_public = library.name == publish_name;
            assert_eq!(
                library.public, expected_public,
                "Expected \"{}\" to have public value of {}",
                library.name, expected_public
            );
            count += 1;
        });
        assert_eq!(count, 2);
    }

    #[actix_web::test]
    async fn test_publish_user_library_approval_req() {
        unimplemented!();
    }

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
