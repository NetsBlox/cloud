use futures::TryStreamExt;
use mongodb::{
    bson::doc,
    options::{FindOneAndUpdateOptions, FindOptions, ReturnDocument},
    Collection,
};
use netsblox_cloud_common::{
    api::{self, PublishState},
    Library,
};

use crate::{
    auth,
    errors::{InternalError, UserError},
    utils,
};

pub(crate) struct LibraryActions<'a> {
    libraries: &'a Collection<Library>,
}

impl<'a> LibraryActions<'a> {
    pub(crate) fn new(libraries: &'a Collection<Library>) -> Self {
        Self { libraries }
    }

    pub(crate) async fn list_community_libraries(
        &self,
    ) -> Result<Vec<api::LibraryMetadata>, UserError> {
        let options = FindOptions::builder().sort(doc! {"name": 1}).build();
        let public_filter = doc! {"state": PublishState::Public};
        let cursor = self
            .libraries
            .find(public_filter, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        let libraries: Vec<api::LibraryMetadata> = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|lib| lib.into())
            .collect();

        Ok(libraries)
    }

    pub(crate) async fn list_user_libraries(
        &self,
        ll: &auth::ListLibraries,
    ) -> Result<Vec<api::LibraryMetadata>, UserError> {
        let query = if ll.visibility == PublishState::Private {
            doc! {"owner": &ll.username}
        } else {
            doc! {
            "owner": &ll.username,
            "state": PublishState::Public,
            }
        };

        let options = FindOptions::builder().sort(doc! {"name": 1}).build();
        let libraries: Vec<_> = self
            .libraries
            .find(query, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|lib| lib.into())
            .collect();

        Ok(libraries)
    }

    pub(crate) fn get_library_code(vl: &auth::ViewLibrary) -> String {
        vl.library.blocks.clone()
    }

    pub(crate) async fn save_library(
        &self,
        el: &auth::EditLibrary,
        data: &api::CreateLibraryData,
    ) -> Result<api::LibraryMetadata, UserError> {
        let query = doc! {"owner": &el.owner, "name": &data.name};
        let update = doc! {
            "$set": {
                "notes": &data.notes,
                "blocks": &data.blocks,
            },
            "$setOnInsert": {
                "owner": &el.owner,
                "name": &data.name,
                "state": PublishState::Private,
            }
        };
        let options = FindOneAndUpdateOptions::builder()
            .upsert(true)
            .return_document(ReturnDocument::After)
            .build();

        let library = self
            .libraries
            .find_one_and_update(query.clone(), update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::LibraryNotFoundError)?; // this shouldn't happen since we are upserting

        // Check if we need to demote it to "needs approval"
        let needs_approval = if matches!(library.state, PublishState::Public) {
            utils::is_approval_required(&library.blocks)
        } else {
            false
        };

        if needs_approval {
            let update = doc! {
                "$set": {
                    "state": PublishState::PendingApproval
                }
            };
            let options = FindOneAndUpdateOptions::builder()
                .return_document(ReturnDocument::After)
                .build();
            let library = self
                .libraries
                .find_one_and_update(query, update, options)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .ok_or(UserError::LibraryNotFoundError)?;

            Ok(library.into())
        } else {
            Ok(library.into())
        }
    }

    pub(crate) async fn delete_library(
        &self,
        vl: &auth::EditLibrary,
        name: &api::LibraryName,
    ) -> Result<api::LibraryMetadata, UserError> {
        let query = doc! {"owner": &vl.owner, "name": name};
        let library = self
            .libraries
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::LibraryNotFoundError)?;

        Ok(library.into())
    }

    pub(crate) async fn publish(
        &self,
        pl: &auth::PublishLibrary,
        name: &api::LibraryName,
    ) -> Result<api::LibraryMetadata, UserError> {
        let query = doc! {"owner": &pl.owner, "name": name};
        let update = doc! {"$set": {"state": PublishState::PendingApproval}};

        let library = self
            .libraries
            .find_one_and_update(query.clone(), update, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::LibraryNotFoundError)?;

        let can_approve = !utils::is_approval_required(&library.blocks) || pl.can_approve;

        if can_approve {
            let update = doc! {"$set": {"state": PublishState::Public}};
            let options = FindOneAndUpdateOptions::builder()
                .return_document(ReturnDocument::After)
                .build();

            let library = self
                .libraries
                .find_one_and_update(query, update, options)
                .await
                .map_err(InternalError::DatabaseConnectionError)?
                .ok_or(UserError::LibraryNotFoundError)?;

            Ok(library.into())
        } else {
            Ok(library.into())
        }
    }

    pub(crate) async fn unpublish(
        &self,
        pl: &auth::PublishLibrary,
        name: &api::LibraryName,
    ) -> Result<api::LibraryMetadata, UserError> {
        let query = doc! {"owner": &pl.owner, "name": name};
        let update = doc! {"$set": {"state": PublishState::Private}};
        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let library = self
            .libraries
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::LibraryNotFoundError)?;

        Ok(library.into())
    }

    pub(crate) async fn list_pending_libraries(
        &self,
        _ml: &auth::ModerateLibraries,
    ) -> Result<Vec<api::LibraryMetadata>, UserError> {
        let cursor = self
            .libraries
            .find(doc! {"state": PublishState::PendingApproval}, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        let libraries = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|lib| lib.into())
            .collect();

        Ok(libraries)
    }

    pub(crate) async fn set_library_state(
        &self,
        _ml: &auth::ModerateLibraries,
        owner: &str,
        name: &api::LibraryName,
        state: api::PublishState,
    ) -> Result<api::LibraryMetadata, UserError> {
        let query = doc! {"owner": owner, "name": name};
        let update = doc! {"$set": {"state": state}};
        let options = FindOneAndUpdateOptions::builder()
            .upsert(true)
            .return_document(ReturnDocument::After)
            .build();
        let library = self
            .libraries
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::LibraryNotFoundError)?;

        Ok(library.into())
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils;

    use super::*;
    use netsblox_cloud_common::User;

    #[actix_web::test]
    async fn test_save_user_lib() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        test_utils::setup()
            .with_users(&[user.clone()])
            .run(|app_data| async move {
                let actions = app_data.as_library_actions();
                let auth_el = auth::EditLibrary::test(user.username.clone());
                let data = api::CreateLibraryData {
                    name: api::LibraryName::new("mylibrary"),
                    notes: "some notes".into(),
                    blocks: "<blocks/>".into(),
                };
                actions.save_library(&auth_el, &data).await.unwrap();

                let query = doc! {};
                let metadata = app_data.libraries.find_one(query, None).await.unwrap();

                assert!(metadata.is_some(), "Library not found in the database");
                let metadata = metadata.unwrap();
                assert_eq!(metadata.name.as_str(), "mylibrary");
            })
            .await;
    }

    #[actix_web::test]
    async fn test_list_user_libs_public() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let pub1 = Library {
            owner: user.username.clone(),
            name: api::LibraryName::new("pub1"),
            notes: "".into(),
            blocks: "<blocks/>".into(),
            state: api::PublishState::Public,
        };
        let pub2 = Library {
            owner: user.username.clone(),
            name: api::LibraryName::new("pub2"),
            notes: "".into(),
            blocks: "<blocks/>".into(),
            state: api::PublishState::Public,
        };
        let private = Library {
            owner: user.username.clone(),
            name: api::LibraryName::new("priv"),
            notes: "".into(),
            blocks: "<blocks/>".into(),
            state: api::PublishState::Private,
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_libraries(&[pub1.clone(), pub2.clone(), private])
            .run(|app_data| async move {
                let actions = app_data.as_library_actions();
                let auth_ll =
                    auth::ListLibraries::test(user.username.clone(), PublishState::Public);

                let libraries = actions.list_user_libraries(&auth_ll).await.unwrap();
                assert_eq!(libraries.len(), 2);
                assert!(libraries.iter().any(|lib| lib.name == pub1.name));
                assert!(libraries.iter().any(|lib| lib.name == pub2.name));
            })
            .await;
    }

    #[actix_web::test]
    async fn test_list_user_libs_private() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let pub1 = Library {
            owner: user.username.clone(),
            name: api::LibraryName::new("pub1"),
            notes: "".into(),
            blocks: "<blocks/>".into(),
            state: api::PublishState::Public,
        };
        let pub2 = Library {
            owner: user.username.clone(),
            name: api::LibraryName::new("pub2"),
            notes: "".into(),
            blocks: "<blocks/>".into(),
            state: api::PublishState::Public,
        };
        let private = Library {
            owner: user.username.clone(),
            name: api::LibraryName::new("priv"),
            notes: "".into(),
            blocks: "<blocks/>".into(),
            state: api::PublishState::Private,
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_libraries(&[pub1, pub2, private])
            .run(|app_data| async move {
                let actions = app_data.as_library_actions();
                let auth_ll =
                    auth::ListLibraries::test(user.username.clone(), PublishState::Private);

                let libraries = actions.list_user_libraries(&auth_ll).await.unwrap();
                // Should get all the libraries since it has permissions to view private libraries
                assert_eq!(libraries.len(), 3);
            })
            .await;
    }

    #[actix_web::test]
    async fn test_set_library_state() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let lib = Library {
            owner: user.username.clone(),
            name: api::LibraryName::new("lib"),
            notes: "".into(),
            blocks: "<blocks/>".into(),
            state: api::PublishState::PendingApproval,
        };

        test_utils::setup()
            .with_users(&[user.clone()])
            .with_libraries(&[lib.clone()])
            .run(|app_data| async move {
                let actions = app_data.as_library_actions();
                let auth_ml = auth::ModerateLibraries::test();

                let metadata = actions
                    .set_library_state(&auth_ml, &user.username, &lib.name, PublishState::Public)
                    .await
                    .unwrap();

                assert!(matches!(metadata.state, PublishState::Public));
            })
            .await;
    }
}
