# Publish all crates to crates.io
# Usage: make Publish
publish:
	cargo publish --manifest-path ./worker-sys/Cargo.toml
	cargo publish --manifest-path ./worker-macros/Cargo.toml
	cargo publish --manifest-path ./worker-build/Cargo.toml
	cargo publish --manifest-path ./worker/Cargo.toml
