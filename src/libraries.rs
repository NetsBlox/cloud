use actix_web::{web, HttpResponse, HttpRequest, cookie::Cookie};
use actix_web::{get, post, delete};
use futures::stream::{TryStreamExt};
use mongodb::bson::doc;
use serde::{Serialize, Deserialize};
use mongodb::options::FindOptions;
use crate::database::Database;

#[derive(Serialize, Deserialize)]
struct Library {
    owner: String,
    name: String,
    notes: String,
    public: bool,
}

#[get("/community")]
async fn list_community_libraries(db: web::Data<Database>) -> Result<HttpResponse, std::io::Error> {
    let collection = db.collection::<Library>("libraries");

    let options = FindOptions::builder().sort(doc! {"name": 1}).build();
    let public_filter = doc! {"public": true};
    let mut cursor = collection.find(public_filter, options).await.expect("Library list query failed");

    let mut libraries = Vec::new();
    while let Some(library) = cursor.try_next().await.expect("Could not fetch library") {
        // TODO: should I stream this back?
        libraries.push(library);
    }

    Ok(HttpResponse::Ok().json(libraries))
}

#[get("/{owner}")]
async fn list_user_libraries(db: web::Data<Database>, path: web::Path<(String,)>, req: HttpRequest) -> Result<HttpResponse, std::io::Error> {
    // TODO: Get the user credentials
    let username = path.into_inner().0;
    let only_public = if let Some(cookie) = req.cookie("netsblox") {
        let requestor = cookie.value();
        requestor != username  // FIXME: Make this authentication better
    } else {
        true
    };

    let filter = if only_public {
        doc! {"owner": username, "public": true}
    } else {
        doc! {"owner": username}
    };
    let collection = db.collection::<Library>("libraries");
    let options = FindOptions::builder().sort(doc! {"name": 1}).build();
    let mut cursor = collection.find(filter, options).await.expect("Library list query failed");

    let mut libraries = Vec::new();
    while let Some(library) = cursor.try_next().await.expect("Could not fetch library") {
        // TODO: should I stream this back?
        libraries.push(library);
    }

    Ok(HttpResponse::Ok().json(libraries))
}

#[get("/{owner}/{name}")]
async fn get_user_library(db: web::Data<Database>, path: web::Path<(String,)>) -> Result<HttpResponse, std::io::Error> {
    // TODO: retrieve the library from the database
    // TODO: check the auth
    Ok(HttpResponse::Ok().body("Insert XML here"))
}

#[post("/{owner}/{name}")]
async fn save_user_library(db: web::Data<Database>, path: web::Path<(String,)>) -> Result<HttpResponse, std::io::Error> {
    // TODO: save the library to the database
    Ok(HttpResponse::Ok().finish())
}

#[delete("/{owner}/{name}")]
async fn delete_user_library(db: web::Data<Database>, path: web::Path<(String,)>) -> Result<HttpResponse, std::io::Error> {
    // TODO: delete the library from the database
    Ok(HttpResponse::Ok().finish())
}

#[post("/{owner}/{name}/publish")]
async fn publish_user_library(db: web::Data<Database>, path: web::Path<(String,)>) -> Result<HttpResponse, std::io::Error> {
    // TODO: verify that it is ok to publish
    // TODO: update the library info in the database
    Ok(HttpResponse::Ok().finish())
}

#[post("/{owner}/{name}/unpublish")]
async fn unpublish_user_library(db: web::Data<Database>, path: web::Path<(String,)>) -> Result<HttpResponse, std::io::Error> {
    // TODO: update the library info in the database
    Ok(HttpResponse::Ok().finish())
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(list_community_libraries)
        .service(list_user_libraries)
        .service(get_user_library)
        .service(save_user_library)
        .service(delete_user_library)
        .service(publish_user_library)
        .service(unpublish_user_library);
}

#[cfg(test)]
mod tests {
    use actix_web::test;

    #[actix_web::test]
    async fn test_list_community_libraries() {
        //let req = test::TestRequest::default().app_data(MockDatabase);  // TODO: add mock database
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_list_community_libraries_only_public() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_list_user_libraries() {  // TODO: 403 if not allowed?
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_get_user_library() {
        // TODO: check the contents matches?
        unimplemented!();
    }
}
