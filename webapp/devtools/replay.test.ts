import {test,expect} from 'bun:test';
import {decodeScreenFrames} from '../src/lib/replay-screens';
import {decodeWireframes} from '../src/lib/replay-wireframe';
const lines=(rows:unknown[])=>rows.map(row=>JSON.stringify(row)).join('\n');
test('screen replay keeps recorded moments and dimensions, tolerates a truncated tail and rejects invalid frames',()=>{
  const image={mediaType:'image/png',base64:'eA=='};
  const result=decodeScreenFrames(lines([{...image,t:-1,w:1000,h:500},{...image,t:-5,w:500,h:1000},{...image,t:'bad'},null,{...image,t:0,mediaType:'text/html'},{...image,t:2,w:0,h:-1},{...image,t:3,mediaType:'image/svg+xml'}])+'\n{"t":');
  expect(result.map(frame=>frame.t)).toEqual([-5,-1,2,3]);
  expect(result.slice(0,2).map(frame=>[frame.w,frame.h])).toEqual([[500,1000],[1000,500]]);
  expect(result[2].w).toBeUndefined();expect(result[2].h).toBeUndefined();
});
test('wireframe replay requires a valid keyframe and preserves rotation and absolute timestamps',()=>{
  const node={x:0,y:0,w:500,h:1000,text:'private replaced'};
  const result=decodeWireframes(lines([{ts:100,kind:'delta',added:[],changed:[],removed:[]},{ts:200,kind:'key',width:500,height:1000,nodes:[node]},null,{ts:250,kind:'delta',added:[],changed:[{...node,id:'0,0,500,1000#0'}],removed:[]},{ts:280,kind:'delta'},{ts:300,kind:'key',width:1000,height:500,nodes:[]},{ts:320,kind:'key',width:0,height:500,nodes:[]}])+'\n{');
  expect(result.map(frame=>frame.ts)).toEqual([200,250,300]);
  expect(result[2]).toMatchObject({width:1000,height:500});
});
