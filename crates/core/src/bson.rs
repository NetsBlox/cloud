use crate::{FriendInvite, FriendLinkState};
use bson::{doc, Bson, DateTime};

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
