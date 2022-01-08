# To Do
- [ ] update mobile apps

- [ ] add room update message support
    - event system

- [ ] email Tom about the big update?

- [ ] update the services server connection (zmq)
    - add resolve endpoint?
        - (public role ID resolution)
        - Or the client could send this in the request...
            - context: {project: {name, id}, role: {name, id}}

- [ ] connect the client code and start testing things!
    - [x] refactor cloud
    - [x] create project:
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

    - [x] set client state not working...
        - how will this work with the new server?
            - network/<client_id>/state

    - [x] can I send a message to myself (unauthenticated)?
        - [x] resolve "everyone in room"/"others in room" on the client
        - [x] fix the async issue w/ actix...
            - Can I move the ref into the async block?
                - this fixed it!
        - [x] is the state being set?
            - checking...

        - [x] why is the address quoted?
            - json_serde (use as_str)

    - [x] remove client on disconnect
    - [ ] send room messages
        - [x] detect project rename
        - [x] role rename
        - [ ] add role
        - [ ] delete role
        - [ ] duplicate role
    - [ ] add the "latest" endpoints
        - project
        - role

    - [ ] delete transient projects after inactivity
        - if we disable creating roles without saving, this would be good
        - this isn't great since we wouldn't be able to try public projects...

        - inactivity should probably be determined by network activity?
            - when a client closes, we should delete all transient projects owned by the client ID (or username) after a set amount of time
            - same for logging out?

- [ ] update the compiler for resolving addresses
    - should be pretty easy to just copy the logic over

- [ ] validate user accounts on creation
    - [-] maybe we will need to whitelist email domains later
    - we can block tor exit nodes. Should we record the IP address?

- [ ] ws support
    - [x] sending (netsblox) messages
    - [ ] client-message
    - [ ] user-action
        - how should we handle collaboration?
    - [ ] project-response
    - [ ] request-actions

- [ ] user routes
    - [ ] remove client-side password hashing?
        - [ ] test that the hashing algs are the same?

- [ ] network routes
    - [ ] message passing

- [ ] add tests
    - [ ] group routes

- [ ] api docs with paperclip?

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
        - how can we have services servers register data for a user/group?

    - [x] external client support
        - when/where should I differentiate? I don't think a single network will be able to handle them all
        - maybe have a "app_networks" which can send messages?
            - NetsBlox

            - These would have two levels of hierarchy
            - The saved versions might be different (and might so we probably shouldn't 

        - Let's keep them separate so we can add optimizations to the netsblox one

        - what else might a client state need to include?
            - group IDs?
            - [/] probably only needs to take affect on login.
                - moved to different location


- Do I need "transient" projects on the server?
    - Can I handle name collisions some other way?

- [ ] add caching to the message sending?

- [ ] add benchmarks for message passing??

- [ ] can the updates to the network topology stuff replace the "transient" projects?
    - I don't think so since we will need to know the existing (unopened) projects so their name isn't changed on open (given them priority, that is)
        - what if 

- [ ] session doesn't ensure logged in...
     - new extractor that ensures authenticated?

- [ ] projects routes
    - [x] add blob support for main project data
        - rusoto?
    - [x] don't use hashing to store the data
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

- [x] admin users
    - [ ] add tests

- [ ] add the group IDs (+ GLOBAL) to the clients in the network topology?
    - these would be the user's group + any owned groups
    - the sender and receiver must share at least one

## CLI
- add CLI
- write integration tests?
    - import the server and test against it?

## DONE
- [-] add client ID to cookie...
    - this isn't good since cookies are shared across tabs

- [x] external apps (using message passing)
    - how does this work with the friends?
    - how can we ensure no collisions?
        - we can add #APP_ID afterwards

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

