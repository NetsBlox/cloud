use crate::{
    oauth, FriendInvite, FriendLinkState, Group, InvitationState, LinkedAccount, ProjectId,
    PublishState, RoleMetadata, SaveState, ServiceHost, UserRole,
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
            FriendLinkState::PENDING => Bson::String("PENDING".into()),
            FriendLinkState::APPROVED => Bson::String("APPROVED".into()),
            FriendLinkState::REJECTED => Bson::String("REJECTED".into()),
            FriendLinkState::DELETED => Bson::String("DELETED".into()),
            FriendLinkState::BLOCKED => Bson::String("BLOCKED".into()),
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
            SaveState::CREATED => Bson::String("CREATED".to_string()),
            SaveState::TRANSIENT => Bson::String("TRANSIENT".to_string()),
            SaveState::BROKEN => Bson::String("BROKEN".to_string()),
            SaveState::SAVED => Bson::String("SAVED".to_string()),
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

impl From<Group> for Bson {
    fn from(group: Group) -> Bson {
        Bson::Document(doc! {
            "id": group.id,
            "name": group.name,
            "owner": group.owner,
            "servicesHosts": group.services_hosts,
        })
    }
}

impl From<InvitationState> for Bson {
    fn from(state: InvitationState) -> Bson {
        match state {
            InvitationState::PENDING => Bson::String("PENDING".to_owned()),
            InvitationState::ACCEPTED => Bson::String("ACCEPTED".to_owned()),
            InvitationState::REJECTED => Bson::String("REJECTED".to_owned()),
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
