# NetsBlox API
This project is an experiment exploring implementing a new version of the "NetsBlox cloud" in Rust.

## Motivation
1. The current API server has significant cruft as it was initially an attempt to reverse engineer the Snap server. They have rewritten theirs but we still have the legacy code from the original one..
2. There are actually a decent number of security issues that should be fixed. The recent malicious user makes me a little cautious about this as it wouldn't be hard to try to cause problems...
3. The current API is not easily testable. This would be nice to improve.
4. There are some additional features that would be nice to have such as friends (important given the recent malicious user) and persistent collaboration invitations (less important).
4. On that note, adding better support for external applications (such as pyblox) would be good to have. This could also help decouple some of the client and server code so NetsBlox can provide generic message routing capabilities.
5. Using a strongly typed language would be nice to prevent bugs. There have been a number of bugs that wouldn't have happened if the language was compiled.
6. Scalability. This should have a significantly smaller memory footprint and I expect it to scale better with respect to message passing. Ideally, this will be migrated to webRTC when possible but this is for the future.
7. Unlike the NetsBlox services server, this server has relatively low churn and not many contributors. This gives us more flexibility for implementing it in something focusing more on performance rather than simply ease of contribution.
8. Given the first few points, I had already been migrating to a new API within the NetsBlox server (see `src/server/api`). However, I realized that I might as well work on addressing all the points (and hopefully making it more scalable) and make a bigger refactor rather than these tiny refactors.
