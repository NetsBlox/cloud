# Architecture
This document describes the high-level architecture of netsblox-cloud and the general code structure.

At a high level, netsblox-cloud is an API server for the core functions of NetsBlox such as account management and message routing. It does not provide any services but does provide mechanisms for integrating with other NetsBlox RPC/service providers (ie, service hosts).

The code layout is basically a bunch of files which correspond to endpoints managing some resource. For example, friend management is found in `friends/` and `projects/` contains code for endpoints used for project management. Each directory contains 3 files: `routes.rs`, `actions.rs`, and `mod.rs`. These contain the actual endpoint definitions, the implementations for the core logic (using the witness pattern to give compile-time checks for access control), and a small file to expose the necessarily functions as a rust module, respectively.
- `actions`
There are a few exceptions:
- `app_data/` contains the main application data shared between requests (and across threads)
- `errors.rs` defines the error types used by the server
- `test_utils.rs` helpers for writing tests
- `config.rs` defines the configuration file for the server
- `main.rs` defines the entrypoint for running the application (like all rust binaries)
