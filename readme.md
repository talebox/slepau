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

As well as nginx (an http reverse proxy server) configuration files.

## Scripts 🗒

Nushell scripts to automate run/stop/deploy actions.

## Tmp 🧪

Slepau temporary savefiles/cache when running locally for debugging/testing.

## Out 📦

All production files generated after `build_all` is executed.

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
- Run media like so: `run_media`

For more information just read scripts on `scripts` folder.

# To build:

## On Linux
- Get build dependencies `apt install ffmpeg nginx`
- Add this line to /etc/hosts `127.0.0.1 auth.local chunk.local media.local`
- Run `curl --proto '=https' --tlsv1.2 -sSf https://talebox.anty.dev/standalone.sh | sh` which will download, extract, and run the standalone project.