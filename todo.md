# To Do
- [ ] update mobile apps

- [ ] add room update message support
    - event system

- [ ] email Tom about the big update?

- [ ] connect the client code and start testing things!
    - [x] refactor cloud
    - [ ] create project:
        - [x] new project
            - blob connection test (minio)
        - [ ] save projects
        - [x] login
        - [x] signup
        - [x] cookie is blank
            - it seems to be set correctly... Maybe this is an issue with fetch?

        - [ ] the cookie still doesn't seem to persist...
            - maybe if I set the expires_in value?
                - this doesn't seem to be working...
            - I think this is actually an issue on the client side...

    - [ ] set client state not working...
        - how will this work with the new server?
            - network/<client_id>/state

    - [ ] can I send a message to myself (unauthenticated)?
        - [x] resolve "everyone in room"/"others in room" on the client
        - [ ] fix the async issue w/ actix...
            - Can I move the ref into the async block?

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

- [ ] add tests
    - [ ] group routes

- [ ] routes
    - [x] collaboration invites
        - keep the invites (only can send one per person/project)
    - [ ] friends
        - store these as a list of usernames?
            - what if the friend deletes his/her account?
        - store these in an "edges" collection?
            - easier to update on deletion
            - this is probably the better way to go
    - [ ] friend requests?
        - should we have "block"/decline
    - [x] service hosts
    - [ ] projects
        - project_metadata
            - maybe add a method for only the public fields?
        - projects
            - get the project source from the blob
        - roles?
        - [x] add blob support

    - [ ] apiKeys. Should these be managed from the services server?
        - probably


- Do I need "transient" projects on the server?
    - Can I handle name collisions some other way?

- [ ] can the updates to the network topology stuff replace the "transient" projects?
    - I don't think so since we will need to know the existing (unopened) projects so their name isn't changed on open (given them priority, that is)
        - what if 

- [ ] session doesn't ensure logged in...
     - new extractor that ensures authenticated?

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

- [ ] add client ID to cookie...

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

- [x] group routes
    - how are invalid object IDs handled?
        - 404? (hopefully)

- [x] ban user support
    - [x] add tests

- [-] not implemented

- [x] should I replace "collection" with specific names:
    - users, projects, etc?
    - [ ] do the types provide a projection?

- [x] where do they get the services hosts, client ID, etc?
    - move away from server side code generation
    - configuration should be:

- [x] add configuration file

