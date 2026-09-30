"""A webhook receiver that writes what it was sent, and nothing else.

    python3 scripts/lib/webhook-sink.py <port> <outFile>

Used by the e2e to answer one question: did the server actually POST
when an issue was created. A mock inside the server could not answer
it — the whole point is that the transport was registered, reached and
used, and only something outside the process sees that.
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

port, out = int(sys.argv[1]), sys.argv[2]
received = []


class Sink(BaseHTTPRequestHandler):
    def do_POST(self):  # noqa: N802 — the base class names it
        n = int(self.headers.get("content-length") or 0)
        raw = self.rfile.read(n).decode("utf-8", "replace")
        try:
            body = json.loads(raw)
        except ValueError:
            body = {"unparsed": raw}
        # The caller knocks on `/ping` to learn the port is open. That
        # is not a delivery, and counting it made the first assertion
        # read the probe instead of the product.
        if self.path == "/ping":
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"ok")
            return
        received.append({"path": self.path, "body": body})
        with open(out, "w", encoding="utf-8") as f:
            json.dump(received, f)
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b"ok")

    def log_message(self, *_args):
        # The e2e's own output is the log; this would interleave with it.
        pass


HTTPServer(("127.0.0.1", port), Sink).serve_forever()
