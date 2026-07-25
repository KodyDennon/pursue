// Tier membership only. Download sources, pinned revisions, and integrity hashes live in the
// Rust registry (src-tauri/src/analysis/registry.rs) and are resolved by `provision_model` from
// the id alone — duplicating URLs here is how a bare repo id once reached the downloader.
export const MODELS = {
	Standard: [
		{
			id: 'bge-small',
			name: 'BGE Small v1.5',
			filename: 'bge-small-en-v1.5.onnx'
		},
		{
			id: 'tokenizer',
			name: 'BGE Tokenizer',
			filename: 'tokenizer.json'
		},
		{
			id: 'gemma-4-e4b-q4',
			name: 'Gemma 4 E4B IT (Official QAT Q4_0)',
			filename: 'gemma-4-E4B_q4_0-it.gguf'
		}
	],
	Elite: [
		{
			id: 'bge-small',
			name: 'BGE Small v1.5',
			filename: 'bge-small-en-v1.5.onnx'
		},
		{
			id: 'tokenizer',
			name: 'BGE Tokenizer',
			filename: 'tokenizer.json'
		},
		{
			id: 'gemma-4-e4b-q4',
			name: 'Gemma 4 E4B IT (Official QAT Q4_0)',
			filename: 'gemma-4-E4B_q4_0-it.gguf'
		}
	]
};
