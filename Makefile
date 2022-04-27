.PHONY: install
install:
	CARGO_INCREMENTAL=0 \
	CARGO_PROFILE_RELEASE_LTO=thin \
	RUSTFLAGS="-C force-frame-pointers=yes" \
	cargo install --path .
