# To Do
- [ ] switch admin flag to role: ["admin", "moderator"]

- [ ] allow login with linked accounts
    - working on this...
    - [x] need to be able to retrieve the email address

- [ ] apiKeys. Should these be managed from the services server?
    - probably
    - how can we have services servers register data for a user/group?

    - these can be associated with groups or users...

- [ ] collaborative editing action acceptance
    - maybe we don't need to persist them...

- [ ] update the services server connection (zmq)
    - add resolve endpoint?
        - (public role ID resolution)
        - Or the client could send this in the request...
            - context: {project: {name, id}, role: {name, id}, app: ''}
    - add a REST endpoint for this?

- [ ] online friends (admin returns all)

- [ ] occupants
    - [ ] invite occupant
        - these can probably be transient invitations
        - maybe persist in mongo with a short ttl (a few minutes or something)
    - [ ] respond-to-invite
        - this probably doesn't make sense from the CLI

- [ ] test ban account
    - [ ] email should be banned, too
    - [ ] no new create

- general
    - [ ] finalize output formats (machine vs human?)
    - [ ] finish updating the browser

- [ ] add index to projects collection for "id"

- [ ] add unvisited saveState (w/ a ttl)
    - CREATED -> TRANSIENT -> BROKEN/SAVED

- [ ] public URL is set when opening role
- [ ] connect the client code and start testing things!
    - [x] send room messages
        - [x] detect project rename
        - [x] role rename
        - [x] add role
            - [ ] is this a little slow?
        - [x] delete role
        - [x] duplicate role
            - [x] needs latest role endpoint
            - [x] getting a 404 error
            - [x] getting a 404 error for createRole

    - [x] add the "latest" endpoints
        - project
        - role
            - how can I perform req-reply over the ws connection?
                - add a "mailbox" for the responses
                - send then async sleep until a response is received

            - How can I get an async result from ctx.spawn(fut)?
                - Can I get it from the result?

                - Can I just add a method to get a copy of the client(s) at a role?
                    - then I could handle the async stuff on my end
                    - we also shouldn't need a hashmap of requests, either

                    - There might be a better abstraction rather than copying the client
                        - maybe a ClientChannel?
                            - channel.send().await?
                        - maybe a RoleDataRequest?
                            - request.send().await?

                    - [x] we need some shared memory to write the response into...
                        - make a shared response buffer (maybe a queue?)

                - Should the response be over ws or http?
                    - http would have access to cookies...
                    - what is the benefit to using ws?
                        - maybe slightly more efficient?

    - [ ] delete transient projects after inactivity
        - if we disable creating roles without saving, this would be good
        - this isn't great since we wouldn't be able to try public projects...

        - inactivity should probably be determined by network activity?
            - when a client closes, we should delete all transient projects owned by the client ID (or username) after a set amount of time
            - same for logging out?

        - [ ] set projects as "broken" on broken ws connections
        - [ ] test this!
            - make sure the broken project is not deleted once another client reconnects

- [ ] don't clean up projects when server goes down?
    - set all projects to BROKEN
    - can we differentiate btwn server initiated Away and client?


- [ ] validate user accounts on creation
    - [-] maybe we will need to whitelist email domains later
    - we can block tor exit nodes. Should we record the IP address?

- [ ] ws support
    - [x] sending (netsblox) messages
    - [ ] client-message
        - refactor a lot of things to use this...
    - [ ] user-action
        - how should we handle collaboration?
    - [x] project-response
    - [ ] request-actions

- [ ] add tests
    - [ ] group routes

- [ ] api docs with paperclip?

- [ ] add address caching to the message sending?
    - [ ] update cache on "send room state"

- [ ] add benchmarks for message passing??

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

- [ ] store additional info in the cookie:
    - groups (for networking things)?
    - admin?

- [ ] add the group IDs (+ GLOBAL) to the clients in the network topology?
    - these would be the user's group + any owned groups
    - the sender and receiver must share at least one

## Related project updates/migrations
- [ ] unban?

- [ ] project \_id -> id

- [ ] update the compiler for resolving addresses
    - should be pretty easy to just copy the logic over

- [ ] ensure no usernames have @ symbol in them

- [ ] update mobile apps

- [ ] email Tom about the big update?

- [ ] gallery

## CLI
- [x] add CLI
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

- [x] refactor cloud
- [x] create project:
    - [x] new project
        - blob connection test (minio)
    - [ ] save projects
        - [x] need to fix the cookie problem first...
    - [x] list projects
        - [x] ObjectId is serializing very strangely ($oid)
            - [x] changing project Id...
    - [ ] cookie on initial load does not seem to be present
    - [x] login
    - [x] signup
    - [x] cookie is blank
        - it seems to be set correctly... Maybe this is an issue with fetch?

    - [/] the cookie still doesn't seem to persist...
        - maybe if I set the expires_in value?
            - this doesn't seem to be working...
        - I think this is actually an issue on the client side...
            - [x] set same-site...

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

- [-] add room update message support
    - event system
    - just notify the network from the app

- [x] rename projects to avoid collisions
    - I don't think this is currently working...
    - it looks fine - except for the duplicate key error

- [x] show usernames in room state messages

- [x] move the public types to an api crate or something?

- rename to:
    - netsblox-cloud
    - netsblox-api/client?
    - netsblox-cli

- [x] refactor errors...
    - define a user error for something specific - maybe a database error?

- [x] libraries
     - [x] list --community --approval-needed
     - [x] delete
     - [x] publish
     - [x] unpublish
     - [x] approve

     - [x] import?
        - save?

- [x] add password salts

- [x] groups
    - [x] view group
    - [x] list groups 
    - [x] create groups 
    - [x] delete group
    - [x] rename group
    - [x] list members group

- [x] services hosts
    - [x] list --user-only --group --user
        - [x] need the group-only option...
    - [x] add --group --user
        - user is overloaded now...
        - I think it is fine
    - [x] remove --group --user

- [x] projects
    - [x] list  --shared
    - [x] export
    - [x] publish
    - [x] unpublish
    - [x] delete
    - [x] rename

    - [x] invite collaborator
    - [x] list invites
    - [x] respond to invite
    - [x] list collabs
    - [x] rm collab

- [x] friends
    - [x] list friends
    - [x] list invites
    - [x] send invite
    - [x] respond to invite
    - [x] block user (unblock?)
    - [x] unfriend
    - [x] list (online) friends
        - include the app they are using?
        - client IDs (if netsblox)
        - addresses?
            
            {username, contexts: {clientId, app?}}

        - just usernames for now...

- [x] export latest
    - make a CLI to test some of this, too?
    - list networks?
    - send message?
        - should we be able to receive messages? Maybe send message and wait?
            - #NetsBloxCLI

    - [x] create user?
        - maybe only group members?
        - how could we prevent malicious use?
            - only group members is probably fine
    - delete members?

- [-] update collaboration?
    - no more recording the latest one on the server?
    - maybe leave it as is for now?
    - moved to separate bullet point
        
- [x] user routes
    - [x] remove client-side password hashing?
        - [ ] test that the hashing algs are the same?

- [x] Do I need "transient" projects on the server?
    - Can I handle name collisions some other way?

- [x] change "project_name" to "name"

- [-] can the updates to the network topology stuff replace the "transient" projects?
    - I don't think so since we will need to know the existing (unopened) projects so their name isn't changed on open (given them priority, that is)
        - what if 

- [x] routes
    - [x] collaboration invites
        - keep the invites (only can send one per person/project)
    - [x] friends
        - store these as a list of usernames?
            - what if the friend deletes his/her account?
        - store these in an "edges" collection?
            - easier to update on deletion
            - this is probably the better way to go
    - [x] friend requests?
        - should we have "block"/decline
        - should reject auto block?
            - probably not
    - [x] service hosts
    - [x] projects
        - project_metadata
            - maybe add a method for only the public fields?
        - projects
            - get the project source from the blob
        - roles?
        - [x] add blob support

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

- [x] admin users
    - [ ] add tests

- [x] network
    - [x] list
      - should this just list the networks for a given user?
      - since this will be an admin endpoint to start anyway, it would probably be good to just keep it simple - we can extend it later
        - it should probably just list the browser networks (or external)

        - what about?
                
            network list -> <project IDs>
            network list --external -> (address, username, app)[]

    - [x] view <project> -> RoomState

            skip for external

    - [x] connect
        - mostly works for now. Probably fine

    - [x] invalid response unknown variant mongodb

- [x] network routes
    - [x] message passing

- [x] users
    - [x] create
    - [x] list
    - [x] set-password
    - [x] list
    - [x] delete
    - [x] view
    - [x] link
    - [x] unlink

