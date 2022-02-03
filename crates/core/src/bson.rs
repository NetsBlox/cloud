use crate::{FriendInvite, FriendLinkState, LinkedAccount, RoleMetadata, SaveState, ServiceHost};
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
            SaveState::TRANSIENT => Bson::String("TRANSIENT".to_string()),
            SaveState::BROKEN => Bson::String("BROKEN".to_string()),
            SaveState::SAVED => Bson::String("SAVED".to_string()),
        }
    }
}

impl From<RoleMetadata> for Bson {
    fn from(role: RoleMetadata) -> Bson {
        Bson::Document(doc! {
            "ProjectName": role.name,
            "SourceCode": role.code,
            "Media": role.media,
        })
    }
}
