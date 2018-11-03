
main: main_d main.c target/release/libbrotli_decompressor.[ds][ylo]*
	gcc -O3 -o main -g main.c -I. target/release/libbrotli_decompressor.[ds][ylo]*
main_d: main.c target/debug/libbrotli_decompressor.[ds][ylo]*
	cargo build && gcc -o main_d -g main.c -I. target/debug/libbrotli_decompressor.[ds][ylo]* 
target/release/libbrotli_decompressor.so:
	cargo build #--release --no-default-features --features=no-stdlib-ffi-binding
target/debug/libbrotli_decompressor.so:
	cargo build #--no-default-features --features=no-stdlib-ffi-binding
target/release/libbrotli_decompressor.[ds][ylo]*:
	cargo build --release #--no-default-features --features=no-stdlib-ffi-binding
target/debug/libbrotli_decompressor.[ds][ylo]*:
	cargo build #--no-default-features --features=no-stdlib-ffi-binding
clean:
	cargo clean && rm -f main main_d
