
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
# Metadata callback tests (see metadata_test.c). The _reference build runs the
# same checks against the system libbrotlidec (>= 1.1.0).
# Phony so that cargo always gets to rebuild the library they link.
.PHONY: metadata_test metadata_test_reference metadata_differential
metadata_test: metadata_test.c brotli/decode.h target/debug/libbrotli_decompressor.[ds][ylo]*
	cargo build && gcc -Wall -Wextra -Werror -std=c99 -g -o metadata_test metadata_test.c -I. target/debug/libbrotli_decompressor.[ds][ylo]*
	./metadata_test
metadata_test_reference: metadata_test.c brotli/decode.h
	gcc -Wall -Wextra -Werror -std=c99 -g -o metadata_test_reference metadata_test.c -I. -lbrotlidec
	./metadata_test_reference
metadata_differential: metadata_test metadata_test_reference
	./metadata_test_reference --log 1 3000 > metadata_reference.log
	./metadata_test --log 1 3000 > metadata_ours.log
	diff metadata_reference.log metadata_ours.log && echo "metadata_differential: logs match"
clean:
	cargo clean && rm -f main main_d metadata_test metadata_test_reference metadata_reference.log metadata_ours.log
