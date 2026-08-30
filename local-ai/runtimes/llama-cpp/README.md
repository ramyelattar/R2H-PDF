# R2H PDF — llama.cpp Runtime

## Required Binaries

Place the following executables in this directory:

### LLM Generation (required)
- `llama-cli.exe` — Text generation ✅ Present

### Dense Embeddings (optional, enables hybrid retrieval)
- `llama-server.exe` — Embedding server (runs on 127.0.0.1:18080) ✅ Present

Without `llama-server.exe`, the app uses BM25 keyword retrieval as fallback.
With it, the app uses hybrid retrieval (dense + BM25) for better accuracy.

### Embedding Server Usage

The app starts `llama-server.exe` automatically when building a document index:
```
llama-server.exe -m <embedding-model>.gguf --embedding --host 127.0.0.1 --port 18080 --offline
```

- Binds to localhost only (127.0.0.1)
- No external network access (--offline)
- No model downloads at runtime
- Stopped automatically when the app closes

### How to get llama-server.exe

Download from the llama.cpp releases:
https://github.com/ggerganov/llama.cpp/releases

Look for the Windows release zip (e.g. `llama-bXXXX-bin-win-avx2-x64.zip`).
Extract `llama-server.exe` into this directory.

### Supported DLLs

The following DLLs must be present (already included):
- ggml-base.dll
- ggml-cpu-*.dll (CPU dispatch variants)
- ggml.dll
- llama.dll
- llama-common.dll

### Models

- LLM: `local-ai/models/llm/Qwen3-4B-GGUF/Qwen3-4B-Q4_K_M.gguf`
- Embeddings: `local-ai/models/embeddings/Qwen3-Embedding-4B-GGUF/Qwen3-Embedding-4B-Q4_K_M.gguf`
- Reranker: `local-ai/models/rerankers/Qwen3-Reranker-0.6B-GGUF/qwen3-reranker-0.6b-q8_0.gguf`

### Flags used

Generation: `--single-turn --reasoning off --offline --no-display-prompt`
Embedding server: `-m <model> --embedding --host 127.0.0.1 --port 18080 --offline`
