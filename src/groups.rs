use actix_web::{delete, get, patch, post};
use actix_web::{web, HttpResponse};

#[get("/")] // TODO: pass the owner ID
async fn list_groups() -> Result<HttpResponse, std::io::Error> {
    unimplemented!();
}

#[get("/{id}")]
async fn view_group() -> Result<HttpResponse, std::io::Error> {
    // TODO: add auth
    unimplemented!();
}

#[get("/{id}/members")]
async fn list_members() -> Result<HttpResponse, std::io::Error> {
    // TODO: add auth
    unimplemented!();
}

#[post("/")]
async fn create_group() -> Result<HttpResponse, std::io::Error> {
    // TODO: add auth
    unimplemented!();
}

#[patch("/{id}")]
async fn update_group() -> Result<HttpResponse, std::io::Error> {
    // TODO: add auth
    unimplemented!();
}

#[delete("/{id}")]
async fn delete_group() -> Result<HttpResponse, std::io::Error> {
    // TODO: add auth
    unimplemented!();
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_groups)
        .service(view_group)
        .service(list_members)
        .service(update_group)
        .service(create_group);
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http, test, App};
    use mongodb::{Client, Collection, Database};

    #[actix_web::test]
    async fn test_list_groups() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_list_groups_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_view_group() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_view_group_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_view_group_404() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_list_members() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_list_members_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_list_members_404() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_create_group() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_create_group_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_update_group() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_update_group_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_update_group_404() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_group() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_group_403() {
        unimplemented!();
    }

    #[actix_web::test]
    async fn test_delete_group_404() {
        unimplemented!();
    }
}
