use futures::stream::StreamExt;
use mongodb::{bson::doc, Client, Database};
use netsblox_cloud_common as cloud;

mod origin;

#[tokio::main]
async fn main() {
    // TODO: Add CLI args?
    let src_db = connect_db("mongodb:://127.0.0.1:27017/admin").await;
    let dst_db = connect_db("mongodb:://127.0.0.1:27017/netsblox_v2").await;

    // Convert users
    let src_users = src_db.collection::<origin::User>("users");
    let dst_users = dst_db.collection::<cloud::User>("users");
    let mut cursor = src_users
        .find(None, None)
        .await
        .expect("Unable to fetch users");

    while let Some(user) = cursor.next().await {
        let new_user: cloud::User = user.expect("Unable to retrieve user").into();
        let query = doc! {"username": &new_user.username};
        let update = doc! {"$setOnInsert": &new_user};
        dst_users
            .update_one(query, update, None)
            .await
            .unwrap_or_else(|_err| panic!("Unable to update username: {}", &new_user.username));
    }
    drop(src_users);

    // migrate groups
    let src_groups = src_db.collection::<origin::Group>("groups");
    let dst_groups = dst_db.collection::<cloud::Group>("groups");

    let query = doc! {};
    let mut cursor = src_groups.find(query, None).await.unwrap();

    while let Some(group) = cursor.next().await {
        let group = group.unwrap();
        if let Some(usernames) = group.members.clone() {
            for name in usernames {
                let query = doc! {"username": &name};
                let update = doc! {
                    "$set": {
                        "groupId": &group._id
                    }
                };

                dst_users
                    .update_one(query, update, None)
                    .await
                    .unwrap_or_else(|_err| panic!("Unable to set group for {}", &name));
            }
        }

        let new_group: cloud::Group = group.into();
        let query = doc! {"id": &new_group.id};
        let update = doc! {"$setOnInsert": &new_group};
        dst_groups
            .update_one(query, update, None)
            .await
            .unwrap_or_else(|_err| panic!("Unable to update group: {}", &new_group.id));
    }
    drop(src_groups);
    drop(dst_groups);

    // Convert libraries
    let src_libraries = src_db.collection::<origin::Library>("libraries");
    let dst_libraries = dst_db.collection::<cloud::Library>("libraries");
    let query = doc! {};
    let mut cursor = src_libraries.find(query, None).await.unwrap();

    while let Some(library) = cursor.next().await {
        let library = library.unwrap();
        let new_lib: cloud::Library = library.into();
        let query = doc! {
            "owner": &new_lib.owner,
            "name": &new_lib.name
        };
        let update = doc! {"$setOnInsert": &new_lib};
        dst_libraries.update_one(query, update, None).await.unwrap();
    }

    drop(src_libraries);
    drop(dst_libraries);

    // Convert banned accounts
    let src_bans = src_db.collection::<origin::BannedAccount>("bannedAccounts");
    let dst_bans = dst_db.collection::<cloud::BannedAccount>("bannedAccounts");
    let query = doc! {};
    let mut cursor = src_bans.find(query, None).await.unwrap();

    while let Some(account) = cursor.next().await {
        let account = account.unwrap();
        let new_acct: cloud::BannedAccount = account.into();
        let query = doc! {
            "username": &new_acct.username,
        };
        let update = doc! {"$setOnInsert": &new_acct};
        dst_bans.update_one(query, update, None).await.unwrap();
    }

    // TODO: Convert projects
}

async fn connect_db(mongo_uri: &str) -> Database {
    Client::with_uri_str(mongo_uri)
        .await
        .expect("Could not connect to mongodb.")
        .default_database()
        .expect("Could not connect to default source database")
}
