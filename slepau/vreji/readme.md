What's a log if not a long list of lines.

Right?

Well, really, I've been looking at a few time-series db's:
- [ReducDB](https://lib.rs/crates/reductstore)
- [sonnerie](https://lib.rs/crates/sonnerie) **<**

The hardest part about this is compression, aggregation, and cache.

But I won't be too picky since this is really my first attempt. So something like sonnerie would work beatifully, especially if we can use it as a library and bundle it within our log executable.

Soo, what do we need?

We need two things, to POST new actions/data, and GET data.

But, before we get ahead of ourselves defining things, what actions/events do I want to save?
- Auth
    - `auth_get_user <user> <ip_v4>`
    - `auth_login <user> <ip_v4>`
    - `auth_login_error <user> <ip_v4>`
    - `auth_register <user> <ip_v4>`
    - `auth_register_error <user> <ip_v4>`
    - ... almost all actions
- Chunk 
    - `chunk_new <chunk_id_u32> <user>`
    - `chunk_edit <chunk_id_u32> <user> <delta_change_string>`
    - 
- Media
    - `media_post <media_id_u64> <user>`
    - `media_get <media_id_u64> <user>`
- Service health checks, like a status for each service `<app>_status`
    - Auth would have (#sites, #users)
    - Media size (#total items, #total size)
    - Chunk (#total chunks, #total edits, #total shared, #page views)

All entries have format `<app>_<action> <t u64> `.

Now, what about the UI, how do we want that to look?

UI is all about showing relevant info.
- How many edits are users making daily. And who's making those? (Chunk)
- How many ips are connected, and, how many users are active? (Auth)
- How many chunks/media does each user have. (Chunk/Media)
- How many pages are being viewed, by what ips/locations the most. (Chunk + IP Location)