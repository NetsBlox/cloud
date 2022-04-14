# NetsBlox Cloud
This contains a new implementation of the NetsBlox core server (ie, excluding services) with enhanced performance and security (among other things - for a complete list, check out `./crates/cloud/README.md`).

## Quick Start
This has yet to be completed but we are planning to make this available as a docker image. If you want to build from source, check out the development section.

## Development
First, install stable rust ([rustup](https://rustup.rs/) recommended). Then run with
```
cd crates/cloud
cargo run
```
To customize the deployment, check out the [configuration file](./crates/cloud/config/default.toml).

Check out the [architecture](./architecture.md) document for an overview of the codebase.
