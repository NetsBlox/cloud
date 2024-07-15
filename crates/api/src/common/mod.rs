pub use netsblox_api_common::CreateMagicLinkData;
pub use netsblox_api_common::{
    oauth, AppId, AuthorizedServiceHost, ServiceHost, ServiceHostScope, ServiceSettings,
};
pub use netsblox_api_common::{
    BannedAccount, Credentials, LinkedAccount, LoginRequest, NewUser, User, UserRole,
};
pub use netsblox_api_common::{
    ChangeGalleryData, CreateGalleryData, CreateGalleryProjectData, Gallery, GalleryId,
    GalleryProjectMetadata,
};
pub use netsblox_api_common::{
    ClientConfig, ClientId, ClientInfo, ClientState, ClientStateData, ExternalClient,
    ExternalClientState,
};
pub use netsblox_api_common::{
    CollaborationInvite, FriendInvite, FriendLinkState, InvitationId, InvitationResponse,
    InvitationState,
};
pub use netsblox_api_common::{CreateGroupData, Group, GroupId, UpdateGroupData};
pub use netsblox_api_common::{CreateLibraryData, LibraryMetadata};
pub use netsblox_api_common::{
    CreateProjectData, Project, ProjectId, ProjectMetadata, PublishState, RoleData, RoleId,
    RoomState, SaveState, UpdateProjectData, UpdateRoleData,
};

//
//
//

#[cfg(target_arch = "wasm32")]
pub mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
