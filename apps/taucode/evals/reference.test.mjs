import assert from 'node:assert/strict';
import {test} from 'node:test';
import {readFileSync, readdirSync, mkdtempSync, cpSync, rmSync, existsSync} from 'node:fs';
import {join, dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {tmpdir} from 'node:os';
import {spawnSync} from 'node:child_process';
const root=dirname(fileURLToPath(import.meta.url));
const schema=JSON.parse(readFileSync(join(root,'task.schema.json')));
function validate(value,s) {
  if(s.enum) assert.ok(s.enum.includes(value));
  if(s.type==='object') {
    assert.equal(typeof value,'object'); assert.ok(value && !Array.isArray(value));
    for(const k of s.required??[]) assert.ok(k in value,k);
    for(const [k,v] of Object.entries(value)) { assert.ok(s.properties[k],k); validate(v,s.properties[k]); }
  }
  if(s.type==='array') { assert.ok(Array.isArray(value)); value.forEach(v=>validate(v,s.items)); }
  if(s.type==='string') { assert.equal(typeof value,'string'); if(s.pattern) assert.match(value,new RegExp(s.pattern)); if(s.minLength) assert.ok(value.length>=s.minLength); }
  if(s.type==='integer') { assert.ok(Number.isInteger(value)); if(s.minimum) assert.ok(value>=s.minimum); if(s.maximum) assert.ok(value<=s.maximum); }
}
const catalog=JSON.parse(readFileSync(join(root,'catalog.json')));
for(const file of readdirSync(join(root,'tasks')).sort()) {
 const task=JSON.parse(readFileSync(join(root,'tasks',file)));
 test(task.id,()=>{
  validate(task,schema);
  assert.deepEqual(task,catalog.tasks.find(t=>t.id===task.id));
  const dir=mkdtempSync(join(tmpdir(),'tau-eval-'));
  try {
   cpSync(join(root,'repos',task.repo),dir,{recursive:true});
   const env={PATH:process.env.PATH,HOME:dir,TMPDIR:dir,PYTHONDONTWRITEBYTECODE:'1'};
   const shell=command=>spawnSync('/bin/sh',['-c',command],{cwd:dir,env,timeout:10000,encoding:'utf8'});
   for(const setup of task.setup) assert.equal(shell(setup).status,0);
   const check=()=>shell(task.check.command).status===task.check.expectExit && task.extraChecks.every(c=>existsSync(join(dir,c.file)) && readFileSync(join(dir,c.file),'utf8').includes(c.contains)) && task.answerContains.every(token=>existsSync(join(dir,'.answer')) && readFileSync(join(dir,'.answer'),'utf8').includes(token));
   assert.equal(check(),false,'base must fail');
   const patch=spawnSync('git',['apply',join(root,'solutions',task.id+'.patch')],{cwd:dir,env,encoding:'utf8'});
   assert.equal(patch.status,0,patch.stderr);
   assert.equal(check(),true,'reference must pass');
  } finally { rmSync(dir,{recursive:true,force:true}); }
 });
}
test('suite distribution',()=>{
 const counts=Object.fromEntries(['question','bugfix','feature','refactor','browser','computerUse'].map(k=>[k,catalog.tasks.filter(t=>t.category===k).length]));
 assert.deepEqual(counts,{question:6,bugfix:10,feature:7,refactor:3,browser:1,computerUse:1});
 const smoke=catalog.tasks.filter(t=>t.tags.includes('smoke'));
 assert.equal(smoke.length,8);
 assert.equal(smoke.filter(t=>t.repo==='ts-lib'&&t.category==='bugfix').length,2);
 assert.equal(smoke.filter(t=>t.repo==='ts-lib'&&t.category==='feature'&&!t.tags.includes('hard')).length,1);
 for(const category of ['bugfix','feature']) assert.equal(smoke.filter(t=>t.repo==='py-cli'&&t.category===category).length,1);
 for(const category of ['question','refactor']) assert.equal(smoke.filter(t=>t.category===category).length,1);
 assert.equal(catalog.tasks.filter(t=>t.tags.includes('hard')).length,4);
 assert.equal(smoke.filter(t=>t.tags.includes('hard')).length,2);
 assert.ok(smoke.some(t=>t.id==='ts-feature-discount'));
});

