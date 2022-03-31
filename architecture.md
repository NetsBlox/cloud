# Architecture
This document describes the high-level architecture of netsblox-cloud (and netsblox-cli) and any other contained crates.

NetsBlox-Cloud contains the core functionality of NetsBlox including:
- message routing (ie, virtual network overlay)
  - extensible to external clients, too
- room management and other social aspects of NetsBlox
  - occupant invitations
  - collaboration invitations
  - friends
- account/project/library management and other administrative functionality (class/group management, etc)
  - this includes banning accounts, password resets, blocking login from Tor, etc
- integration with service hosts (ie, servers that host NetsBlox services/RPCs)
  - authenticated services can send messages and query user info

NetsBlox-Cloud does _not_ contain:
- NetsBlox client code
- NetsBlox RPCs

The design was also informed by lessons learned from the previous NetsBlox server aims to:
- better facilitate (remote) management/admin
- provide stronger security

NetsBlox-CLI is a command line interface to netsblox-cloud which supports connecting to remote hosts and covers almost all functionality supported by the server. To give it a try, run `cargo run help` from `crates/cli`.

## Code Map
This section briefly outlines the different crates/directories and how they relate to one another:

### `crates/core`
This crate contains the common data types used by both netsblox-api and the netsblox-cloud crate and are used for communication between them.

### `crates/api`
This crate contains netsblox-api, a client library for using the netsblox-cloud API. It depends on netsblox-core for the data type definitions and re-exports them.

### `crates/cli`
This crate contains netsblox-cli, a command line interface to netsblox-api.

### `crates/cloud`
This crate contains the netsblox-cloud crate (ie, the netsblox server). `crates/cloud/src/models.rs` often contains structs with the same name as some defined in netsblox-core to add additional private information. For example, `User` contains additional fields for the salt and password hash.

