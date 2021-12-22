use crate::app_data::AppData;
use crate::models::ServiceHost;
use actix_session::Session;
use actix_web::{get, post};
use actix_web::{web, HttpResponse};
use mongodb::bson::{doc, oid::ObjectId};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThinUser {
    username: String,
    services_hosts: Vec<ServiceHost>,
}

#[derive(Serialize, Deserialize)]
struct ThinGroup {
    _id: ObjectId,
    services_hosts: Vec<ServiceHost>,
}

#[get("/group/{id}")]
async fn list_group_hosts(
    app: web::Data<AppData>,
    path: web::Path<(String,)>,
    session: Session,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: is group owner or super user
    let (group_id,) = path.into_inner();
    let collection = app.collection::<ThinGroup>("groups");
    Ok(HttpResponse::Ok().json(true))
}

#[post("/group/{id}")]
async fn set_group_hosts(
    db: web::Data<AppData>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
    // TODO: set up auth
    //let (group_id,) = path.into_inner();
    //let collection = db.collection::<Group>("groups");
    //let service_hosts: Vec<ServiceHost> = hosts.into_inner();
    //let update = doc! {"$set": {"servicesHosts": &service_hosts }};
    //let filter = doc! {"_id": group_id};  // TODO: Check this
    //let result = collection.update_one(filter, update, None).await.expect("Unable to update group");
    //if result.modified_count == 1 {
    //Ok(HttpResponse::Ok().finish())
    //} else {
    //Ok(HttpResponse::NotFound().finish())
    //}
}

#[get("/user/{username}")]
async fn list_user_hosts(
    db: web::Data<AppData>,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: check authorization (if requestor != username)
    let username = path.into_inner().0;
    let collection = db.collection::<ThinUser>("users");
    let filter = doc! {"username": username};
    let result = collection
        .find_one(filter, None)
        .await
        .expect("User not found"); // FIXME: status code

    if let Some(user) = result {
        Ok(HttpResponse::Ok().json(user.services_hosts))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[post("/user/{username}")]
async fn set_user_hosts(
    db: web::Data<AppData>,
    path: web::Path<(String,)>,
    hosts: web::Json<Vec<ServiceHost>>,
) -> Result<HttpResponse, std::io::Error> {
    // TODO: set up auth
    let collection = db.collection::<ThinUser>("users");
    let username = path.into_inner().0;
    let service_hosts: Vec<ServiceHost> = hosts.into_inner();
    let update = doc! {"$set": {"servicesHosts": &service_hosts }};
    let filter = doc! {"username": username};
    let result = collection
        .update_one(filter, update, None)
        .await
        .expect("Unable to update user");
    if result.modified_count == 1 {
        Ok(HttpResponse::Ok().finish())
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[get("/all/{username}")]
async fn list_all_hosts(db: web::Data<AppData>) -> Result<HttpResponse, std::io::Error> {
    // TODO
    Ok(HttpResponse::Ok().json(true))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_group_hosts)
        .service(set_group_hosts)
        .service(list_user_hosts)
        .service(set_user_hosts)
        .service(list_all_hosts);
}

mod test {}
