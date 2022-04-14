# To Do
- [ ] test...
    - [ ] recording messages
    - [x] message caching
    - [ ] created (but never occupied) projects - they should be automatically deleted after 15 minutes or so
 
- [ ] add aspect ratio padding support
    - use image crate

- [ ] add email support
    - [ ] new account creation
    - [x] password reset

    - [x] lettre crate?
        - smtp or ses?
        - mock the method now?

- [ ] collaborative editing action acceptance
    - maybe we don't need to persist them...
    - accept actions when collaborating
        - this is currently a perf bottleneck in the nodejs version
    - [ ] add a TTL for the latest action ID
    - [ ] need 2 collections:
        - project action IDs (w/ TTL)
        - project actions (w/ TTL)

    - [ ] how should I initialize the action index?
        - in the nodejs one, we set it when the project is opened
        - what if we just use the get_latest_project fn if there is no action index?
            - this might be slow to start but it should be fine, I think
        - should we add another ws message type?

    - [ ] check edit permissions

    - could I use client-message for this?
        - 
    - [ ] what would it look like w webrtc?
        - a communication channel
        - a library for collab primitives (key, type)
            - CRDT text
            - blocks (strong consistancy by scope)
            - project notes (CRDT or LWW)
            - LWW registers (rotation) w/ vector clocks

- [ ] update the services server connection (zmq)
    - [-] add public role ID resolution endpoint?
        - (public role ID resolution)
        - Or the client could send this in the request...
            - context: {project: {name, id}, role: {name, id}, app: ''}
        - we will need to be able to lookup the username and the context...
    - [ ] add a REST endpoint for message sending?
        - network/messages/send
            - recipient address
            - message type
            - message content
            - optional sender address
        - how to authorize?
            - app-level or user-level?

            - if sender address is provided, we could check the requestor can edit the user
            - else, we could use an app-level approach... Maybe something simple like a secret token for now?
                - technically, this is all we need for now

            - what if we connected the services server like a 3rd party app?
                - it would need to authenticate as a single user though
                    - NetsBloxServices?
                    - address could be 
                        - TicTacToe@NetsBlox #NetsBloxServices
                        - ProjectID@TicTacToe@NetsBlox #NetsBloxServices

                        - Services@NetsBlox #NetsBloxServices  // no response allowed
                    - this could actually make it possible to add responding to messages to the spec, too!
                        - the server would need to still be occupying those states though :/
                            - maybe we could route the message using the sender ID?
                                - wouldn't work since a project can use multiple services simultaneously

    - what would app-level look like?
        - admins adding a client 
        - client adds Access ID & Secret Token to send message requests
        - save these in the database
        - this wouldn't be in the config anymore

            - netsblox services add --authorize <client ID>
            - netsblox services list --global
            - netsblox services list --group
            - netsblox services list --only-user

            - netsblox service-host authorize <URL> <client ID> -> <secret token>
            - netsblox service-host unauthorize <URL>

        - these are actually different from the current services-hosts:
            - current ones are client-side configurations about endpoints to ping
            - new ones provide permissions to the service-host to be able to resolve client IDs and send messages

            - netsblox integrations add <name> ID -> <secret token>
            - netsblox integrations remove ID
            - netsblox integrations list

    - how is the API used by the services server?
        - authenticate users
            - (whoami endpoint)
        - send messages
        - CRD api keys (settings?)
            - this would make this part easier...

    - [ ] add client ID resolution endpoint
        - [x] authentication is at the app-level so this should be fine
        - [ ] client ID secret should also be included so it isn't spoofed...

    - [ ] integrate them!
        - [x] state endpoint
            - what should the state look like?
                - currently we already have ExternalClient
                - add BrowserClientState?
                    - option 1:
                        - username (optional)
                        - role_id
                        - project_id
                        - role_name
                        - project_name
                    - option 2:
                        - username (optional)
                        - state (optional)
                            - project_id
                            - role_id

                            - username
                            - address
                            - app_id

                - it would be nice to separate username, state
                - the state should probably be different...
                    - actually, we could just use the room state endpoint for some of these things and cache the value
                - should we rename state to location? or address? Maybe location since address
                - ClientInfo? ClientData

        - [x] room state endpoints
        - [ ] service settings endpoints
              - user, member, groups
              - [ ] should these be under a new route path? like /service-settings?
                  - or should they be under the users/groups paths?
                  - probably their own path since it would be nice to have a method for getting combined/all settings

                  - they could be nested under service hosts
                      - service_hosts/{id}/settings/

                  - [ ] CRUD options for users/groups/etc (based on a host)
                      - should the ID be simple alphanumeric (+underscores?)

                  - user/{username}/{service_host_ID} (get)
                  - user/{username}/{service_host_ID} (post)
                  - user/{username}/{service_host_ID} (delete)
                  - user/{username}/{service_host_ID}/all (get)

                  - group/{id}/{service_host_ID} (get)
                  - group/{id}/{service_host_ID} (post)
                  - group/{id}/{service_host_ID} (delete)

                - should we store these in their own collection?
                    - {host, settings, owner: {user: <username> | group: <groupID>}}
                    - this would make the queries pretty straight-forward...

                - services/hosts/
                - services/settings/user/

              - [ ] add operations to CLI?

        - [ ] send message endpoints
        
        - [ ] what to do about oauth?
            - should we support it in the rust server? Seems reasonable...
              - if so, how should we interoperate with the services server?
            - should we just support it in the services server?
              - 
            - skip this for now?

        - [x] update the client index.html?

- [ ] finish updating the browser

- [ ] public URL is set when opening role

- [ ] don't clean up projects when server goes down? (The ws close reason seems to be Away when the browser tab closes *and* when the server is terminated)
    - set all projects to BROKEN
    - can we differentiate btwn server initiated Away and client?

- [ ] ws support
    - [x] sending (netsblox) messages
    - [ ] client-message
        - refactor a lot of things to use this...
    - [ ] user-action
        - how should we handle collaboration?
    - [x] project-response
    - [ ] request-actions

- [ ] Block messages between users that don't share a group (+admin)
    - add the group IDs (+ GLOBAL) to the clients in the network topology?
    - admins should be able to send a message to anyone
    - these would be the user's group + any owned groups
    - the sender and receiver must share at least one

- [ ] make sure email works

## Future stuff
- [ ] generic library for collaborative editing (different CRDTs, etc)
    - use the concept of streams/pipes?

- [ ] connect the client code and start testing things!

- [ ] add benchmarks for message passing??

- [ ] logout on ban? Or just ensure not banned

- [ ] allow moderators to bypass profanity checker?

- [ ] require login to send messages?

- [ ] better pwd reset process (send link instead)
    - IP-based rate limiting...

- [ ] store additional info in the cookie? (optimize lookups)
    - groups (for networking things)?
    - admin?

## Related project updates/migrations
- [ ] unban?

- [ ] project \_id -> id
    - migrate the data

- [ ] update the compiler for resolving addresses
    - should be pretty easy to just copy the logic over

- [ ] ensure no usernames have @ symbol in them

- [ ] update mobile apps

- [ ] email Tom about the big update?

- [ ] gallery

- [ ] api docs with paperclip?

- [ ] make usernames case-insensitive for routes?
    - it's probably fine the way it currently is

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

- [x] test ban account
    - [x] email should be banned, too
    - [x] no new create

- [x] allow login with linked accounts
    - working on this...
    - [x] need to be able to retrieve the email address

- [x] add index to projects collection for "id"

- [x] switch admin flag to role: ["admin", "moderator"]
    - moderators can approve libraries, etc
    - should this be called "type" instead?

- [x] add evict
    - [-] should it require the project ID?
        - it's probably fine
    - we should probably have a method to get the client's current state
        - (awk to pass project ID to CLI)
    - should we be able to evict clients from other apps, too?
        - probably
        - these ones may not have a project associated...

    - we should probably be able to evict ourselves (regardless of permissions)
        - in other words, a client can be evicted:
            - by project owner
            - by project collaborators
            - by anyone who can edit the given user

    - we may want to change the endpoint...
        - maybe /network/clients/{clientID}/evict

    - the rough flow would be:
        - get the state for the client
        - check permissions
            - if browser client, project owner, collaborator, or user editor can evict
            - if external client, user editor can evict
        - evict
            - how to handle this since it is async?

    - [x] test evict

- [x] block tor exit nodes

- [-] validate user accounts on creation
    - [-] maybe we will need to whitelist email domains later
    - we can block tor exit nodes. Should we record the IP address?

- [x] add message on evict

- [x] online friends (admin returns all)

- [x] projects routes
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

- general
    - [x] finalize output formats (machine vs human?)
        - what kind of errors should we have?
            - base it off the status codes (+reqwest errors)?
        - BadRequestError
        - UnauthorizedError
        - NotFoundError
        - InternalServerError
        - RequestError

- [x] fix login required errors (to unauthorized)

- [-] session doesn't ensure logged in...
     - new extractor that ensures authenticated?

- [x] make usernames case-insensitive
    - for all the routes, too?
    - this can be taken care of elsewhere or later

- [-] add tests
    - [ ] group routes

- [x] username length

- [/] record messages
    - this can follow the same method as before
        - add a TTL of something like 1 day to messages?
    - recordings can be their own collection. The ttl means we won't need to keep it in sync with the projects
        - security considerations
            - we can still check that the user can view the project
        - if the project is recording messages can be stored with the project itself
                - this avoids any issues
        - recorded messages can have a ttl

        - start
            - (post) network/id/{id}/trace/
            - return trace ID
        - get
            - (get) network/id/{id}/trace/{traceId}
            - return messages
        - stop
            - (delete) network/trace/id/:id
                - or (post) network/trace/id/:id/stop
        - delete?
                
        - check if recording network traces w/ a cache
            - where should it be cached? In app data?

    - [ ] record messages on send, if recording
    - [ ] should we take a decentralized approach and have the clients report?
        - then the clients would need to share with each other that a trace is being collected
            - what if the client joins during a trace?
                - then the new client needs to be updated by one of the recording node
        - if someone is recording the network and a message is sent or received, it would need to be added to the active traces

        - MessageTrace struct with send/received enum
        - This could make the animation look better...
        - routes could be:
            - (broadcast client message to room start)
            - save a message event
                - (post) network/id/{project_id}/trace
            - retrieve messages
                - (get) network/id/{project_id}/trace?startTime=1234&endTime=1345
            - messages will delete on their own (expire in a few hours or something)...
            - get current time endpoint
                - /time

        - advantages of clients recording the messages:
            - no need to query the database on each message and determine if it needs to be recorded
            - easier transition to decentralized messagig like webrtc

    - game plan:
        - [ ] implement a (slow but functional) version w/o caching
            - [ ] test it!
        - [ ] add project metadata caching
            - can be used for collaborators and other metadata, too
            - should I change project_metadata?
                - project_metadata.collection
                - project_metadata.get(id)
                - project_metadata.get_many(ids)
            - or should I do something like:
                - app.get_project_metadata(id)
                - app.get_project_metadata(id)  // with a batch option
            - cache will need to be made Arc
                - the whole app will need to be passed to the network topology then :/
                    - I guess this is ok...

            - [x] let's make the cache lazy static?
        - [x] invalidate the cache when...
            - [x] role is renamed
            - [x] project is renamed
            - [x] collaborator is added
            - [x] collaborator is removed
            - [x] network trace is started
            - [x] network trace is stopped

- [x] change ensure_can_edit_project to ID?

- [x] add address caching to the message sending?
    - [x] update cache on "send room state"

- [x] only allow one user with the given email address

- [x] add unvisited saveState (w/ a ttl)
    - CREATED -> TRANSIENT -> BROKEN/SAVED
    - [ ] test this

- [x] allow disabled tor IPs
    - add to config

- [x] occupants
    - [ ] invite occupant
        - these can probably be transient invitations
        - [x] maybe persist in mongo with a short ttl (a few minutes or something)
        - [ ] should it send the invite via ws?
            - probably wouldn't be bad...
    - [-] respond-to-invite
        - this probably doesn't make sense from the CLI

    - can we think of these as access grants instead?
        - server can provide minimal CRUD features
        - client can send invite over ws itself with project, role, etc

        - remaining questions:
            - revoking access grant should boot existing users
                - or should it be a separate call? The problem is that a grant is for a username while an eviction is for a client ID...
            - let's make evict a separate call
            - maybe we should return to the idea of invitations...
    - invite occupant from CLI?
    - [x] test this from the browser
        - [x] can send invite
        - [-] accepting invite removes invite from database
        - [-] close additional invites when one is accepted
              - this is a little annoying. Will I actually need to ID invites?
        - [x] allow user to open project using invite

- [-] connect the client code and start testing things!
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
- [x] apiKeys. Should these be managed from the services server?
    - probably
    - how can we have services servers register data for a user/group?

    - these can be associated with groups or users...
        - how can we delete these when the user/group is deleted?
    - what if I just had a "serviceSettings" dictionary?
        - the dict would look like:
            {
                "https://editor.netsblox.org/services": {apiKeys}
                "https://myOtherServices.com/": {apiKeys}
            }

    - [x] add settings for groups, too
    - [-] should we make the service settings public?
    - [ ] add endpoints for it?

- [x] auth integration with services endpoint
    - maybe the services endpoint should hit this one?

