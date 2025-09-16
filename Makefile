.PHONY: build
build: wasi_snapshot_preview1.proxy.wasm
	cargo build --release --target wasm32-wasip1 $(BUILD_ARGS)
	wasm-tools component new --skip-validation target/wasm32-wasip1/release/hello_wasip3_http.wasm --adapt wasi_snapshot_preview1.proxy.wasm -o hello.wasm

wasi_snapshot_preview1.proxy.wasm:
	curl -OL https://github.com/bytecodealliance/wasmtime/releases/download/v36.0.2/wasi_snapshot_preview1.proxy.wasm
