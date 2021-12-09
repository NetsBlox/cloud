use crate::database::Database;
use actix_web::{delete, get, post};
use actix_web::{web, HttpRequest, HttpResponse};
use futures::stream::TryStreamExt;
use lazy_static::lazy_static;
use mongodb::bson::doc;
use mongodb::options::FindOptions;
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
            notes: notes.unwrap_or("".to_string()),
            public,
        }
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
async fn list_community_libraries(db: web::Data<Database>) -> Result<HttpResponse, std::io::Error> {
    println!("listing community libraries");
    let collection = db.collection::<LibraryMetadata>("libraries");

    let options = FindOptions::builder().sort(doc! {"name": 1}).build();
    let public_filter = doc! {"public": true};
    let mut cursor = collection
        .find(public_filter, options)
        .await
        .expect("Library list query failed");

    let mut libraries = Vec::new();
    while let Some(library) = cursor.try_next().await.expect("Could not fetch library") {
        // TODO: should I stream this back?
        libraries.push(library);
    }

    Ok(HttpResponse::Ok().json(libraries))
}

#[get("/{owner}")] // TODO: scope these under user/? currently, this wont work if the username is "community"
async fn list_user_libraries(
    db: web::Data<Database>,
    path: web::Path<(String,)>,
    req: HttpRequest,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: Get the user credentials
    let username = path.into_inner().0;
    let only_public = if let Some(cookie) = req.cookie("netsblox") {
        let requestor = cookie.value();
        requestor != username // FIXME: Make this authentication better
    } else {
        true
    };

    let filter = if only_public {
        doc! {"owner": username, "public": true}
    } else {
        doc! {"owner": username}
    };
    let collection = db.collection::<LibraryMetadata>("libraries");
    let options = FindOptions::builder().sort(doc! {"name": 1}).build();
    let mut cursor = collection
        .find(filter, options)
        .await
        .expect("Library list query failed");

    let mut libraries = Vec::new();
    while let Some(library) = cursor.try_next().await.expect("Could not fetch library") {
        // TODO: should I stream this back?
        libraries.push(library);
    }

    Ok(HttpResponse::Ok().json(libraries))
}

#[get("/{owner}/{name}")]
async fn get_user_library(
    db: web::Data<Database>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: retrieve the library from the database
    // TODO: check the auth
    let collection = db.collection::<Library>("libraries");
    let (owner, name) = path.into_inner();
    let query = doc! {"owner": owner, "name": name};
    // TODO: get the library post data
    if let Some(library) = collection
        .find_one(query, None)
        .await
        .expect("Unable to retrieve from database")
    {
        Ok(HttpResponse::Ok().body(library.blocks))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[post("/{owner}/{name}")]
async fn save_user_library(
    db: web::Data<Database>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, std::io::Error> {
    let (owner, name) = path.into_inner();
    if !is_valid_name(&name) {
        return Ok(HttpResponse::BadRequest().body("Invalid library name"));
    }

    // TODO: authenticate
    let collection = db.collection::<LibraryMetadata>("libraries");
    let query = doc! {"owner": &owner, "name": &name};
    // TODO: get the library post data. What should this include? xml, notes, etc?

    // TODO: check if it needs re-approval?
    //let update = doc!{"$set": {"owner": owner, "name": name, "blocks": blocks}};
    let update = doc! {"$set": {"owner": &owner, "name": &name}}; // FIXME:
    let result = collection
        .update_one(query, update, None)
        .await
        .expect("Unable to save in database");
    if result.matched_count == 0 {
        Ok(HttpResponse::Created().finish())
    } else {
        Ok(HttpResponse::Ok().finish())
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

#[delete("/{owner}/{name}")]
async fn delete_user_library(
    db: web::Data<Database>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: authenticate
    let collection = db.collection::<LibraryMetadata>("libraries");
    let (owner, name) = path.into_inner();
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

#[post("/{owner}/{name}/publish")]
async fn publish_user_library(
    db: web::Data<Database>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: get the requestor and authorize
    let collection = db.collection::<LibraryMetadata>("libraries");
    let (owner, name) = path.into_inner();

    // TODO: check if approval is required
    let query = doc! {"owner": owner, "name": name};
    let update = doc! {"$set": {"public": true}};
    let result = collection
        .update_one(query, update, None)
        .await
        .expect("Library publish operation failed");
    if result.matched_count == 0 {
        Ok(HttpResponse::NotFound().finish())
    } else {
        Ok(HttpResponse::Ok().finish())
    }
}

#[post("/{owner}/{name}/unpublish")]
async fn unpublish_user_library(
    db: web::Data<Database>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: update the library info in the database
    // TODO: get the requestor and authorize
    let collection = db.collection::<LibraryMetadata>("libraries");
    let (owner, name) = path.into_inner();

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
                "brian".to_string(),
                "public example".to_string(),
                true,
                None,
            ),
            LibraryMetadata::new(
                "brian".to_string(),
                "private example".to_string(),
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
            LibraryMetadata::new("cassie".to_string(), "project 1".to_string(), false, None),
            LibraryMetadata::new("brian".to_string(), "project 2".to_string(), false, None),
            LibraryMetadata::new("brian".to_string(), "project 3".to_string(), true, None),
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
        let publish_name = "to-publish-example".to_string();
        let libraries = vec![
            LibraryMetadata::new("brian".to_string(), publish_name.clone(), false, None),
            LibraryMetadata::new(
                "brian".to_string(),
                "private example".to_string(),
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

        while let Some(library) = cursor.try_next().await.expect("Could not fetch library") {
            let expected_public = library.name == publish_name;
            assert_eq!(
                library.public, expected_public,
                "Expected \"{}\" to have public value of {}",
                library.name, expected_public
            );
            count += 1;
        }
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
