import {test} from 'node:test';
import assert from 'node:assert/strict';
import * as parser from './parser.ts';
test('lazy consumption, physical lines and one-shot iterators',()=>{
  let consumed=0;
  function* source() { for(const line of ['# skip','a=1','','broken','z=2']) {consumed++;yield line;} }
  assert.equal(typeof parser.parseLines,'function');
  const rows=parser.parseLines(source()); assert.equal(consumed,0);
  assert.deepEqual(rows.next().value,{key:'a',value:'1'}); assert.equal(consumed,2);
  assert.throws(()=>rows.next(),{name:'SyntaxError',message:'Invalid record at line 4'}); assert.equal(consumed,4);
  assert.deepEqual([...parser.parseLines(['a=1','a=2'])],[{key:'a',value:'1'},{key:'a',value:'2'}]);
  for(const fn of [parser.parseRecords,parser.parseConfig]) assert.match(fn.toString(),/parseLines/,'public APIs must use the shared parser');
});
