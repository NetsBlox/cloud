use crate::FriendLinkState;
use bson::Bson;

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
