# Making slepau definitions

A project's code and it's behaviour are inextricably linked. 

The aim of this reordering is putting things in the right places 
and hopefully set myself up for better encapsulation.

This was the right step given the **explosion** of functionality 
since the project began and all the ideas that have come up since then.

Based on the idea of slepau (atoms). We've made a cargo virtual (because it has no binary of itself) workspace as a top level. And all slepau are inside the `slepau` folder.

---
# Folder overview

## Config ⚙

Where all environment variables are set for running slepau on development/production.

It also holds project regex. So they can be shared between rust and javascript.

## Scripts 

Nushell scripts to automate run/stop/deploy actions.

## Tmp 

Slepau temporary savefiles/cache when running locally for debugging/testing.

## Out

