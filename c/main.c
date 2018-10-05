#include "brotli/decode.h"
#include <stdlib.h>
#include <stdio.h>
#include <assert.h>
int custom_alloc_data = 0;
void * custom_alloc(void*opaque, size_t size) {
    assert(opaque == &custom_alloc_data);
    return malloc(size);
}
void custom_free(void*opaque, void* addr) {
    assert(opaque == &custom_alloc_data);
    free(addr);
}

int main() {
    BrotliDecoderState * state = BrotliDecoderCreateInstance(custom_alloc, custom_free, &custom_alloc_data);
    unsigned char ibuffer[4096];
    unsigned char obuffer[4096];
    size_t total_out = 0;
    BrotliDecoderResult rest;
    while(1) {
        size_t avail_in = fread(ibuffer, 1, sizeof(ibuffer), stdin);
        int is_eof = (avail_in == 0);
        const unsigned char *i_ptr = &ibuffer[0];
        while (1) {
            unsigned char *o_ptr = &obuffer[0];
            size_t avail_out = sizeof(obuffer);
            rest = BrotliDecoderDecompressStream(state, &avail_in, &i_ptr, &avail_out, &o_ptr, &total_out);
            if (o_ptr != &obuffer[0]) {
                size_t ret = fwrite(obuffer, 1, o_ptr - &obuffer[0], stdout);
                assert(ret == o_ptr - &obuffer[0]);
            }
            if (rest == BROTLI_DECODER_RESULT_NEEDS_MORE_INPUT) {
                break;
            }
            if (rest == BROTLI_DECODER_RESULT_SUCCESS || rest == BROTLI_DECODER_RESULT_ERROR) {
                break;
            }
        }
        if (rest == BROTLI_DECODER_RESULT_NEEDS_MORE_INPUT && is_eof) {
            fprintf(stderr, "Unexpected EOF\n");
            exit(1);
        }
        if (rest == BROTLI_DECODER_RESULT_SUCCESS || rest == BROTLI_DECODER_RESULT_ERROR) {
            break;
        }
    }
    BrotliDecoderDestroyInstance(state);
}
