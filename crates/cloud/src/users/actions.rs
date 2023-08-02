pub(crate) struct UserActions {
    // TODO: can I make cached projects?
    // project_metadata: Collection<ProjectMetadata>,
    // bucket: String,
    // s3: S3Client,
    // network: Addr<TopologyActor>,
}

impl UserActions {
    // TODO: How can we conditionally check? Maybe have an edit group value for no group?
    pub(crate) async fn create_user(&self) -> Result<api::User, UserError> {
        todo!()
    }
}
