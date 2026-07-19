#pragma once

#include <stddef.h>
#include <stdint.h>

#include "ggml.h"
#include "llama.h"

// Stable declaration-only view of common/fit.h for bindgen. These retain C++
// linkage and therefore resolve to the same symbols implemented by fit.cpp,
// without making libclang parse std::vector and the host C++ standard library.
enum common_params_fit_status {
    COMMON_PARAMS_FIT_STATUS_SUCCESS = 0,
    COMMON_PARAMS_FIT_STATUS_FAILURE = 1,
    COMMON_PARAMS_FIT_STATUS_ERROR   = 2,
};

common_params_fit_status common_fit_params(
        const char * path_model,
        llama_model_params * mparams,
        llama_context_params * cparams,
        float * tensor_split,
        llama_model_tensor_buft_override * tensor_buft_overrides,
        size_t * margins,
        uint32_t n_ctx_min,
        ggml_log_level log_level);

void common_fit_print(
        const char * path_model,
        llama_model_params * mparams,
        llama_context_params * cparams);

void common_memory_breakdown_print(const llama_context * ctx);
