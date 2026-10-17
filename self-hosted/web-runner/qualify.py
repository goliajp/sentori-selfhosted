#!/usr/bin/env python3
"""Disposable real Chromium/Lighthouse checks behind the production destination policy."""
import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile
import time
import uuid

ROOT=pathlib.Path(__file__).resolve().parents[2]
FILES=sorted((ROOT/'self-hosted/web-runner').glob('*.mjs'))+[ROOT/'self-hosted/web-runner/Dockerfile',ROOT/'self-hosted/web-runner/package.json',ROOT/'self-hosted/web-runner/bun.lock',ROOT/'scripts/lib/headless-chrome.mjs']
DIGEST=hashlib.sha256(b''.join(p.read_bytes() for p in FILES)).hexdigest()
IMAGE='sentori-web-check-test:'+DIGEST[:16]

def docker(*args,**kwargs):return subprocess.check_output(['docker',*args],text=True,**kwargs).strip()

FIXTURE='''
import http.server,time
class Handler(http.server.BaseHTTPRequestHandler):
 def do_GET(self):
  path=self.path.split('?')[0]
  if path=='/robots.txt':
   self.send_response(200);self.end_headers();self.wfile.write(b'User-agent: *\\nDisallow: /robots-denied\\nSitemap: http://web.sentori.example/sitemap.xml');return
  if path=='/sitemap.xml':
   self.send_response(200);self.end_headers();self.wfile.write(b'<urlset><url><loc>http://web.sentori.example/healthy</loc></url></urlset>');return
  if path=='/redirect-private':
   self.send_response(302);self.send_header('Location','http://169.254.169.254/');self.end_headers();return
  if path=='/missing.js' or path=='/missing':
   self.send_response(404);self.end_headers();return
  if path=='/private' and 'auth=ok' not in self.headers.get('Cookie',''):
   self.send_response(403);self.end_headers();self.wfile.write(b'<input type="password">');return
  if path=='/slow':time.sleep(1)
  head='<title>Browser fixture</title><meta name="description" content="Deterministic monitoring fixture"><link rel="canonical" href="http://web.sentori.example'+path+'">'
  body='<main id="ready">A deterministic application page with enough text to establish rendered content.</main><button id="button" onclick="document.getElementById(\\\'ready\\\').textContent=\\\'Ready\\\'">Read</button>'
  if path in ['/private','/noindex']:head+='<meta name="robots" content="noindex">'
  if path=='/private':body+='<p>PRIVATE_PROFILE_MARKER_MUST_NOT_APPEAR_IN_RESULTS</p>'
  if path=='/broken':body+='<script src="/missing.js"></script><script>setTimeout(()=>{throw new Error("fixture JS failure")},50)</script>'
  if path=='/blocked':body+='<img src="http://169.254.169.254/latest/meta-data/">'
  if path=='/canonical':head='<title>Fixture</title><link rel="canonical" href="/missing">'
  self.send_response(200);self.send_header('Content-Type','text/html');self.end_headers();self.wfile.write(('<!doctype html><html lang="en"><head>'+head+'</head><body>'+body+'</body></html>').encode())
 def log_message(self,*args):pass
http.server.ThreadingHTTPServer(('0.0.0.0',80),Handler).serve_forever()
'''

def main():
    if '--build' in sys.argv:
        subprocess.run(['docker','build','-f','self-hosted/web-runner/Dockerfile','-t',IMAGE,'.'],cwd=ROOT,check=True)
        print('PASS isolated Web runner image build '+IMAGE);return
    docker('image','inspect',IMAGE)
    prefix='sentori-web-fixture-'+uuid.uuid4().hex[:12]
    network=prefix;fixture=prefix+'-http';containers=[]
    with tempfile.TemporaryDirectory(prefix=prefix) as directory:
        root=pathlib.Path(directory);(root/'server.py').write_text(FIXTURE)
        subnet=f'11.{128+int(uuid.uuid4().hex[:2],16)%100}.{int(uuid.uuid4().hex[:2],16)}.0/24'
        docker('network','create','--internal','--subnet',subnet,network)
        try:
            docker('run','-d','--name',fixture,'--network',network,'--network-alias','web.sentori.example','-v',str(root)+':/fixture:ro','python:3.13-alpine','python','/fixture/server.py');containers.append(fixture)
            for _ in range(50):
                ready=subprocess.run(['docker','exec',fixture,'python','-c',"import urllib.request;urllib.request.urlopen('http://127.0.0.1/healthy').read()"],capture_output=True)
                if ready.returncode==0:break
                time.sleep(.1)
            assert ready.returncode==0
            project=str(uuid.uuid4());app=str(uuid.uuid4())
            def check(path,**extra):
                name=prefix+'-'+uuid.uuid4().hex[:8];containers.append(name)
                config=dict(appId=app,name='Fixture',url='http://web.sentori.example'+path,policy='public',allowedOrigins=[],steps=[],readySelector='#ready',device='desktop',lighthouse=False);config.update(extra)
                profile=dict(projectId=project,appId=app,name='fixture-auth',storageState=dict(cookies=[dict(name='auth',value='ok',domain='web.sentori.example',path='/',expires=-1,httpOnly=True,secure=False,sameSite='Lax')],origins=[]))
                job=dict(id=str(uuid.uuid4()),projectId=project,appId=app,configuration=config)
                args=['run','--rm','--init','--name',name,'--network',network,'--memory=1g','--cpus=1','--pids-limit=256','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--tmpfs=/tmp:rw,nosuid,nodev,size=512m','-e','SENTORI_WEB_RUNNER_SANDBOX=off','-i',IMAGE]
                result=json.loads(docker(*args,input=json.dumps(dict(job=job,profiles=[profile])),timeout=180))
                assert result['source']=='synthetic' and result['coverage']['indexing']=='not-measured',result
                assert 'PRIVATE_PROFILE_MARKER' not in json.dumps(result)
                return result
            healthy=check('/healthy');assert healthy['status']=='passed',healthy
            broken=check('/broken');assert broken['status']=='failed',broken
            assert {'javascript.exception','resource.http'} <= {f['rule'] for f in broken['findings']},broken
            noindex=check('/noindex');assert any(f['rule']=='seo.noindex' for f in noindex['findings']),noindex
            denied=check('/robots-denied');assert any(f['rule']=='seo.robots' for f in denied['findings']),denied
            private=check('/private',policy='private',authProfile='fixture-auth');assert private['status']=='passed',private
            expired=check('/private',policy='private',authProfile='missing');assert expired['status']=='error',expired
            blocked=check('/blocked');assert blocked['coverage']['blockedRequests']>0,blocked
            redirect=check('/redirect-private');assert redirect['status']=='error',redirect
            slow=check('/slow',lighthouse=True,budgets={'lcpMs':1});assert slow['status']=='failed',slow
            assert slow['coverage']['lighthouse']=='measured' and any(m['name']=='TBT' for m in slow['metrics']),slow
            assert not any(m['name']=='INP' for m in slow['metrics']),slow
            print('PASS isolated Chromium/Lighthouse: healthy, JS/resource failure, public/private SEO, scoped auth, redirect/subresource blocks and lab budgets')
        finally:
            for name in reversed(containers):subprocess.run(['docker','rm','-f',name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            subprocess.run(['docker','network','rm',network],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)

if __name__=='__main__':main()
