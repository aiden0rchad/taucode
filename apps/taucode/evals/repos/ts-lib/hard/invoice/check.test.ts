import {test} from 'node:test';
import assert from 'node:assert/strict';
import {checkout} from './checkout.ts';
import {quote} from './quote.ts';
import * as money from './money.ts';
test('both consumers reconcile proportional rounding and stable ties',()=>{
  for(const [amounts,discount,want] of [[[101,101,101],2,[100,100,101]],[[100,300],101,[75,224]],[[0,5],3,[0,2]],[[3,7],10,[0,0]],[[],0,[]],[[0,0],0,[0,0]]] as [number[],number,number[]][]) {
    const copy=[...amounts]; const lines=amounts.map((cents,i)=>({id:String(i),cents}));
    assert.deepEqual(checkout(amounts,discount),want);
    const q=quote(lines,discount); assert.deepEqual(q.items,lines.map((line,i)=>({...line,cents:want[i]})));
    assert.equal(q.total,money.sum(amounts)-discount); assert.deepEqual(amounts,copy);
  }
});
test('public allocator and validation agree at both call sites',()=>{
  assert.equal(typeof money.allocateDiscount,'function');
  assert.deepEqual(money.allocateDiscount([1,1,1],1),[1,0,0]);
  for(const [xs,d] of [[[1],2],[[-1],0],[[1.5],1],[[1],-1],[[1],0.5],[[0],1],[[NaN],0],[[1],Infinity]] as [number[],number][]) {
    assert.throws(()=>money.allocateDiscount(xs,d),RangeError);
    assert.throws(()=>checkout(xs,d),RangeError);
    assert.throws(()=>quote(xs.map(cents=>({id:'a',cents})),d),RangeError);
  }
});
