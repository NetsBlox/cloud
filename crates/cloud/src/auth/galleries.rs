use crate::{app_data::AppData, errors::InternalError};
use actix_web::HttpRequest;
use mongodb::bson::doc;
use netsblox_cloud_common::{api, Gallery};

use crate::errors::UserError;

// Permissions on galleries
pub(crate) struct ViewGallery {
    pub(crate) metadata: Gallery,
    _private: (),
}

pub(crate) struct EditGallery {
    pub(crate) metadata: Gallery,
    _private: (),
}

pub(crate) struct DeleteGallery {
    pub(crate) metadata: Gallery,
    _private: (),
}

// functions to try to obtain the given permissions
pub(crate) async fn try_view_gallery(
    app: &AppData,
    req: &HttpRequest,
    id: &api::GalleryId,
) -> Result<ViewGallery, UserError> {
    // for now you can only view the gallery if you are allowed to edit it
    try_edit_gallery(app, req, id).await.map(|eg| ViewGallery {
        metadata: eg.metadata.to_owned(),
        _private: (),
    })
}

pub(crate) async fn try_edit_gallery(
    app: &AppData,
    req: &HttpRequest,
    id: &api::GalleryId,
) -> Result<EditGallery, UserError> {
    let query = doc! {"id": id};
    let result = app
        .galleries
        .find_one(query, None)
        .await
        .map_err(InternalError::DatabaseConnectionError)?
        .ok_or(UserError::GalleryNotFoundError)?;

    // return early with error if user is not owner
    super::try_edit_user(app, req, None, &result.owner).await?;

    // else, return permissions EditGallery
    Ok(EditGallery {
        metadata: result,
        _private: (),
    })
}

/// Try to obtain permissions to delete a gallery. Only gallery owners
/// are allowed to delete the group.
pub(crate) async fn try_delete_gallery(
    app: &AppData,
    req: &HttpRequest,
    id: &api::GalleryId,
) -> Result<DeleteGallery, UserError> {
    // for now you can only delete the gallery if you are allowed to edit it
    try_edit_gallery(app, req, id)
        .await
        .map(|eg| DeleteGallery {
            metadata: eg.metadata.to_owned(),
            _private: (),
        })
}

//TODO: Add tests
#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{get, HttpResponse};

    #[actix_web::test]
    async fn test_try_edit_gallery_owner() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_try_edit_group_other() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_try_edit_group_admin() {
        unimplemented!();
    }

    #[get("/test")]
    async fn view_test() -> Result<HttpResponse, UserError> {
        unimplemented!();
    }

    #[get("/test")]
    async fn edit_test() -> Result<HttpResponse, UserError> {
        unimplemented!();
    }

    #[get("/test")]
    async fn delete_test() -> Result<HttpResponse, UserError> {
        unimplemented!();
    }
}
