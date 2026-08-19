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
    size_t decoded_size = sizeof(output);
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
    size_t decoded_size = sizeof(output);
    unsigned char scratch_u8[131072] = {0};
    uint32_t scratch_u32[16384] = {0};
    HuffmanCode HuffmanCodeZero = {0,0};
    HuffmanCode scratch_hc[65536] = {HuffmanCodeZero};
    BrotliDecoderReturnInfo ret = BrotliDecoderDecompressPrealloc(sizeof(brotli_file), brotli_file, decoded_size, output,
                                                                  sizeof(scratch_u8), scratch_u8,
                                                                  sizeof(scratch_u32) / sizeof(uint32_t), scratch_u32,
                                                                  sizeof(scratch_hc) / sizeof(HuffmanCode), scratch_hc);
    assert(ret.decoded_size == sizeof(key));
    assert(memcmp(output, key, sizeof(key) - 1) == 0);
    assert(output[sizeof(key) - 1] == '\n');
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
    BrotliDecoderDestroyInstance(state);
}

/* Matches google-brotli/java/org/brotli/dec/CompoundDictionaryTest.java and
 * verifies that the C header, exported symbol, and raw attach path agree. */
void shared_dictionary_test() {
    const unsigned char compressed[] = {
        0xa1, 0xa8, 0x00, 0xc0, 0x2f, 0x01, 0x10, 0xc4, 0x44, 0x09, 0x00};
    const unsigned char dictionary[] = "Kot lomom kolol slona!";
    unsigned char output[sizeof(dictionary) - 1];
    size_t avail_in = sizeof(compressed);
    size_t avail_out = sizeof(output);
    size_t total_out = 0;
    const unsigned char *input = compressed;
    unsigned char *output_ptr = output;
    BrotliDecoderState *state = BrotliDecoderCreateInstance(
        custom_alloc, custom_free, &custom_alloc_data);
    assert(state != NULL);
    assert(BrotliDecoderAttachDictionary(
        state, BROTLI_SHARED_DICTIONARY_RAW,
        sizeof(dictionary) - 1, dictionary) == BROTLI_TRUE);
    assert(BrotliDecoderDecompressStream(
        state, &avail_in, &input, &avail_out, &output_ptr, &total_out) ==
        BROTLI_DECODER_RESULT_SUCCESS);
    assert(total_out == sizeof(output));
    assert(memcmp(output, dictionary, sizeof(output)) == 0);
    BrotliDecoderDestroyInstance(state);
}

int main() {
    simple_test();
    simple_prealloc_test();
    negative_test();
    shared_dictionary_test();
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
                size_t written = (size_t)(o_ptr - &obuffer[0]);
                size_t ret = fwrite(obuffer, 1, written, stdout);
                assert(ret == written);
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
