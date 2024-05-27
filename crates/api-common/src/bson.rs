use crate::{
    oauth, FriendInvite, FriendLinkState, GroupId, InvitationState, LinkedAccount, MagicLinkId,
    Name, ProjectId, ProjectName, PublishState, RoleMetadata, SaveState, ServiceHost,
    ServiceHostScope, UserRole,
};
use bson::{doc, Bson, DateTime};

impl From<ServiceHost> for Bson {
    fn from(host: ServiceHost) -> Bson {
        Bson::Document(doc! {
            "url": host.url,
            "categories": host.categories
        })
    }
}

impl From<LinkedAccount> for Bson {
    fn from(account: LinkedAccount) -> Bson {
        Bson::Document(doc! {
            "username": account.username,
            "strategy": account.strategy,
        })
    }
}

impl From<FriendLinkState> for Bson {
    fn from(link_state: FriendLinkState) -> Bson {
        match link_state {
            FriendLinkState::Pending => Bson::String("Pending".into()),
            FriendLinkState::Approved => Bson::String("Approved".into()),
            FriendLinkState::Rejected => Bson::String("Rejected".into()),
            FriendLinkState::Blocked => Bson::String("Blocked".into()),
        }
    }
}

impl From<UserRole> for Bson {
    fn from(role: UserRole) -> Bson {
        match role {
            UserRole::Admin => Bson::String("admin".into()),
            UserRole::Moderator => Bson::String("moderator".into()),
            UserRole::User => Bson::String("user".into()),
            UserRole::Teacher => Bson::String("teacher".into()),
        }
    }
}

impl From<FriendInvite> for Bson {
    fn from(invite: FriendInvite) -> Bson {
        Bson::Document(doc! {
            "id": invite.id,
            "sender": invite.sender,
            "recipient": invite.recipient,
            "createdAt": DateTime::from_system_time(invite.created_at),
        })
    }
}

impl From<SaveState> for Bson {
    fn from(state: SaveState) -> Bson {
        match state {
            SaveState::Created => Bson::String("Created".to_string()),
            SaveState::Transient => Bson::String("Transient".to_string()),
            SaveState::Broken => Bson::String("Broken".to_string()),
            SaveState::Saved => Bson::String("Saved".to_string()),
        }
    }
}

impl From<RoleMetadata> for Bson {
    fn from(role: RoleMetadata) -> Bson {
        Bson::Document(doc! {
            "name": role.name,
            "code": role.code,
            "media": role.media,
        })
    }
}

impl From<PublishState> for Bson {
    fn from(state: PublishState) -> Bson {
        match state {
            PublishState::Private => Bson::String("Private".into()),
            PublishState::PendingApproval => Bson::String("PendingApproval".into()),
            PublishState::ApprovalDenied => Bson::String("ApprovalDenied".into()),
            PublishState::Public => Bson::String("Public".into()),
        }
    }
}

impl From<GroupId> for Bson {
    fn from(id: GroupId) -> Bson {
        Bson::String(id.as_str().to_owned())
    }
}

impl<T: crate::Validate> From<Name<T>> for Bson {
    fn from(name: Name<T>) -> Bson {
        Bson::String(name.to_string())
    }
}

impl From<InvitationState> for Bson {
    fn from(state: InvitationState) -> Bson {
        match state {
            InvitationState::Pending => Bson::String("Pending".to_owned()),
            InvitationState::Accepted => Bson::String("Accepted".to_owned()),
            InvitationState::Rejected => Bson::String("Rejected".to_owned()),
        }
    }
}

impl From<ProjectId> for Bson {
    fn from(id: ProjectId) -> Bson {
        Bson::String(id.0)
    }
}

impl From<oauth::ClientId> for Bson {
    fn from(id: oauth::ClientId) -> Bson {
        Bson::String(id.as_str().to_owned())
    }
}

impl From<oauth::Client> for Bson {
    fn from(client: oauth::Client) -> Bson {
        Bson::Document(doc! {
            "name": client.name,
            "id": client.id,
        })
    }
}

impl From<oauth::CodeId> for Bson {
    fn from(id: oauth::CodeId) -> Bson {
        Bson::String(id.as_str().to_owned())
    }
}

impl From<oauth::Code> for Bson {
    fn from(code: oauth::Code) -> Bson {
        Bson::Document(doc! {
            "id": code.id,
            "username": code.username,
            "clientId": code.client_id,
            "redirectUri": code.redirect_uri,
            "createdAt": DateTime::from_system_time(code.created_at),
        })
    }
}

impl From<oauth::TokenId> for Bson {
    fn from(id: oauth::TokenId) -> Bson {
        Bson::String(id.as_str().to_owned())
    }
}

impl From<ServiceHostScope> for Bson {
    fn from(scope: ServiceHostScope) -> Bson {
        match scope {
            ServiceHostScope::Public(cats) => Bson::Document(doc! {
                "public": Into::<Bson>::into(cats),
            }),
            ServiceHostScope::Private => Bson::String("private".into()),
        }
    }
}

impl From<MagicLinkId> for Bson {
    fn from(id: MagicLinkId) -> Bson {
        Bson::String(id.0)
    }
}

#[cfg(test)]
mod tests {
    use bson::to_bson;

    use super::*;

    #[test]
    fn test_deser_priv_service_host_scope() {
        let data = bson::to_bson(&ServiceHostScope::Private)
            .unwrap()
            .to_string();
        let priv_scope: Result<ServiceHostScope, _> = serde_json::from_str(&data);

        assert!(priv_scope.is_ok());

        let data = bson::to_bson(&ServiceHostScope::Public(vec!["hello".into()]))
            .unwrap()
            .to_string();
        let scope: Result<ServiceHostScope, _> = serde_json::from_str(&data);
        assert!(scope.is_ok());
    }
    #[test]
    fn test_bson_serialize_update_project_data() {
        let new_name = "new project";
        let name = ProjectName::new(new_name.to_string());

        // Serialize the struct to BSON
        let serialized = to_bson(&name).expect("Failed to serialize to BSON");
        println!("Serialized BSON: {:?}", serialized);

        // Deserialize the BSON back to the struct
        let deserialized: ProjectName =
            bson::from_bson(serialized).expect("Failed to deserialize from BSON");
        println!("Deserialized struct: {:?}", deserialized);

        // Check that the deserialized value matches the original
        assert_eq!(deserialized.as_str(), new_name);
    }
}
