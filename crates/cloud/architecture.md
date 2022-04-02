# Architecture
This document describes the high-level architecture of netsblox-cloud and the general code structure.

At a high level, netsblox-cloud is an API server for the core functions of NetsBlox such as account management and message routing. It does not provide any services but does provide mechanisms for integrating with other NetsBlox RPC/service providers (ie, service hosts).

The code layout is basically a bunch of files which correspond to endpoints managing some resource. For example, friend management is found in `friends.rs` and `project.rs` contains code for endpoints used for project management. If the code for a given set of endpoints can be refactored into multiple files for a better separation of concerns, then a directory is used instead where the routes are the main `mod.rs` file (eg, `users/`). There are a few exceptions:
- `app_data/` contains the main application data shared between requests (and across threads)
- `models.rs` defines custom types used across multiple files (often app_data and the corresponding route definitions)
- `errors.rs` defines the error types used by the server
- `config.rs` defines the configuration file for the server
- `main.rs` defines the entrypoint for running the application (like all rust binaries)
