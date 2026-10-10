"""Disposable SMTP sink and loopback reader for password-recovery browser tests.

Only the fixture owns this server. It stores mail in memory and sends nothing.
"""
import email
import email.policy
import http.server
import json
import socketserver
import threading


class Mailbox:
    def __init__(self):
        self.messages = []
        self.lock = threading.Lock()
        mailbox = self

        class SMTP(socketserver.StreamRequestHandler):
            def handle(self):
                self.connection.settimeout(15)
                self.wfile.write(b'220 fixture.example.test ESMTP\r\n')
                while True:
                    line = self.rfile.readline(65536)
                    if not line:
                        return
                    verb = line.split(b' ', 1)[0].strip().upper()
                    if verb in [b'EHLO', b'HELO']:
                        self.wfile.write(b'250 fixture.example.test\r\n')
                    elif verb == b'DATA':
                        self.wfile.write(b'354 End with a dot\r\n')
                        lines = []
                        while True:
                            line = self.rfile.readline(65536)
                            if not line or line == b'.\r\n':
                                break
                            lines.append(line[1:] if line.startswith(b'..') else line)
                            if sum(map(len, lines)) > 1000000:
                                raise ValueError('Fixture mail exceeds 1 MB')
                        message = email.message_from_bytes(b''.join(lines), policy=email.policy.default)
                        body = message.get_body(preferencelist=('plain',)) if message.is_multipart() else message
                        with mailbox.lock:
                            mailbox.messages.append({'to':str(message['To']), 'subject':str(message['Subject']), 'text':body.get_content()})
                        self.wfile.write(b'250 Received\r\n')
                    elif verb == b'QUIT':
                        self.wfile.write(b'221 Bye\r\n')
                        return
                    elif verb in [b'MAIL', b'RCPT', b'RSET', b'NOOP']:
                        self.wfile.write(b'250 OK\r\n')
                    else:
                        self.wfile.write(b'502 Unsupported\r\n')

        class Reader(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                if self.path != '/messages':
                    self.send_error(404)
                    return
                with mailbox.lock:
                    body = json.dumps(mailbox.messages).encode()
                self.send_response(200)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_):
                pass

        self.smtp = socketserver.ThreadingTCPServer(('0.0.0.0', 0), SMTP)
        self.smtp.daemon_threads = True
        self.http = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Reader)
        self.http.daemon_threads = True
        for server in [self.smtp, self.http]:
            threading.Thread(target=server.serve_forever, daemon=True).start()
        self.port = self.smtp.server_address[1]
        self.url = 'http://127.0.0.1:' + str(self.http.server_address[1])

    def close(self):
        for server in [self.smtp, self.http]:
            server.shutdown()
            server.server_close()
