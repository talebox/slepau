# Making slepau definitions

Code on chunk was getting too big. So I restructured things into smaller `slepau` (atoms).

This was the right step given the **explosion** of functionality 
since the project began and all the ideas that have come up since then.

Based on the idea of slepau (atoms). We've made a cargo virtual (because it has no binary of itself) workspace as a top level. And all slepau are inside the `slepau` folder.

---
# Get started

Our scripts can be run with [Nushell](https://nushell.sh/), which is multi-platform. So make sure it's installed.

Development is currently happening on [Arch Linux](https://wiki.archlinux.org/), so that's the only platform these steps have been tested at.

## Always bring scripts into scope `source scripts/source.nu` for each terminal.

The way this works requests reach Nginx, then get routed to either the static files (web apps), or the running rust services (backend).

So for things to work, <b>Nginx</b> has to be running, there have to be some <b>web static files</b> built, and the <b>rust service</b> you're using has to be running.

<b>Web Apps</b>

- Build and watch for changes: `run_web watch`

<b>Nginx</b>

- `run_nginx`

<b>Rust Services</b>

- `run_auth`
- `run_chunk`
- `run_media`


For more information, just read the scripts on `scripts` folder.

# To build:

## On Linux
- Get build dependencies `apt install ffmpeg nginx`
- (Optional, the script will add it automatically) Add this line to /etc/hosts `127.0.0.1 auth.local chunk.local media.local`
- Run `curl --proto '=https' --tlsv1.2 -sSf https://talebox.dev/standalone.sh | sh` which will download, extract, and run the standalone project for the first time. That's it!

## For arm
- On arch, install aur cross-compile toolchain `https://aur.archlinux.org/arm-none-linux-gnueabihf-toolchain-bin.git`
- Set docker to aur target context, then:
    - `build_server bin_armv7hf ["--target", "armv7-unknown-linux-gnueabihf", "-Zbuild-std"]`
	- `organize_out bin_armv7hf`
    - for each binrary name to deploy do: `deploy_docker <binary_name>`
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

For the svg icons. I've heavily relied on [Bootstrap Icons](https://icons.getbootstrap.com/). Credit goes to them for these.