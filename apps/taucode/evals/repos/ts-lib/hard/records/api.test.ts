import {test} from 'node:test';
import assert from 'node:assert/strict';
import {parseRecords,parseConfig} from './parser.ts';
test('existing public API and errors remain stable',()=>{
  const input=' # comment\r\n a = x=y \r\n\n a=last\n__proto__=safe\n';
  assert.deepEqual(parseRecords(input),[{key:'a',value:' x=y '},{key:'a',value:'last'},{key:'__proto__',value:'safe'}]);
  assert.deepEqual(parseConfig(input),JSON.parse('{"a":"last","__proto__":"safe"}'));
  assert.deepEqual(parseRecords(''),[]);
  for(const text of ['# ignore\n\ninvalid','x=y\n# comment\n =empty']) {
    for(const parse of [parseRecords,parseConfig]) assert.throws(()=>parse(text),{name:'SyntaxError',message:'Invalid record at line 3'});
  }
});
