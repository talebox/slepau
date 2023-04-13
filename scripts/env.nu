#!/bin/nu

export def-env load_env [] {
	open "config/regex.toml" | flatten | rotate --ccw | rename name value | reduce -f {} {|it,acc| $acc | upsert $"REGEX_($it.name)" $it.value} | load-env
	$env.APP_VERSION = (open Cargo.toml | get workspace.package.version)
	$env.APP_BUILD_TIME = (date now | date format "%D %R")
	let-env CHUNK_PAGE_PATH = $"(pwd)/out/web/chunk/page.html"
}
export def-env load_env_prod [] {
	# Load all configs into build scope
	load_env
	open "config/prod.toml" | load-env
}
export def-env load_env_dev [] {
	load_env
	open "config/dev.toml" | load-env
}
