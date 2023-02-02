#!/bin/nu

export def-env load_env [] {
	open "config/regex.toml" | flatten | rotate --ccw | rename name value | reduce -f {} {|it,acc| $acc | upsert $"REGEX_($it.name)" $it.value} | load-env
	$env.APP_VERSION = (open Cargo.toml | get workspace.package.version)
}

# def main [] {
# 	env_load "config/regex.toml"
# }
