# Making slepau definitions

A project's code and it's behaviour are inextricably linked. 

The aim of this reordering is putting things in the right places 
and hopefully set myself up for better encapsulation.

This was the right step given the **explosion** of functionality 
since the project began and all the ideas that have come up since then.

Based on the idea of slepau (atoms). We've made a cargo virtual (because it has no binary of itself) workspace as a top level. And all slepau are inside the `slepau` folder.

---
# Folder overview

## Slepau 🔩

Slepau rust projects.

## Web 🪟

Slepau web projects.

## Config ⚙

Where all environment variables are set for running slepau on development/production.

It also holds project regex. So they can be shared between rust and javascript.

## Scripts 🗒

Nushell scripts to automate run/stop/deploy actions.

## Tmp 🧪

Slepau temporary savefiles/cache when running locally for debugging/testing.

## Out 📦

All production files generated after `build` is executed.

## Container ⚓

Dockerfiles.

---
# Get started

Our scripts can be run with [Nushell](https://nushell.sh/), which is multi-platform. So make sure it's installed.

Development is currently happening on [Arch Linux](https://wiki.archlinux.org/), so that's the only platform these steps have been tested at.

## Always bring scripts into scope `source scripts/source.nu` for each terminal.

- To build web projects and watch for changes: `cd web; yarn watch`
- Run auth like so: `run_auth`
- Run chunk like so: `run_chunk`
- Run media like so: `run_chunk`

For more information just read scripts on `scripts` folder.

# To build:

## Ubuntu
- Get build dependencies `apt install openssl libssl-dev pkg-config ffmpeg gcc`
- Install rust environment `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- Nuhsell `cargo install nu`
- On nushell `source scripts/source.nu; build_all`
- Built files will be on the `out` directory.