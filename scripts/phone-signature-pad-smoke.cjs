const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const html=fs.readFileSync(path.join(__dirname,'../apps/desktop/assets/phone-signature.html'),'utf8');
const script=html.match(/<script>([\s\S]*?)<\/script>/)[1];
function setup(ok=true){
 const buttons={undo:{},clear:{},send:{}},status={},calls=[];
 const ctx={clearRect(){},beginPath(){},lineTo(){},moveTo(){},stroke(){}};
 const canvas={getContext:()=>ctx,getBoundingClientRect:()=>({left:0,top:0,width:100,height:50}),setPointerCapture(){}};
 const document={querySelector:q=>q==='canvas'?canvas:q==='#status'?status:buttons[q.slice(1)],querySelectorAll:()=>Object.values(buttons)};
 vm.runInNewContext(script,{document,location:{pathname:'/draw/test-token'},fetch:async(url,args)=>{calls.push({url,...args});return {ok};}});
 const point=(x,y)=>({pointerId:1,clientX:x,clientY:y,preventDefault(){}});
 return {buttons,canvas,calls,status,point};
}
(async()=>{
 let s=setup();await s.buttons.send.onclick();assert.equal(s.calls.length,0);
 s.canvas.onpointerdown(s.point(10,20));s.canvas.onpointermove(s.point(90,40));await s.buttons.send.onclick();assert.equal(s.calls.length,0);
 s.canvas.onpointerup(s.point(90,40));await s.buttons.send.onclick();assert.equal(s.calls.length,1);
 assert.equal(s.calls[0].url,'/draw/test-token/submit');assert.deepEqual(JSON.parse(s.calls[0].body),[[[0.1,0.4],[0.9,0.8]]]);
 assert.equal(s.buttons.send.disabled,true);await s.buttons.send.onclick();assert.equal(s.calls.length,1);
 s=setup();s.canvas.onpointerdown(s.point(-10,-20));s.canvas.onpointermove(s.point(200,100));s.canvas.onpointerup(s.point(200,100));s.buttons.undo.onclick();await s.buttons.send.onclick();assert.equal(s.calls.length,0);
 s=setup(false);s.canvas.onpointerdown(s.point(0,0));s.canvas.onpointermove(s.point(100,50));s.canvas.onpointerup(s.point(100,50));await s.buttons.send.onclick();await s.buttons.send.onclick();assert.equal(s.calls.length,2);assert.notEqual(s.buttons.send.disabled,true);
 console.log('PASS mobile pad: empty, pointer lifecycle, normalized payload, undo, single send, failed retry');
})().catch(e=>{console.error(e);process.exitCode=1;});
