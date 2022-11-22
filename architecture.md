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

### `crates/api-common`
This crate contains the data types used by the public API (communication btwn netsblox-api and the netsblox-cloud crate).

### `crates/cloud-common`
This crate contains the data types used by the cloud (and stored in the database). Often contains structs with the same name as some defined in netsblox-api-types to add additional private information. For example, `User` contains additional fields for the salt and password hash.

### `crates/api`
This crate contains netsblox-api, a client library for using the netsblox-cloud API. It depends on netsblox-api-types for the data type definitions and re-exports them.

### `crates/cli`
This crate contains netsblox-cli, a command line interface to netsblox-api.

### `crates/cloud`
This crate contains the netsblox-cloud crate (ie, the netsblox server).

### `crates/migrate-v1`
This crate contains a program for migrating from NetsBlox v1.50.0 (in JS) to this NetsBlox cloud.

