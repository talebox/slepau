#!/bin/nu
export def status [] {
	echo "Status -------"
	let p = (ps --long | find "cargo");echo $"cargo ($p | length) active";echo $p
	let p = (ps --long | find "yarn parcel");echo $"yarn parcel ($p | length) active";echo $p
	let p = (ps | find "chunk-app");echo $"chunk-app ($p | length) active";echo $p
}

