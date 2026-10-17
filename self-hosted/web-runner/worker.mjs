import { readProfiles, authState } from './check.mjs';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
const base = new URL(process.env.SENTORI_WEB_RUNNER_API_URL || 'http://127.0.0.1:8080');
const token = process.env.SENTORI_WEB_RUNNER_TOKEN;
if (!token || token.length < 32 || !['http:','https:'].includes(base.protocol) || base.username || base.password) throw new Error('Configure the runner API URL and a dedicated token of at least 32 characters');
let stopping = false;
process.on('SIGTERM', () => { stopping = true; }); process.on('SIGINT', () => { stopping = true; });
async function request(path, body = {}) {
  const response = await fetch(new URL(path, base), { method:'POST',headers:{ Authorization:`Bearer ${token}`,'Content-Type':'application/json' },body:JSON.stringify(body),redirect:'error',signal:AbortSignal.timeout(15_000) });
  if (!response.ok) throw new Error(`runner API HTTP ${response.status}`);
  return response.json();
}
async function isolatedCheck(job, profiles) {
  const image=process.env.SENTORI_WEB_RUNNER_IMAGE;
  if(!image) throw new Error('Set SENTORI_WEB_RUNNER_IMAGE to a qualified image');
  const name='sentori-web-'+randomUUID();
  let selected=[];
  // Pass only this app's selected test identity, never the runner API token or other profiles.
  try { const state=authState(profiles,job); if(state) selected=[{projectId:job.projectId,appId:job.appId,name:job.configuration.authProfile,storageState:state}]; } catch { /* The check returns a scoped authentication error. */ }
  const args=['run','--rm','--init','--name',name,'--memory=1g','--cpus=1','--pids-limit=256','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges',
    '--tmpfs=/tmp:rw,nosuid,nodev,size=512m','--network',process.env.SENTORI_WEB_RUNNER_NETWORK || 'bridge',
    '-e','SENTORI_WEB_RUNNER_SANDBOX=off','-e','SENTORI_WEB_RUNNER_LOCATION='+(process.env.SENTORI_WEB_RUNNER_LOCATION || 'unspecified'),'-i',image];
  const launcher=process.env.SENTORI_WEB_RUNNER_LOCK;
  const child=spawn(launcher || 'docker',launcher ? ['heavy','--max-wait','30','docker',...args] : args,{stdio:['pipe','pipe','pipe'],env:{PATH:process.env.PATH,HOME:process.env.HOME}});
  let output='',failed=false;
  const remove=()=>new Promise(resolve=>{const cleanup=spawn('docker',['rm','-f',name],{stdio:'ignore'});cleanup.on('exit',resolve);cleanup.on('error',resolve);});
  const timer=setTimeout(()=>{failed=true;void remove();},270_000);
  child.stdout.on('data',chunk=>{if(failed)return;if(output.length+chunk.length>256*1024){failed=true;void remove();return;}output+=chunk;});
  child.stderr.on('data',()=>{});
  child.stdin.on('error',()=>{});child.stdin.end(JSON.stringify({job,profiles:selected}));
  try {
    const code=await new Promise((resolve,reject)=>{child.on('error',reject);child.on('exit',resolve);});
    if(code!==0 || failed) throw new Error('isolated browser failed');
    return JSON.parse(output);
  } finally {clearTimeout(timer);await remove();}
}
// One active browser per process. The server enforces two globally and two queued/running per project.
while (!stopping) {
  try {
    const { job } = await request('/internal/web/claim');
    if (!job) { await new Promise(resolve => setTimeout(resolve, 5000)); continue; }
    const result = await isolatedCheck(job, await readProfiles(process.env.SENTORI_WEB_RUNNER_AUTH_FILE));
    await request(`/internal/web/runs/${job.id}/complete`, { leaseId:job.leaseId,result });
    console.log(JSON.stringify({ run:job.id,status:result.status,findings:result.findings.length }));
  } catch (error) {
    // Do not print target HTML, credentials, profile contents or browser output.
    console.error(error?.message?.startsWith('runner API HTTP') ? error.message : 'Web runner cycle failed');
    await new Promise(resolve => setTimeout(resolve, 10_000));
  }
}
