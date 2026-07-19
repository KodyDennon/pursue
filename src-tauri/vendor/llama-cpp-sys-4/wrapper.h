// Only include the public headers that ship in llama.cpp/include/.
// llama-grammar.h and llama-sampler.h live in src/ (internal) — everything
// we need from them is already re-exported through llama.h.
#include "llama.h"
#include "fit_bindings.h"
#include "mtp_shim.h"
#include "ext_shim.h"

// Bind only the extension ABI consumed by llama-cpp-4. The upstream C++
// header also exposes implementation-only std::map and quantization test
// helpers; parsing those drags host standard-library internals into bindgen.
#include "llama_ext_bindings.h"

#ifdef RPC_SUPPORT
#include "ggml-rpc.h"
#endif

#ifdef MTMD_SUPPORT
#include "mtmd.h"
#include "mtmd-helper.h"
#endif
