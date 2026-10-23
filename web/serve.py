"""Loopback-only SDK fixture host. Never used by a shipped plugin.

python web/serve.py --fixtures artifacts/rea/unit-selected-sources --output artifacts/rea/wasm-sdk-20261024/browser
"""
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, unquote
import argparse, json, mimetypes, time
ROOT=Path(__file__).resolve().parents[1]
def main():
 p=argparse.ArgumentParser();p.add_argument('--port',type=int,default=8765);p.add_argument('--fixtures',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
 roots={'/example/':ROOT/'web/example','/wasm-sdk/':ROOT/'dist/wasm-sdk'}
 if a.fixtures:roots['/fixtures/']=a.fixtures.resolve()
 class Handler(BaseHTTPRequestHandler):
  def do_GET(self):
   url=unquote(urlparse(self.path).path)
   if url=='/favicon.ico':self.send_response(204);self.end_headers();return
   if url=='/':self.send_response(302);self.send_header('Location','/example/');self.end_headers();return
   for prefix,root in roots.items():
    if url.startswith(prefix):
     path=(root/(url[len(prefix):] or 'index.html')).resolve()
     if not path.is_relative_to(root.resolve()) or not path.is_file():break
     data=path.read_bytes();self.send_response(200);self.send_header('Content-Type','application/wasm' if path.suffix=='.wasm' else mimetypes.guess_type(path)[0] or 'application/octet-stream');self.send_header('Content-Length',str(len(data)));self.send_header('Cache-Control','no-store');self.send_header('Content-Security-Policy',"default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; connect-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'");self.end_headers();self.wfile.write(data);return
   self.send_error(404)
  def do_POST(self):
   length=int(self.headers.get('Content-Length','0'))
   if self.path!='/receipt' or not 0<length<=16*1024*1024:self.send_error(400);return
   data=self.rfile.read(length);receipt=json.loads(data)
   path=a.output/f'receipt-{time.time_ns()}.json';path.write_text(json.dumps(receipt,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');print(path,flush=True)
   self.send_response(200);self.end_headers();self.wfile.write(b'OK')
  def log_message(self,fmt,*args):pass
 print(f'http://127.0.0.1:{a.port}/example/',flush=True)
 ThreadingHTTPServer(('127.0.0.1',a.port),Handler).serve_forever()
if __name__=='__main__':main()
