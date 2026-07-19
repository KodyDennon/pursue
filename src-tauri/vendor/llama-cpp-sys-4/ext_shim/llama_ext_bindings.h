#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "llama.h"

// Declaration-only view of the llama-ext C++ ABI used by llama-cpp-4. Keep
// this deliberately narrow so binding generation never depends on host C++
// standard-library implementation details.
LLAMA_API int32_t llama_model_n_expert(const struct llama_model * model);
LLAMA_API int32_t llama_model_n_devices(const struct llama_model * model);
LLAMA_API ggml_backend_dev_t llama_model_get_device(const struct llama_model * model, int i);

LLAMA_API void llama_set_embeddings_nextn(struct llama_context * ctx, bool value, bool masked);
LLAMA_API void llama_set_nextn_layer_offset(struct llama_context * ctx, int32_t offset);
LLAMA_API float * llama_get_embeddings_nextn(struct llama_context * ctx);
LLAMA_API float * llama_get_embeddings_nextn_ith(struct llama_context * ctx, int32_t i);
LLAMA_API void llama_set_embeddings_layer_inp(struct llama_context * ctx, uint32_t lid, bool value);
LLAMA_API float * llama_get_embeddings_layer_inp(struct llama_context * ctx, uint32_t lid);
LLAMA_API llama_context * llama_get_ctx_other(struct llama_context * ctx);

LLAMA_API const int32_t * llama_model_target_layer_ids(const struct llama_model * model);
LLAMA_API uint32_t llama_model_target_layer_ids_n(const struct llama_model * model);
