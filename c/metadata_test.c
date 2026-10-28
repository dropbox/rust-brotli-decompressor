/* Tests BrotliDecoderSetMetadataCallbacks through the C header.
 *
 * Only the public API is used, so the same source also links against the
 * reference decoder (libbrotlidec >= 1.1.0); see the Makefile:
 *
 *   make metadata_test            self-checking run against this library
 *   make metadata_test_reference  the same checks against libbrotlidec
 *   make metadata_differential    compare event logs of the two decoders on
 *                                 randomized, corrupted and truncated streams
 *
 * No encoder emits metadata meta-blocks, so streams are assembled bit by bit
 * (see the Builder functions), as in src/test_metadata.rs.
 */
#include "brotli/decode.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures = 0;
#define CHECK(cond)                                                     \
  do {                                                                  \
    if (!(cond)) {                                                      \
      fprintf(stderr, "%s:%d: CHECK failed: %s\n", __FILE__, __LINE__, #cond); \
      failures++;                                                       \
    }                                                                   \
  } while (0)

static void* xrealloc(void* p, size_t size) {
  p = realloc(p, size ? size : 1);
  if (!p) {
    fprintf(stderr, "out of memory\n");
    exit(2);
  }
  return p;
}

typedef struct {
  uint8_t* data;
  size_t len;
  size_t cap;
} Bytes;

static void bytes_push(Bytes* b, const uint8_t* data, size_t len) {
  if (b->len + len > b->cap) {
    b->cap = (b->len + len) * 2 + 16;
    b->data = (uint8_t*)xrealloc(b->data, b->cap);
  }
  if (len) memcpy(b->data + b->len, data, len);
  b->len += len;
}

static int bytes_equal(const Bytes* b, const uint8_t* data, size_t len) {
  return b->len == len && (len == 0 || memcmp(b->data, data, len) == 0);
}

/* ---- Stream builder ---------------------------------------------------- */

typedef struct {
  Bytes stream;
  size_t bit_count;
  Bytes output;          /* what a decoder must produce */
  Bytes metadata;        /* all metadata bytes, concatenated */
  size_t sizes[256];     /* announced size of each metadata block */
  size_t num_blocks;
} Builder;

static void put(Builder* b, int n_bits, uint32_t value) {
  int i;
  for (i = 0; i < n_bits; i++) {
    uint8_t zero = 0;
    if (b->bit_count % 8 == 0) bytes_push(&b->stream, &zero, 1);
    b->stream.data[b->stream.len - 1] |=
        (uint8_t)(((value >> i) & 1) << (b->bit_count % 8));
    b->bit_count++;
  }
}

static void align(Builder* b) { b->bit_count = (b->bit_count + 7) & ~(size_t)7; }

static void raw_bytes(Builder* b, const uint8_t* data, size_t len) {
  bytes_push(&b->stream, data, len);
  b->bit_count += 8 * len;
}

static void builder_init(Builder* b) {
  memset(b, 0, sizeof(*b));
  put(b, 1, 0); /* WBITS = 16 */
}

static void builder_free(Builder* b) {
  free(b->stream.data);
  free(b->output.data);
  free(b->metadata.data);
}

static void metadata_block(Builder* b, int is_last, const uint8_t* data, size_t len) {
  uint32_t skip_bytes, i;
  put(b, 1, (uint32_t)is_last);           /* ISLAST */
  if (is_last) put(b, 1, 0);              /* ISLASTEMPTY */
  put(b, 2, 3);                           /* MNIBBLES = 0: metadata */
  put(b, 1, 0);                           /* reserved */
  skip_bytes = len == 0 ? 0 : len - 1 < (1u << 8) ? 1 : len - 1 < (1u << 16) ? 2 : 3;
  put(b, 2, skip_bytes);                  /* MSKIPBYTES */
  for (i = 0; i < skip_bytes; i++) put(b, 8, (uint32_t)((len - 1) >> (8 * i)) & 0xff);
  align(b);
  raw_bytes(b, data, len);
  bytes_push(&b->metadata, data, len);
  b->sizes[b->num_blocks++] = len;
}

static void uncompressed_block(Builder* b, const uint8_t* data, size_t len) {
  put(b, 1, 0);                           /* ISLAST */
  put(b, 2, 0);                           /* MNIBBLES = 4 */
  put(b, 16, (uint32_t)(len - 1));
  put(b, 1, 1);                           /* ISUNCOMPRESSED */
  align(b);
  raw_bytes(b, data, len);
  bytes_push(&b->output, data, len);
}

/* `count` (1..5) copies of `literal` with single-symbol prefix codes. */
static void literals_block(Builder* b, uint8_t literal, size_t count) {
  size_t i;
  put(b, 1, 0); put(b, 2, 0); put(b, 16, (uint32_t)(count - 1));
  put(b, 1, 0);                           /* ISUNCOMPRESSED */
  put(b, 1, 0); put(b, 1, 0); put(b, 1, 0); /* NBLTYPES L, I, D = 1 */
  put(b, 2, 0); put(b, 4, 0);             /* NPOSTFIX, NDIRECT */
  put(b, 2, 0);                           /* literal context mode */
  put(b, 1, 0); put(b, 1, 0);             /* NTREESL, NTREESD = 1 */
  put(b, 2, 1); put(b, 2, 0); put(b, 8, literal);
  put(b, 2, 1); put(b, 2, 0); put(b, 10, (uint32_t)(count * 8));
  put(b, 2, 1); put(b, 2, 0); put(b, 6, 0);
  for (i = 0; i < count; i++) bytes_push(&b->output, &literal, 1);
}

static void finish(Builder* b) {
  put(b, 1, 1); /* ISLAST */
  put(b, 1, 1); /* ISLASTEMPTY */
}

static void pattern(uint8_t* out, size_t len, unsigned seed) {
  size_t i;
  for (i = 0; i < len; i++) out[i] = (uint8_t)((i * 31 + seed * 7 + 1) ^ (i >> 8));
}

/* ---- Callback recorder ------------------------------------------------- */

typedef struct {
  size_t starts[512];
  size_t num_starts;
  size_t chunk_lens[8192];
  size_t num_chunks;
  Bytes data;
  size_t pending;      /* bytes still owed to the current block */
  int contract_ok;
} Recorder;

static void recorder_init(Recorder* r) {
  memset(r, 0, sizeof(*r));
  r->contract_ok = 1;
}

static void on_start(void* opaque, size_t size) {
  Recorder* r = (Recorder*)opaque;
  if (r->pending != 0) r->contract_ok = 0; /* previous block unfinished */
  if (r->num_starts < 512) r->starts[r->num_starts] = size;
  r->num_starts++;
  r->pending = size;
}

static void on_chunk(void* opaque, const uint8_t* data, size_t size) {
  Recorder* r = (Recorder*)opaque;
  if (size == 0 || size > r->pending) r->contract_ok = 0;
  r->pending -= size < r->pending ? size : r->pending;
  if (r->num_chunks < 8192) r->chunk_lens[r->num_chunks] = size;
  r->num_chunks++;
  bytes_push(&r->data, data, size);
}

/* ---- Decoding driver --------------------------------------------------- */

typedef struct {
  BrotliDecoderResult result;
  int error_code;
  Bytes output;
  size_t consumed;
} Outcome;

/* Feeds `input` in pieces ending at each of `cuts` (ascending) and then the
 * rest, with `out_step` bytes of output space per call. */
static Outcome decode(const uint8_t* input, size_t len, const size_t* cuts, size_t num_cuts,
                      size_t out_step, Recorder* rec) {
  Outcome o;
  uint8_t out[64];
  size_t piece, start = 0;
  BrotliDecoderState* s = BrotliDecoderCreateInstance(0, 0, 0);
  memset(&o, 0, sizeof(o));
  if (out_step > sizeof(out)) out_step = sizeof(out);
  if (rec) BrotliDecoderSetMetadataCallbacks(s, on_start, on_chunk, rec);
  o.result = BROTLI_DECODER_RESULT_NEEDS_MORE_INPUT;
  for (piece = 0; piece <= num_cuts; piece++) {
    size_t end = piece < num_cuts ? cuts[piece] : len;
    size_t available_in;
    const uint8_t* next_in;
    if (end < start) continue;
    available_in = end - start;
    next_in = input + start;
    do {
      size_t available_out = out_step;
      uint8_t* next_out = out;
      o.result = BrotliDecoderDecompressStream(s, &available_in, &next_in, &available_out,
                                               &next_out, 0);
      bytes_push(&o.output, out, out_step - available_out);
    } while (o.result == BROTLI_DECODER_RESULT_NEEDS_MORE_OUTPUT);
    start = end - available_in;
    if (o.result != BROTLI_DECODER_RESULT_NEEDS_MORE_INPUT) break;
  }
  o.consumed = start;
  o.error_code = (int)BrotliDecoderGetErrorCode(s);
  BrotliDecoderDestroyInstance(s);
  return o;
}

/* Decodes `b`'s stream whole, byte by byte, and split once at every
 * position, checking output, metadata and the callback contract. */
static void check_builder(const Builder* b) {
  size_t split, i;
  for (split = 0; split <= b->stream.len + 1; split++) {
    size_t cuts[1 << 12];
    size_t num_cuts = 0, out_step;
    if (split == b->stream.len + 1) {
      for (i = 1; i < b->stream.len && num_cuts < (1 << 12); i++) cuts[num_cuts++] = i;
    } else if (split != 0) {
      cuts[num_cuts++] = split;
    }
    for (out_step = 1; out_step <= 64; out_step *= 64) {
      Recorder rec;
      Outcome o;
      recorder_init(&rec);
      o = decode(b->stream.data, b->stream.len, cuts, num_cuts, out_step, &rec);
      CHECK(o.result == BROTLI_DECODER_RESULT_SUCCESS);
      CHECK(bytes_equal(&o.output, b->output.data, b->output.len));
      CHECK(o.consumed == b->stream.len);
      CHECK(rec.contract_ok && rec.pending == 0);
      CHECK(rec.num_starts == b->num_blocks);
      CHECK(memcmp(rec.starts, b->sizes, b->num_blocks * sizeof(size_t)) == 0);
      CHECK(bytes_equal(&rec.data, b->metadata.data, b->metadata.len));
      free(rec.data.data);
      free(o.output.data);
    }
  }
}

/* ---- Deterministic cases ----------------------------------------------- */

static uint8_t scratch[70000];

static void test_length_classes(void) {
  static const size_t lens[] = {0, 1, 2, 7, 8, 9, 255, 256, 257, 65535, 65536, 65537, 70000};
  size_t i;
  for (i = 0; i < sizeof(lens) / sizeof(lens[0]); i++) {
    Builder b;
    Recorder rec;
    Outcome o;
    builder_init(&b);
    pattern(scratch, lens[i], (unsigned)i);
    metadata_block(&b, 0, scratch, lens[i]);
    finish(&b);
    recorder_init(&rec);
    o = decode(b.stream.data, b.stream.len, 0, 0, 64, &rec);
    CHECK(o.result == BROTLI_DECODER_RESULT_SUCCESS);
    CHECK(rec.contract_ok && rec.num_starts == 1 && rec.starts[0] == lens[i]);
    CHECK(bytes_equal(&rec.data, scratch, lens[i]));
    free(rec.data.data);
    free(o.output.data);
    builder_free(&b);
  }
}

static void test_mixed_streams(void) {
  size_t count, len;
  for (count = 1; count <= 5; count++) {
    for (len = 0; len < 20; len += 3) {
      Builder b;
      uint8_t tail[40];
      builder_init(&b);
      pattern(scratch, len, (unsigned)count);
      pattern(tail, sizeof(tail), 99);
      literals_block(&b, 'a', count);
      metadata_block(&b, 0, scratch, len);   /* starts in the accumulator */
      metadata_block(&b, 0, tail, sizeof(tail));
      literals_block(&b, 'b', 6 - count);    /* after a cold bit reader */
      uncompressed_block(&b, (const uint8_t*)"plain", 5);
      metadata_block(&b, 0, scratch, len);
      finish(&b);
      check_builder(&b);
      builder_free(&b);
    }
  }
}

static void test_last_block_is_metadata(void) {
  size_t count, len;
  for (count = 1; count <= 5; count++) {
    for (len = 0; len < 12; len++) {
      Builder b;
      Outcome o;
      Recorder rec;
      size_t stream_len;
      builder_init(&b);
      pattern(scratch, len, 5);
      literals_block(&b, 'L', count);
      metadata_block(&b, 1, scratch, len);
      check_builder(&b);
      /* Trailing bytes stay unconsumed, even if the reader prefetched them. */
      stream_len = b.stream.len;
      raw_bytes(&b, (const uint8_t*)"TRAILING", 8);
      recorder_init(&rec);
      o = decode(b.stream.data, b.stream.len, 0, 0, 64, &rec);
      CHECK(o.result == BROTLI_DECODER_RESULT_SUCCESS);
      CHECK(o.consumed == stream_len);
      CHECK(bytes_equal(&rec.data, scratch, len));
      free(rec.data.data);
      free(o.output.data);
      builder_free(&b);
    }
  }
}

static void test_null_callbacks(void) {
  Builder b;
  BrotliDecoderState* s;
  size_t available_in, available_out, total_out = 0;
  const uint8_t* next_in;
  uint8_t out[64];
  uint8_t* next_out = out;
  Recorder rec;
  builder_init(&b);
  metadata_block(&b, 0, (const uint8_t*)"abc", 3);
  literals_block(&b, 'n', 2);
  finish(&b);

  /* A NULL state is not tested here: the reference decoder dereferences it.
   * (This library ignores it; see null_state_is_ignored in
   * src/test_metadata.rs.) Both callbacks NULL, and either one NULL: */
  s = BrotliDecoderCreateInstance(0, 0, 0);
  BrotliDecoderSetMetadataCallbacks(s, 0, 0, 0);
  available_in = b.stream.len; next_in = b.stream.data; available_out = sizeof(out);
  CHECK(BrotliDecoderDecompressStream(s, &available_in, &next_in, &available_out, &next_out,
                                      &total_out) == BROTLI_DECODER_RESULT_SUCCESS);
  CHECK(total_out == 2);
  BrotliDecoderDestroyInstance(s);

  recorder_init(&rec);
  s = BrotliDecoderCreateInstance(0, 0, 0);
  BrotliDecoderSetMetadataCallbacks(s, on_start, 0, &rec);
  available_in = b.stream.len; next_in = b.stream.data; available_out = sizeof(out);
  next_out = out;
  CHECK(BrotliDecoderDecompressStream(s, &available_in, &next_in, &available_out, &next_out,
                                      0) == BROTLI_DECODER_RESULT_SUCCESS);
  CHECK(rec.num_starts == 1 && rec.starts[0] == 3 && rec.num_chunks == 0);
  BrotliDecoderDestroyInstance(s);

  recorder_init(&rec);
  rec.pending = 3; /* no start callback will announce the size */
  s = BrotliDecoderCreateInstance(0, 0, 0);
  BrotliDecoderSetMetadataCallbacks(s, 0, on_chunk, &rec);
  available_in = b.stream.len; next_in = b.stream.data; available_out = sizeof(out);
  next_out = out;
  CHECK(BrotliDecoderDecompressStream(s, &available_in, &next_in, &available_out, &next_out,
                                      0) == BROTLI_DECODER_RESULT_SUCCESS);
  CHECK(rec.num_starts == 0 && bytes_equal(&rec.data, (const uint8_t*)"abc", 3));
  free(rec.data.data);
  BrotliDecoderDestroyInstance(s);

  /* The one-shot API takes no callbacks and skips metadata. */
  {
    size_t decoded = sizeof(out);
    CHECK(BrotliDecoderDecompress(b.stream.len, b.stream.data, &decoded, out) ==
          BROTLI_DECODER_RESULT_SUCCESS);
    CHECK(decoded == 2 && out[0] == 'n' && out[1] == 'n');
  }
  builder_free(&b);
}

static void test_replace_callbacks_mid_block(void) {
  Builder b;
  Recorder early, late;
  BrotliDecoderState* s;
  uint8_t first[20], second[20], out[16];
  size_t second_data, available_in, available_out;
  const uint8_t* next_in;
  uint8_t* next_out = out;
  pattern(first, 20, 1);
  pattern(second, 20, 2);
  builder_init(&b);
  metadata_block(&b, 0, first, 20);
  metadata_block(&b, 0, second, 20);
  finish(&b);
  second_data = 2 + 20 + 2; /* WBITS + first header, data, second header */
  CHECK(memcmp(b.stream.data + second_data, second, 20) == 0);

  recorder_init(&early);
  recorder_init(&late);
  s = BrotliDecoderCreateInstance(0, 0, 0);
  BrotliDecoderSetMetadataCallbacks(s, on_start, on_chunk, &early);
  available_in = second_data + 4; next_in = b.stream.data; available_out = sizeof(out);
  CHECK(BrotliDecoderDecompressStream(s, &available_in, &next_in, &available_out, &next_out,
                                      0) == BROTLI_DECODER_RESULT_NEEDS_MORE_INPUT);
  BrotliDecoderSetMetadataCallbacks(s, on_start, on_chunk, &late);
  available_in = b.stream.len - (size_t)(next_in - b.stream.data);
  CHECK(BrotliDecoderDecompressStream(s, &available_in, &next_in, &available_out, &next_out,
                                      0) == BROTLI_DECODER_RESULT_SUCCESS);
  BrotliDecoderDestroyInstance(s);

  CHECK(early.num_starts == 2 && early.starts[0] == 20 && early.starts[1] == 20);
  CHECK(early.data.len == 24 && memcmp(early.data.data, first, 20) == 0 &&
        memcmp(early.data.data + 20, second, 4) == 0);
  CHECK(late.num_starts == 0 && bytes_equal(&late.data, second + 4, 16));
  free(early.data.data);
  free(late.data.data);
  builder_free(&b);
}

static void test_invalid_headers(void) {
  /* reserved bit; MSKIPBYTES = 2 with a zero last byte; nonzero padding */
  int variant;
  for (variant = 0; variant < 3; variant++) {
    Builder b;
    Recorder rec;
    Outcome o;
    builder_init(&b);
    put(&b, 1, 0); put(&b, 2, 3);
    if (variant == 0) {
      put(&b, 1, 1);
    } else if (variant == 1) {
      put(&b, 1, 0); put(&b, 2, 2); put(&b, 8, 5); put(&b, 8, 0);
    } else {
      put(&b, 1, 0); put(&b, 2, 1); put(&b, 8, 0); put(&b, 1, 1);
    }
    finish(&b);
    recorder_init(&rec);
    o = decode(b.stream.data, b.stream.len, 0, 0, 64, &rec);
    CHECK(o.result == BROTLI_DECODER_RESULT_ERROR);
    CHECK(rec.num_starts == 0 && rec.num_chunks == 0);
    free(o.output.data);
    builder_free(&b);
  }
}

/* ---- Randomized streams for the differential log ----------------------- */

static uint32_t rng_state;
static uint32_t rnd(uint32_t n) {
  rng_state = rng_state * 1103515245u + 12345u;
  return ((rng_state >> 8) & 0xffffff) % n;
}

static uint32_t fnv(const uint8_t* data, size_t len) {
  uint32_t h = 2166136261u;
  size_t i;
  for (i = 0; i < len; i++) h = (h ^ data[i]) * 16777619u;
  return h;
}

static void random_stream(Builder* b) {
  int blocks = 1 + (int)rnd(8), i;
  builder_init(b);
  for (i = 0; i < blocks; i++) {
    uint32_t kind = rnd(4);
    if (kind <= 1) {
      size_t len = rnd(4) == 0 ? rnd(400) : rnd(12);
      if (rnd(40) == 0) len = 65530 + rnd(12);
      pattern(scratch, len, (unsigned)i);
      metadata_block(b, 0, scratch, len);
    } else if (kind == 2) {
      literals_block(b, (uint8_t)('a' + i), 1 + rnd(5));
    } else {
      size_t len = 1 + rnd(30);
      pattern(scratch, len, (unsigned)i + 50);
      uncompressed_block(b, scratch, len);
    }
  }
  if (rnd(3) == 0) {
    size_t len = rnd(12);
    pattern(scratch, len, 77);
    metadata_block(b, 1, scratch, len);
  } else {
    finish(b);
  }
  if (rnd(4) == 0) raw_bytes(b, (const uint8_t*)"junk", 4);
}

static void log_decode(int index, const char* schedule, const uint8_t* input, size_t len,
                       const size_t* cuts, size_t num_cuts, size_t out_step) {
  Recorder rec;
  Outcome o;
  size_t i;
  recorder_init(&rec);
  o = decode(input, len, cuts, num_cuts, out_step, &rec);
  printf("%d %s result=%d error=%d consumed=%zu out=%zu/%08x md=%zu/%08x contract=%d starts",
         index, schedule, (int)o.result, o.error_code, o.consumed, o.output.len,
         fnv(o.output.data, o.output.len), rec.data.len, fnv(rec.data.data, rec.data.len),
         rec.contract_ok);
  for (i = 0; i < rec.num_starts && i < 512; i++) printf(" %zu", rec.starts[i]);
  printf(" chunks");
  for (i = 0; i < rec.num_chunks && i < 8192; i++) printf(" %zu", rec.chunk_lens[i]);
  printf("\n");
  free(rec.data.data);
  free(o.output.data);
}

static void differential_log(uint32_t seed, int count) {
  int index;
  rng_state = seed;
  for (index = 0; index < count; index++) {
    Builder b;
    size_t cuts[1 << 12], num_cuts = 0, i;
    random_stream(&b);
    if (rnd(3) == 0) { /* corrupt one bit */
      size_t bit = rnd((uint32_t)b.stream.len * 8);
      b.stream.data[bit / 8] ^= (uint8_t)(1u << (bit % 8));
    }
    if (rnd(6) == 0) b.stream.len = rnd((uint32_t)b.stream.len); /* truncate */
    log_decode(index, "whole", b.stream.data, b.stream.len, 0, 0, 64);
    for (i = 1; i < b.stream.len && num_cuts < (1 << 12); i++) cuts[num_cuts++] = i;
    log_decode(index, "bytewise", b.stream.data, b.stream.len, cuts, num_cuts, 1);
    num_cuts = 0;
    for (i = 0; i < 3 && b.stream.len > 1; i++) {
      size_t cut = 1 + rnd((uint32_t)b.stream.len - 1);
      if (num_cuts == 0 || cut > cuts[num_cuts - 1]) cuts[num_cuts++] = cut;
    }
    log_decode(index, "random", b.stream.data, b.stream.len, cuts, num_cuts, 7);
    builder_free(&b);
  }
}

int main(int argc, char** argv) {
  if (argc == 4 && strcmp(argv[1], "--log") == 0) {
    differential_log((uint32_t)strtoul(argv[2], 0, 10), atoi(argv[3]));
    return 0;
  }
  test_length_classes();
  test_mixed_streams();
  test_last_block_is_metadata();
  test_null_callbacks();
  test_replace_callbacks_mid_block();
  test_invalid_headers();
  if (failures) {
    fprintf(stderr, "%d check(s) failed\n", failures);
    return 1;
  }
  printf("metadata_test: all checks passed\n");
  return 0;
}
