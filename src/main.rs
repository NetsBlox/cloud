use actix_web::{web, App, HttpResponse, HttpServer, middleware, get};
use serde::{Serialize, Deserialize};
use mongodb::{Client, Database};
use mongodb::options::FindOptions;
use mongodb::bson::doc;
use futures::stream::{TryStreamExt};

#[derive(Serialize, Deserialize)]
struct Library {
    owner: String,
    name: String,
    notes: String,
}

impl Library {
    fn new(owner: &str, name: &str, notes: &str) -> Library {
        Library{
            owner: owner.to_string(),
            name: name.to_string(),
            notes: notes.to_string()
        }
    }
}

struct AppState {
    db: Database,
}

// TODO: add routes for projects, users, etc
#[get("/libraries/community")]
async fn list_community_libraries(data: web::Data<AppState>) -> Result<HttpResponse, std::io::Error> {
    let collection = data.db.collection::<Library>("libraries");

    let options = FindOptions::builder().sort(doc! {"name": 1}).build();
    let mut cursor = collection.find(None, options).await.expect("Library list query failed");

    let mut libraries = Vec::new();  // TODO: convert this to a stream?
    while let Some(library) = cursor.try_next().await.expect("Could not fetch library") {
        libraries.push(library);
    }

    Ok(HttpResponse::Ok().json(libraries))

    // TODO: should I stream this back?
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let client = Client::with_uri_str("mongodb://127.0.0.1:27017/").await.expect("Could not connect to mongodb.");
    let db = client.database("netsblox-tests");

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .data(AppState{db: db.clone()})
            .service(list_community_libraries)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
