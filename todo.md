# To Do
- [ ] add room update message support
    - event system
- [ ] add configuration file

- [ ] email Tom about the big update?

- [x] ban user support
    - [ ] add tests

- [ ] validate user accounts on creation
    - [-] maybe we will need to whitelist email domains later
    - we can block tor exit nodes. Should we record the IP address?

- [ ] ws support
    - [ ] sending (netsblox) messages
    - [ ] client-message
    - [ ] user-action
    - [ ] project-response
    - [ ] request-actions

- [ ] user routes
    - [ ] remove client-side password hashing?
        - [ ] test that the hashing algs are the same?
- [ ] network routes
    - [ ] message passing

- [ ] group routes
    - how are invalid object IDs handled?
        - 404? (hopefully)

- [ ] session doesn't ensure logged in...
     - new extractor that ensures authenticated?

- [ ] not implemented

- [ ] service hosts routes
- [ ] should I replace "collection" with specific names:
    - users, projects, etc?
    - [ ] do the types provide a projection?

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

- [ ] admin users
    - [ ] add tests

- [ ] external apps (using message passing)
    - how does this work with the friends?
    - how can we ensure no collisions?
    - [ ] add the group IDs (+ GLOBAL) to the clients in the network topology?
        - these would be the user's group + any owned groups
        - the sender and receiver must share at least one

## CLI
- add CLI
- write integration tests?
    - import the server and test against it?

## DONE
- [x] library approval endpoint
    - [x] add authentication

- [-] need to figure out the format used!!!
    - should I use xml instead of json?
        - probably not

- [x] authentication
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

    - oso seems promising...

    - Request is allowed if resource is:
        - owned by user (projects, libraries, groups, etc)
        - owned by member of one of my groups
        - is admin

        - are there any exceptions?

