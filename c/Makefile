
main: main_d main.c target/release/libbrotli_decompressor.so 
	gcc -O3 -o main -g main.c -I. target/release/libbrotli_decompressor.so 
main_d: main.c target/debug/libbrotli_decompressor.so 
	cargo build && gcc -o main_d -g main.c -I. target/debug/libbrotli_decompressor.so 
target/release/libbrotli_decompressor.so:
	cargo build --release
target/debug/libbrotli_decompressor.so:
	cargo build
clean:
	cargo clean && rm -f main main_d
