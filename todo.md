# To Do
- [ ] ban user support
- [ ] validate user accounts on creation
    - maybe we will need to whitelist email domains later
- [ ] ws support
    - [ ] sending (netsblox) messages
    - [ ] client-message
    - [ ] user-action
    - [ ] project-response
    - [ ] request-actions

- [ ] user routes
- [ ] network routes
    - [ ] message passing

- [ ] projects routes
    - [ ] add blob support for main project data
    - [ ] don't use hashing to store the data
        - [ ] probably need a migration since this will change assumptions on delete/rename
    - [ ] get project by name (open default role?)
    - [ ] get project by name (entire project)

- [ ] auth integration with services endpoint
    - maybe the services endpoint should hit this one?

- [ ] require login to send messages?

- [ ] library approval endpoint
- [ ] admin users

## CLI
- add CLI
- write integration tests?
    - import the server and test against it?
