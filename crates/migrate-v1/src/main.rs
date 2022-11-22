use mongodb::{Client, Database};
use netsblox_cloud_common as cloud;

mod origin;

#[tokio::main]
async fn main() {
    // TODO: Add CLI args?
    // TODO: Create the database models from JS version
    let src_db = connect_db(&"mongodb:://127.0.0.1:27017/admin").await;
    let dst_db = connect_db(&"mongodb:://127.0.0.1:27017/netsblox_v2").await;

    // TODO: Convert users
    let src_users = src_db.collection::<origin::User>("users");
    let dst_users = dst_db.collection::<cloud::User>("users");
    let cursor = src_users
        .find(None, None)
        .await
        .expect("Unable to fetch users");
    // TODO: can any of the conversions fail?
    // TODO: Convert groups
    // TODO: Convert libraries
    // TODO: Convert projects
    // TODO: Convert banned accounts
    //let groups = db.collection::<Group>(&(prefix.to_owned() + "groups"));
}

async fn connect_db(mongo_uri: &str) -> Database {
    Client::with_uri_str(mongo_uri)
        .await
        .expect("Could not connect to mongodb.")
        .default_database()
        .expect("Could not connect to default source database")
}

fn migrate<S, D>(src_db: &Database, dst_db: &Database, src_name: &str, dst_name: &str) {
    let src = src_db.collection::<S>(src_name);
    let dst = dst_db.collection::<D>(dst_name);

    // TODO: migrate all the given types...
    todo!();
}
