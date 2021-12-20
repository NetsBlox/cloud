# To Do
- [ ] ban user support
    - [ ] add tests
- [ ] validate user accounts on creation
    - maybe we will need to whitelist email domains later
    - we can block tor exit nodes. Should we record the IP address?
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
        - rusoto?
    - [ ] don't use hashing to store the data
        - [ ] probably need a migration since this will change assumptions on delete/rename
    - [ ] get project by name (open default role?)
    - [ ] get project by name (entire project)
    - [ ] list projects
        - projects/id/{ID}
        - projects/user/{owner}
        - projects/shared/{collaborator}

- [ ] auth integration with services endpoint
    - maybe the services endpoint should hit this one?

- [ ] require login to send messages?

- [ ] library approval endpoint
    - [ ] add authentication

- [ ] authentication
    - two main forms:
        - admin (should be easy with an extractor)
        - group owner (a little trickier since it depends on the group...)
    - casbin-rs seems promising. We would just need to define the policies for the database...
        - ACL, RBAC, ABAC?
        - maybe role-based access control?
        - Actually ABAC might be the easiest in terms of integration
            - user cookie can include:
                - group IDs? Wait, that isn't included...

        - It would be nice not to have two sources of truth...

- [ ] admin users

## CLI
- add CLI
- write integration tests?
    - import the server and test against it?
