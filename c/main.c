#include "brotli/decode.h"
#include <stdlib.h>
#include <stdio.h>
#include <assert.h>
#include <string.h>
int custom_alloc_data = 0;
void * custom_alloc(void*opaque, size_t size) {
    assert(opaque == &custom_alloc_data);
    return malloc(size);
}
void custom_free(void*opaque, void* addr) {
    assert(opaque == &custom_alloc_data);
    free(addr);
}

void simple_test() {
    const unsigned char brotli_file[] = {0x1b, 0x30, 0x00, 0xe0, 0x8d, 0xd4, 0x59, 0x2d, 0x39, 0x37, 0xb5, 0x02,
                                   0x48, 0x10, 0x95, 0x2a, 0x9a, 0xea, 0x42, 0x0e, 0x51, 0xa4, 0x16, 0xb9,
                                   0xcb, 0xf5, 0xf8, 0x5c, 0x64, 0xb9, 0x2f, 0xc9, 0x6a, 0x3f, 0xb1, 0xdc,
                                   0xa8, 0xe0, 0x35, 0x07};
    const unsigned char key[] = "THIS IS A TEST OF THE EMERGENCY BROADCAST SYSTEM";
    unsigned char output[sizeof(key) * 2];
    size_t decoded_size = sizeof(output) * 2;
    BrotliDecoderReturnInfo ret;
    BrotliDecoderDecompress(sizeof(brotli_file), brotli_file, &decoded_size, output);
    assert(decoded_size == sizeof(key));
    assert(memcmp(output, key, sizeof(key) - 1) == 0);
    assert(output[sizeof(key) - 1] == '\n');
    memset(output, 0xfc, sizeof(output));
    ret = BrotliDecoderDecompressWithReturnInfo(sizeof(brotli_file), brotli_file, decoded_size, output);
    assert(ret.decoded_size == sizeof(key));
    assert(memcmp(output, key, sizeof(key) - 1) == 0);
    assert(output[sizeof(key) - 1] == '\n');
}
void simple_prealloc_test() {
    const unsigned char brotli_file[] = {0x1b, 0x30, 0x00, 0xe0, 0x8d, 0xd4, 0x59, 0x2d, 0x39, 0x37, 0xb5, 0x02,
                                   0x48, 0x10, 0x95, 0x2a, 0x9a, 0xea, 0x42, 0x0e, 0x51, 0xa4, 0x16, 0xb9,
                                   0xcb, 0xf5, 0xf8, 0x5c, 0x64, 0xb9, 0x2f, 0xc9, 0x6a, 0x3f, 0xb1, 0xdc,
                                   0xa8, 0xe0, 0x35, 0x07};
    const unsigned char key[] = "THIS IS A TEST OF THE EMERGENCY BROADCAST SYSTEM";
    unsigned char output[sizeof(key) * 2];
    size_t decoded_size = sizeof(output) * 2;
    unsigned char scratch_u8[131072] = {0};
    uint32_t scratch_u32[16384] = {0};
    /* A 16-bit prefix must not leave an embedded HuffmanCode array only
       2-byte aligned, even when the enclosing struct is 4-byte aligned. */
    struct {
        uint32_t alignment;
        uint16_t prefix;
        HuffmanCode codes[65536];
    } scratch_hc = {0};
    BrotliDecoderReturnInfo ret = BrotliDecoderDecompressPrealloc(sizeof(brotli_file), brotli_file, decoded_size, output,
                                                                  sizeof(scratch_u8), scratch_u8,
                                                                  sizeof(scratch_u32) / sizeof(uint32_t), scratch_u32,
                                                                  sizeof(scratch_hc.codes) / sizeof(HuffmanCode), scratch_hc.codes);
    assert(ret.decoded_size == sizeof(key));
    assert(memcmp(output, key, sizeof(key) - 1) == 0);
    assert(output[sizeof(key) - 1] == '\n');
}
void large_window_parameter_test() {
    /* key, compressed with brotli --large_window=30 -q 11 */
    const unsigned char brotli_file[] = {0x11, 0x5e, 0xf8, 0x00, 0xe0, 0x77, 0x26, 0xd8, 0xb1, 0x90, 0xc9, 0x29,
                                   0xc1, 0x4e, 0x78, 0x2d, 0xc4, 0xf4, 0x26, 0xa7, 0x14, 0x40, 0x2a, 0x84,
                                   0xae, 0xd2, 0xd8, 0x7a, 0x10, 0x42, 0x14, 0xf1, 0x47, 0x92, 0x40, 0x1e,
                                   0x00, 0x21, 0xbd, 0xff, 0x5f, 0xb8, 0xa9, 0x00};
    const unsigned char key[] = "Large Window Brotli! Large Window Brotli! Large Window Brotli!\n";
    const BrotliDecoderParameter keys[] = {BROTLI_DECODER_PARAM_LARGE_WINDOW};
    const uint32_t values[] = {0};
    unsigned char output[sizeof(key) * 2];
    static unsigned char scratch_u8[131072];
    static uint32_t scratch_u32[16384];
    static HuffmanCode scratch_hc[65536];
    BrotliDecoderReturnInfo ret;
    /* The one-shot decoders accept large-window streams by default... */
    ret = BrotliDecoderDecompressWithParameters(0, NULL, NULL, sizeof(brotli_file), brotli_file,
                                                sizeof(output), output);
    assert(ret.result == BROTLI_DECODER_RESULT_SUCCESS);
    assert(ret.decoded_size == sizeof(key) - 1);
    assert(memcmp(output, key, sizeof(key) - 1) == 0);
    /* ...unless BROTLI_DECODER_PARAM_LARGE_WINDOW is 0. */
    ret = BrotliDecoderDecompressWithParameters(1, keys, values, sizeof(brotli_file), brotli_file,
                                                sizeof(output), output);
    assert(ret.result == BROTLI_DECODER_RESULT_ERROR);
    assert(ret.code == BROTLI_DECODER_ERROR_FORMAT_WINDOW_BITS);
    assert(ret.decoded_size == 0);
    ret = BrotliDecoderDecompressPreallocWithParameters(1, keys, values, sizeof(brotli_file), brotli_file,
                                                        sizeof(output), output,
                                                        sizeof(scratch_u8), scratch_u8,
                                                        sizeof(scratch_u32) / sizeof(uint32_t), scratch_u32,
                                                        sizeof(scratch_hc) / sizeof(HuffmanCode), scratch_hc);
    assert(ret.result == BROTLI_DECODER_RESULT_ERROR);
    assert(ret.code == BROTLI_DECODER_ERROR_FORMAT_WINDOW_BITS);
    assert(ret.decoded_size == 0);
}
void negative_test() {
    BrotliDecoderState * state = BrotliDecoderCreateInstance(custom_alloc, custom_free, &custom_alloc_data);
    const unsigned char brotli_file[] = {0x1b, 0x30, 0x00, 0xe0, 0x8d, 0xd4, 0x59, 0x2d, 0x39, 0xff, 0xb5, 0x02,
                                   0x48, 0x10, 0x95, 0x2a, 0x9a, 0xea, 0x42, 0x0e, 0x51, 0xa4, 0x16, 0xb9,
                                   0xcb, 0xf5, 0xf8, 0x5c, 0x64, 0xb9, 0x2f, 0xc9, 0x6a, 0x3f, 0xb1, 0xdc,
                                   0xa8, 0xe0, 0x35, 0x07};
    size_t avail_in = sizeof(brotli_file);
    size_t avail_out = 0;
    unsigned char obuffer[4096];
    size_t total_out = 0;
    const unsigned char *i_ptr = &brotli_file[0];
    BrotliDecoderReturnInfo return_info =
        BrotliDecoderDecompressWithReturnInfo(
            sizeof(brotli_file), brotli_file, sizeof(obuffer), obuffer);

    unsigned char *o_ptr = &obuffer[0];
    const char * to_be_printed;
    BrotliDecoderResult rest = BrotliDecoderDecompressStream(state, &avail_in, &i_ptr, &avail_out, &o_ptr, &total_out);
    assert(return_info.result == BROTLI_DECODER_RESULT_ERROR);
    assert(return_info.code == BROTLI_DECODER_ERROR_FORMAT_CONTEXT_MAP_REPEAT);
    assert(rest ==  BROTLI_DECODER_RESULT_ERROR);
    to_be_printed = BrotliDecoderGetErrorString(state);
    assert(strcmp(to_be_printed, "ERROR_FORMAT_CONTEXT_MAP_REPEAT") == 0);
}

int main() {
    simple_test();
    simple_prealloc_test();
    large_window_parameter_test();
    negative_test();
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
