import {test} from 'node:test';
import assert from 'node:assert/strict';
import {Store} from './store.ts';
import {reserve} from './reserve.ts';
test('identity isolation across compound keys',()=>{
  const s=new Store(); s.set('a:b','c',9); s.set('a','b:c',4); s.set('c','a:b',7);
  assert.equal(reserve(s,[{sku:'a:b',warehouse:'c',quantity:3}]),true);
  assert.equal(s.get('a:b','c'),6); assert.equal(s.get('a','b:c'),4); assert.equal(s.get('c','a:b'),7);
});
test('duplicates cannot overdraw and failure is atomic',()=>{
  const s=new Store(); s.set('x','w',5); s.set('y','w',10);
  const rows=[{sku:'y',warehouse:'w',quantity:2},{sku:'x',warehouse:'w',quantity:3},{sku:'x',warehouse:'w',quantity:3}];
  const copy=structuredClone(rows);
  assert.equal(reserve(s,rows),false); assert.deepEqual(rows,copy);
  assert.equal(s.get('x','w'),5); assert.equal(s.get('y','w'),10);
  rows[2].quantity=2; assert.equal(reserve(s,rows),true); assert.equal(s.get('x','w'),0); assert.equal(s.get('y','w'),8);
});
test('invalid later lines do not partially write; empty and zero batches work',()=>{
  const s=new Store(); s.set('x','w',5);
  for(const quantity of [-1,0.5,NaN,Infinity]) {
    assert.throws(()=>reserve(s,[{sku:'x',warehouse:'w',quantity:2},{sku:'x',warehouse:'w',quantity}]),RangeError);
    assert.equal(s.get('x','w'),5);
  }
  assert.equal(reserve(s,[]),true); assert.equal(reserve(s,[{sku:'absent',warehouse:'w',quantity:0}]),true);
  assert.equal(reserve(s,[{sku:'absent',warehouse:'w',quantity:1}]),false);
});
