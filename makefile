watch_print:
	cat /tmp/test-struct-derive.rs | rustfmt --edition 2021 | bat --color=always -l rust
