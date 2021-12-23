use crate::models::{CollaborationInvitation, Group, Project, ProjectMetadata, User};
use crate::models::{RoleData, RoleMetadata};
use crate::network::topology::Topology;
use actix::{Actor, Addr};
use mongodb::{Collection, Database};
use rusoto_s3::S3Client;

pub struct AppData {
    prefix: &'static str,
    pub db: Database,
    pub network: Addr<Topology>,
    pub s3: S3Client,
    pub groups: Collection<Group>,
    pub users: Collection<User>,
    pub project_metadata: Collection<ProjectMetadata>,
    pub collab_invites: Collection<CollaborationInvitation>,
}

impl AppData {
    pub fn new(
        db: Database,
        s3: S3Client,
        network: Option<Addr<Topology>>,
        prefix: Option<&'static str>,
    ) -> AppData {
        let network = network.unwrap_or(Topology::new().start());
        let prefix = prefix.unwrap_or("");
        let groups = db.collection::<Group>(&(prefix.to_owned() + "groups"));
        let users = db.collection::<User>(&(prefix.to_owned() + "users"));
        let project_metadata = db.collection::<ProjectMetadata>(&(prefix.to_owned() + "projects"));
        let collab_invites = db.collection::<CollaborationInvitation>(
            &(prefix.to_owned() + "collaborationInvitations"),
        );
        AppData {
            db,
            network,
            s3,
            groups,
            users,
            prefix,
            project_metadata,

            collab_invites,
        }
    }

    pub fn collection<T>(&self, name: &str) -> Collection<T> {
        let name = &(self.prefix.to_owned() + name);
        self.db.collection::<T>(name)
    }

    pub async fn fetch_project(&self, metadata: &ProjectMetadata) -> Project {
        // TODO: populate the source code, media for each role
        todo!();
    }

    pub async fn delete_project(&self, metadata: ProjectMetadata) -> bool {
        todo!();
    }

    pub async fn fetch_role(&self, metadata: &RoleMetadata) -> RoleData {
        todo!();
    }
}

// TODO: add projects
//struct Projects {
//metadata:
//
//}
