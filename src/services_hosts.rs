use actix_web::{web, HttpResponse};
use actix_web::{get, post};
use mongodb::Database;
use mongodb::bson::doc;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
struct ThinUser {
    services_hosts: Vec<ServiceHost>,
}

#[derive(Serialize, Deserialize)]
struct ServiceHost {
    url: String,
    categories: Vec<String>,
}

#[get("/group/{id}")]
async fn list_group_hosts(db: web::Data<Database>) -> Result<HttpResponse, std::io::Error> {
    // TODO
    Ok(HttpResponse::Ok().json(true))
}

#[post("/group/{id}")]
async fn set_group_hosts(db: web::Data<Database>) -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
    // TODO: set up auth
    //let service_hosts: Vec<ServiceHost> = hosts.into_inner();
    //let update = doc! {"servicesHosts": service_hosts };
    //let filter = doc! {"_id": group_id};  // TODO: Check this
    //let result = collection.update_one(filter, update, None).await.expect("Unable to update group");
    //if result.modified_count == 1 {
        //Ok(HttpResponse::Ok().finish())
    //} else {
        //Ok(HttpResponse::NotFound().finish())
    //}
}

#[get("/user/{username}")]
async fn list_user_hosts(db: web::Data<Database>, path: web::Path<(String,)>) -> Result<HttpResponse, std::io::Error> {
    // TODO: check authorization (if requestor != username)
    let username = path.into_inner().0;
    let collection = db.collection::<ThinUser>("users");
    let filter = doc! {"username": username};
    let result = collection.find_one(filter, None).await.expect("User not found");  // FIXME: status code

    if let Some(user) = result {
        Ok(HttpResponse::Ok().json(user.services_hosts))
    } else {
        Ok(HttpResponse::NotFound().finish())
    }

}

#[post("/user/{username}")]
async fn set_user_hosts(db: web::Data<Database>, path: web::Path<(String,)>, hosts: web::Json<Vec<ServiceHost>>) -> Result<HttpResponse, std::io::Error> {
    // TODO: set up auth
    //let collection = db.collection::<ThinUser>("users");
    //let username = path.into_inner().0;
    //let service_hosts: Vec<ServiceHost> = hosts.into_inner();
    //let update = doc! {"servicesHosts": service_hosts };
    //let filter = doc! {"username": username};
    //let result = collection.update_one(filter, update, None).await.expect("Unable to update user");
    //if result.modified_count == 1 {
        //Ok(HttpResponse::Ok().finish())
    //} else {
        //Ok(HttpResponse::NotFound().finish())
    //}
    unimplemented!();
}

#[get("/all/{username}")]
async fn list_all_hosts(db: web::Data<Database>) -> Result<HttpResponse, std::io::Error> {
    // TODO
    Ok(HttpResponse::Ok().json(true))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(list_group_hosts)
        .service(set_group_hosts)
        .service(list_user_hosts)
        .service(set_user_hosts)
        .service(list_all_hosts);
}


