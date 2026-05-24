import http from 'http';

const CHUNK_SIZE = 64 * 1024;
const TOTAL_BYTES = 10 * 1024 * 1024 * 1024; // 10 GB
const PORT = 8081;

const server = http.createServer((req, res) => {
	// Enable CORS so the Svelte frontend can fetch it
	res.setHeader('Access-Control-Allow-Origin', '*');
	res.setHeader('Access-Control-Allow-Methods', 'GET, OPTIONS');
	res.setHeader('Access-Control-Allow-Headers', 'Range');
	res.setHeader('Access-Control-Expose-Headers', 'Content-Length, Content-Range');

	if (req.method === 'OPTIONS') {
		res.writeHead(204);
		res.end();
		return;
	}

	let start = 0;
	let end = TOTAL_BYTES - 1;

	if (req.headers.range) {
		const parts = req.headers.range.replace(/bytes=/, '').split('-');
		start = parseInt(parts[0], 10);
		end = parts[1] ? parseInt(parts[1], 10) : TOTAL_BYTES - 1;
		res.setHeader('Content-Range', `bytes ${start}-${end}/${TOTAL_BYTES}`);
		res.setHeader('Accept-Ranges', 'bytes');
		res.statusCode = 206;
	} else {
		res.statusCode = 200;
	}

	const contentLength = end - start + 1;
	res.setHeader('Content-Length', contentLength);
	res.setHeader('Content-Type', 'application/octet-stream');

	let bytesSent = 0;
	const chunk = Buffer.alloc(CHUNK_SIZE, 0);

	const sendData = () => {
		let ok = true;
		while (bytesSent < contentLength && ok) {
			const bytesToSend = Math.min(CHUNK_SIZE, contentLength - bytesSent);
			ok = res.write(bytesToSend === CHUNK_SIZE ? chunk : chunk.slice(0, bytesToSend));
			bytesSent += bytesToSend;
		}

		if (bytesSent >= contentLength) {
			res.end();
		} else {
			res.once('drain', sendData);
		}
	};

	sendData();
});

server.listen(PORT, () => {
	console.log(`Stress test server listening on http://localhost:${PORT}`);
	console.log(`Serving a synthetic ${TOTAL_BYTES / 1e9}GB stream.`);
});
