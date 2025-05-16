use crate::{auth, utils};
use aws_sdk_s3 as s3;
use mongodb::bson::DateTime;
use std::collections::HashMap;

use futures::TryStreamExt;
use mongodb::{bson::doc, options::ReturnDocument, Collection};
use netsblox_cloud_common::{api, Assignment, Bucket, Group, Submission, User};

use crate::errors::{InternalError, UserError};

pub(crate) struct GroupActions<'a> {
    groups: &'a Collection<Group>,
    users: &'a Collection<User>,
    assignments: &'a Collection<Assignment>,
    submissions: &'a Collection<Submission>,
    bucket: &'a Bucket,
    s3: &'a s3::Client,
}

impl<'a> GroupActions<'a> {
    pub(crate) fn new(
        groups: &'a Collection<Group>,
        users: &'a Collection<User>,
        assignments: &'a Collection<Assignment>,
        submissions: &'a Collection<Submission>,
        bucket: &'a Bucket,
        s3: &'a s3::Client,
    ) -> Self {
        Self {
            groups,
            users,
            assignments,
            submissions,
            bucket,
            s3,
        }
    }

    pub(crate) async fn create_group(
        &self,
        eu: &auth::EditUser,
        data: api::CreateGroupData,
    ) -> Result<api::Group, UserError> {
        let group = Group::from_data(eu.username.clone(), data);
        let query = doc! {
            "name": &group.name,
            "owner": &group.owner,
        };
        let update = doc! {"$setOnInsert": &group};
        let options = mongodb::options::UpdateOptions::builder()
            .upsert(true)
            .build();

        let result = self
            .groups
            .update_one(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if result.matched_count == 1 {
            Err(UserError::GroupExistsError)
        } else {
            let group: api::Group = group.into();
            Ok(group)
        }
    }

    pub(crate) async fn list_groups(
        &self,
        vg: &auth::users::ViewUser,
    ) -> Result<Vec<api::Group>, UserError> {
        let query = doc! {"owner": &vg.username};
        let cursor = self
            .groups
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;
        let groups: Vec<api::Group> = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|g| g.into())
            .collect();

        Ok(groups)
    }

    pub(crate) async fn view_group(
        &self,
        vg: &auth::groups::ViewGroup,
    ) -> Result<api::Group, UserError> {
        let query = doc! {"id": &vg.id};
        let group = self
            .groups
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GroupNotFoundError)?;

        Ok(group.into())
    }

    pub(crate) async fn rename_group(
        &self,
        eg: &auth::groups::EditGroup,
        name: &str,
    ) -> Result<api::Group, UserError> {
        let query = doc! {"id": &eg.id};
        let update = doc! {"$set": {"name": &name}};
        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let group = self
            .groups
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GroupNotFoundError)?;

        Ok(group.into())
    }

    pub(crate) async fn set_group_hosts(
        &self,
        eg: &auth::groups::EditGroup,
        hosts: &[api::ServiceHost],
    ) -> Result<api::Group, UserError> {
        let query = doc! {"id": &eg.id};
        let update = doc! {"$set": {"servicesHosts": hosts}};
        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();
        let group = self
            .groups
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GroupNotFoundError)?;

        Ok(group.into())
    }

    pub(crate) async fn get_service_settings(
        &self,
        vg: &auth::groups::ViewGroup,
    ) -> Result<HashMap<String, String>, UserError> {
        let query = doc! {"id": &vg.id};
        let group = self
            .groups
            .find_one(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::UserNotFoundError)?;

        Ok(group.service_settings)
    }

    pub(crate) async fn set_service_settings(
        &self,
        vg: &auth::groups::EditGroup,
        host: &str,
        settings: &str,
    ) -> Result<api::Group, UserError> {
        let query = doc! {"id": &vg.id};
        let update = doc! {"$set": {format!("serviceSettings.{}", &host): settings}};
        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let group = self
            .groups
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GroupNotFoundError)?;

        Ok(group.into())
    }

    pub(crate) async fn delete_service_settings(
        &self,
        vg: &auth::groups::EditGroup,
        host: &str,
    ) -> Result<api::Group, UserError> {
        let query = doc! {"id": &vg.id};
        let update = doc! {"$unset": {format!("serviceSettings.{}", &host): true}};
        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let group = self
            .groups
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GroupNotFoundError)?;

        Ok(group.into())
    }

    pub(crate) async fn delete_group(
        &self,
        vg: &auth::groups::DeleteGroup,
    ) -> Result<api::Group, UserError> {
        let query = doc! {"id": &vg.id};
        let group = self
            .groups
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::GroupNotFoundError)?;

        Ok(group.into())
    }

    // TODO: move this to the user actions??
    pub(crate) async fn list_members(
        &self,
        vg: &auth::groups::ViewGroup,
    ) -> Result<Vec<api::User>, UserError> {
        let query = doc! {"groupId": &vg.id};
        // TODO:
        let cursor = self
            .users
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;
        let members: Vec<api::User> = cursor
            .try_collect::<Vec<_>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .into_iter()
            .map(|u| u.into())
            .collect();

        Ok(members)
    }

    pub(crate) async fn create_assignment(
        &self,
        auth_ca: &auth::CreateAssignment,
    ) -> Result<api::Assignment, UserError> {
        let assignment = Assignment::from_data(auth_ca.ca_data.clone(), auth_ca.group_id.clone());
        let query = doc! {
            "name": &assignment.name,
            "groupId": &assignment.group_id,
        };
        let update = doc! {"$setOnInsert": &assignment};
        let options = mongodb::options::UpdateOptions::builder()
            .upsert(true)
            .build();

        let res = self
            .assignments
            .update_one(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        if res.matched_count == 1 {
            Err(UserError::AssignmentExistsError)
        } else {
            Ok(api::Assignment::from(assignment))
        }
    }

    pub(crate) async fn view_group_assignments(
        &self,
        auth_vga: &auth::ViewGroupAssignments,
    ) -> Result<Vec<api::Assignment>, UserError> {
        let query = doc! {"groupId": auth_vga.group_id.clone()};
        let assignments: Vec<api::Assignment> = self
            .assignments
            .find(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .map_ok(api::Assignment::from)
            .try_collect()
            .await
            .map_err(InternalError::DatabaseConnectionError)?;
        Ok(assignments)
    }

    pub(crate) async fn edit_assignment(
        &self,
        auth_ea: &auth::EditAssignment,
        update_data: api::UpdateAssignmentData,
    ) -> Result<api::Assignment, UserError> {
        let query = doc! {"id": &auth_ea.assignment.id};

        let mut set_doc = doc! {};
        if let Some(name) = update_data.name.clone() {
            set_doc.insert("name", name);
        }
        if let Some(due_date) = update_data.due_date {
            set_doc.insert("state", DateTime::from(due_date));
        }
        let update = doc! { "$set": set_doc };

        let options = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let assignment = self
            .assignments
            .find_one_and_update(query, update, options)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::AssignmentNotFoundError)?;

        Ok(api::Assignment::from(assignment))
    }

    pub(crate) async fn delete_assignment(
        &self,
        auth_da: &auth::DeleteAssignment,
    ) -> Result<api::Assignment, UserError> {
        let query = doc! {"id": &auth_da.assignment.id};

        let assignment = self
            .assignments
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::AssignmentNotFoundError)?;

        Ok(api::Assignment::from(assignment))
    }

    pub(crate) async fn create_submission(
        &self,
        auth_cs: &auth::CreateSubmission,
    ) -> Result<api::Submission, UserError> {
        let assignment_id = auth_cs.assignment_id.clone();
        let cs_data = auth_cs.cs_data.clone();
        let xml = cs_data.xml.clone();
        let submission = Submission::from_data(assignment_id, cs_data);

        self.submissions
            .insert_one(&submission, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        utils::upload(self.s3, self.bucket, &submission.key, xml).await?;

        Ok(api::Submission::from(submission))
    }

    pub(crate) async fn view_user_submissions(
        &self,
        auth_vus: &auth::ViewOwnerSubmissions,
    ) -> Result<Vec<api::Submission>, UserError> {
        let owner = auth_vus.owner.clone();
        let assignment_id = auth_vus.assignment_id.clone();
        let filter = doc! {"owner": owner, "assignmentId": assignment_id};
        let submissions = self
            .submissions
            .find(filter, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .try_filter_map(|s| async move { Ok(Some(api::Submission::from(s))) })
            .try_collect::<Vec<api::Submission>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(submissions)
    }

    pub(crate) async fn view_assignment_submissions(
        &self,
        auth_vas: &auth::ViewAssignmentSubmissions,
    ) -> Result<Vec<api::Submission>, UserError> {
        let filter = doc! {"assignmentId": auth_vas.assignment.id.clone()};
        let submissions = self
            .submissions
            .find(filter, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .try_filter_map(|s| async move { Ok(Some(api::Submission::from(s))) })
            .try_collect::<Vec<api::Submission>>()
            .await
            .map_err(InternalError::DatabaseConnectionError)?;

        Ok(submissions)
    }

    pub(crate) async fn delete_submission(
        &self,
        auth_ds: &auth::DeleteSubmission,
    ) -> Result<api::Submission, UserError> {
        let query = doc! {"id": auth_ds.submission.id.clone()};

        let submission = self
            .submissions
            .find_one_and_delete(query, None)
            .await
            .map_err(InternalError::DatabaseConnectionError)?
            .ok_or(UserError::SubmissionNotFoundError)?;

        let s3_res = utils::delete(self.s3, self.bucket, submission.key.clone()).await;

        if let Err(s3_err) = s3_res {
            self.submissions
                .insert_one(&submission, None)
                .await
                .map_err(InternalError::DatabaseConnectionError)?;
            Err(s3_err)
        } else {
            Ok(api::Submission::from(submission))
        }
    }

    pub(crate) async fn view_submission_xml(
        &self,
        auth_vs: &auth::ViewSubmission,
    ) -> Result<String, UserError> {
        let key = auth_vs.submission.key.clone();
        let xml = utils::download(self.s3, self.bucket, &key).await?;
        Ok(xml)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils;

    use super::*;

    #[actix_web::test]
    async fn test_create_group_with_hosts() {
        let user: User = api::NewUser {
            username: "user".into(),
            email: "user@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();
        let other: User = api::NewUser {
            username: "other".into(),
            email: "other@netsblox.org".into(),
            password: None,
            group_id: None,
            role: None,
        }
        .into();

        test_utils::setup()
            .with_users(&[user.clone(), other])
            .run(|app_data| async move {
                let actions = app_data.as_group_actions();

                let auth_eu = auth::EditUser::test(user.username.clone());

                // create the group
                let hosts = vec![api::ServiceHost {
                    url: "http://testUrl.org".into(),
                    categories: vec!["someCategory".into()],
                }];
                let data = api::CreateGroupData {
                    name: "someGroup".into(),
                    services_hosts: Some(hosts),
                };
                let group = actions.create_group(&auth_eu, data).await.unwrap();

                // check that it has a service host
                let services_hosts = group.services_hosts.unwrap_or_default();
                assert_eq!(services_hosts.len(), 1);
                let host = services_hosts.first().unwrap();
                assert_eq!(&host.url, "http://testUrl.org");
            })
            .await;
    }
}
