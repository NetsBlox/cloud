# NetsBlox Cloud

This contains:

- a new implementation of the NetsBlox cloud server (ie, excluding services)
  with enhanced performance and security (among other things - for a complete
  list, check out `./crates/cloud/README.md`).
- a CLI for interacting with the cloud server
- a client for interacting with the cloud server (used by the CLI)
- migration crate for migrating data from the old JS NetsBlox server

## Versioning

To simplify determining compatibility, all crates share the same version number
using semantic versioning. This may change in the future to give the CLI and
server both a version number and an API version number (similar to docker). This
is already somewhat there but hasn't yet been made official. (I would expect an
autogenerated compatibility table and/or version numbers reported by the CLI
itself for the server, server API, client, and client API.)

## Quick Start

This has yet to be completed but we are planning to make this available as a
docker image. If you want to build from source, check out the development
section.

## Development

First, install stable rust ([rustup](https://rustup.rs/) recommended). Next,
start a local instance of [MongoDB](https://www.mongodb.com/) and
[Minio](https://min.io/). (I usually run them using docker but native is also
fine.) Minio configuration details can be found
[here](https://github.com/NetsBlox/cloud/blob/main/crates/cloud/config/default.toml#L20-L27).
Then run the server with

```
cd crates/cloud
RUN_MODE=local cargo run
```

To customize the deployment, check out the
[configuration file](./crates/cloud/config/default.toml).

Check out the [architecture](./architecture.md) document for an overview of the
codebase.
