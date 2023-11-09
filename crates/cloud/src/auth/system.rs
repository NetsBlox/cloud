use crate::network::topology::network;

/// This is used to get "system" permissions used when the network topology needs
/// to perform actions. It is not intended to be used directly but, rather, to be
/// able to be converted to other concrete authorizations

pub(crate) struct ManageSystem {
    _private: (),
}

/// Get permissions to manage the system. Only the network topology can obtain these
/// permissions.
pub(crate) fn try_manage_system(_network: &network::Topology) -> ManageSystem {
    ManageSystem { _private: () }
}
