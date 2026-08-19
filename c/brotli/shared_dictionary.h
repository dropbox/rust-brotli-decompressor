/* Copyright 2017 Google Inc. All Rights Reserved.

   Distributed under MIT license.
   See file LICENSE for detail or copy at https://opensource.org/licenses/MIT
*/

/* Shared Dictionary public types used by the decoder API. */

#ifndef BROTLI_COMMON_SHARED_DICTIONARY_H_
#define BROTLI_COMMON_SHARED_DICTIONARY_H_

#include <brotli/port.h>
#include <brotli/types.h>

#if defined(__cplusplus) || defined(c_plusplus)
extern "C" {
#endif

#define SHARED_BROTLI_MIN_DICTIONARY_WORD_LENGTH 4
#define SHARED_BROTLI_MAX_DICTIONARY_WORD_LENGTH 31
#define SHARED_BROTLI_NUM_DICTIONARY_CONTEXTS 64
#define SHARED_BROTLI_MAX_COMPOUND_DICTS 15

/* Practical limit for the total size of attached raw dictionaries: 2 GiB on
 * 64-bit platforms and 128 MiB on 32-bit platforms. */
#define SHARED_BROTLI_MAX_RAW_DICT_SIZE (1u << (23u + sizeof(size_t)))

/**
 * Input data type for ::BrotliDecoderAttachDictionary.
 */
typedef enum BrotliSharedDictionaryType {
  /** Raw LZ77 prefix dictionary. */
  BROTLI_SHARED_DICTIONARY_RAW = 0,
  /** Serialized shared dictionary in the 0x91 0x00 shared-brotli container. */
  BROTLI_SHARED_DICTIONARY_SERIALIZED = 1
} BrotliSharedDictionaryType;

#if defined(__cplusplus) || defined(c_plusplus)
}  /* extern "C" */
#endif

#endif  /* BROTLI_COMMON_SHARED_DICTIONARY_H_ */
