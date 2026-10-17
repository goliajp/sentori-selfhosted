import { check } from './check.mjs';
let input='';
for await(const chunk of process.stdin){input+=chunk;if(input.length>1024*1024)throw new Error('job input too large');}
const {job,profiles}=JSON.parse(input);
const result=await check(job,profiles);
console.log(JSON.stringify(result));
