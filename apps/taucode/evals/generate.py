"""Regenerate deterministic offline fixtures, tasks, patches and embedded catalog."""
import difflib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parent
def write(path, text):
    p = ROOT / path
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text)
js = [
 ('range','n','Array.from({length:n+1},(_,i)=>i)','Array.from({length:n},(_,i)=>i)','assert.deepEqual(lib.range(3),[0,1,2]); assert.deepEqual(lib.range(0),[])','bugfix','Fix range(n): it returns n+1 items.'),
 ('sum','xs','xs.reduce((a,b)=>a+b,1)','xs.reduce((a,b)=>a+b,0)','assert.equal(lib.sum([]),0); assert.equal(lib.sum([2,3]),5)','bugfix','Fix sum, including empty input.'),
 ('unique','xs','xs','[...new Set(xs)]','assert.deepEqual(lib.unique([2,1,2]),[2,1])','bugfix','Fix unique to remove duplicates preserving order.'),
 ('clamp','n,lo,hi','Math.max(lo,Math.max(hi,n))','Math.max(lo,Math.min(hi,n))','assert.equal(lib.clamp(4,0,9),4); assert.equal(lib.clamp(12,0,9),9)','bugfix','Fix clamp to enforce both bounds.'),
 ('median','xs','xs[0]','[...xs].sort((a,b)=>a-b)[Math.floor(xs.length/2)]','const xs=[9,1,5]; assert.equal(lib.median(xs),5); assert.deepEqual(xs,[9,1,5]); assert.equal(lib.median([7,3,1]),3)','bugfix','Fix median of odd nonempty arrays without mutating input.'),
 ('slug','s','s.toLowerCase()',r"s.trim().toLowerCase().replace(/\s+/g,'-')","assert.equal(lib.slug(' Hello  World '),'hello-world')",'feature','Implement slug: trim, lowercase, collapse whitespace to hyphens.'),
 ('chunk','xs,n','[]','Array.from({length:Math.ceil(xs.length/n)},(_,i)=>xs.slice(i*n,(i+1)*n))','assert.deepEqual(lib.chunk([1,2,3],2),[[1,2],[3]]); assert.deepEqual(lib.chunk([],2),[])','feature','Implement chunk(xs,n) for positive integer sizes, retaining partial chunks.'),
 ('count','xs','{}','xs.reduce((out,x)=>(out[x]=(out[x]??0)+1,out),Object.create(null))',"assert.equal(lib.count(['a','b','a']).a,2); assert.equal(lib.count(['__proto__'])['__proto__'],1)",'feature','Implement count, including __proto__ safely.'),
 ('partition','xs,fn','[xs,[]]','[xs.filter(fn),xs.filter(x=>!fn(x))]','assert.deepEqual(lib.partition([1,2,3],x=>x>1),[[2,3],[1]])','feature','Implement partition(xs,fn), preserving order in both groups.'),
 ('even','xs','xs.filter(x=>x%2===0).map(x=>x*2)','xs.reduce((out,x)=>{if(x%2===0)out.push(x*2);return out},[])',r"assert.deepEqual(lib.even([1,2,4]),[4,8]); assert.match(lib.even.toString(),/\.reduce\s*\(/); assert.doesNotMatch(lib.even.toString(),/\.(filter|map)\s*\(/)",'refactor','Refactor even to a single reduce traversal, removing filter and map.'),
]
source = ''.join(f'export const {n} = ({args}) => {base};\n' for n,args,base,*_ in js)
write('repos/ts-lib/src/lib.js',source)
write('repos/ts-lib/package.json','{"type":"module"}\n')
write('repos/ts-lib/test/lib.test.js',"import {test} from 'node:test';\nimport assert from 'node:assert/strict';\nimport * as lib from '../src/lib.js';\n"+''.join(f"test('{n}',()=>{{{check}}});\n" for n,_,_,_,check,*_ in js))
py = [
 ('parse','text','return text.split(",")','return [x.strip() for x in text.split(",") if x.strip()]',"self.assertEqual(cli.parse(' a,, b '),['a','b'])",'bugfix','Fix parse to trim comma-separated values and omit empty entries.'),
 ('average','xs','return sum(xs) / len(xs)','return sum(xs) / len(xs) if xs else 0','self.assertEqual(cli.average([]),0); self.assertEqual(cli.average([2,4]),3)','bugfix','Fix average to return zero on empty input.'),
 ('truth','text','return bool(text)',"return text.lower() in ('true','yes','1')","self.assertFalse(cli.truth('false')); self.assertTrue(cli.truth('YES'))",'bugfix','Fix truth: only true, yes and 1 (case insensitive) are true.'),
 ('extension','text','return ""','return Path(text).suffix.lower()',"self.assertEqual(cli.extension('a.TXT'),'.txt'); self.assertEqual(cli.extension('a'),'')",'feature','Implement extension, returning lowercase final extension or empty string.'),
 ('frequency','xs','return {}','return dict(Counter(xs))',"self.assertEqual(cli.frequency(['a','b','a']),{'a':2,'b':1})",'feature','Implement frequency using the standard library.'),
 ('total','xs','return sum([x for x in xs])','return sum(xs)','self.assertEqual(cli.total([2,3]),5); self.assertNotIn("ListComp", ast.dump(ast.parse(inspect.getsource(cli.total))))','refactor','Refactor total to sum the iterable directly without a list comprehension.'),
]
pysource = 'from pathlib import Path\nfrom collections import Counter\n\n'+''.join(f'def {n}({args}):\n    {base}\n\n' for n,args,base,*_ in py)+"if __name__ == '__main__':\n    import sys\n    print(parse(sys.argv[1]))\n"
write('repos/py-cli/cli.py',pysource)
write('repos/py-cli/test_cli.py','import unittest\nimport ast\nimport inspect\nimport cli\n\nclass Tests(unittest.TestCase):\n'+''.join(f'    def test_{n}(self):\n        {check}\n' for n,_,_,_,check,*_ in py))
write('repos/web-page/index.html','<!doctype html><html lang="en"><title>Inventory</title><h1>Inventory</h1><p id="value"></p><script src="app.js"></script></html>\n')
write('repos/web-page/app.js',"document.querySelector('#value').textContent = 'Inventory: ' + (31 * 7);\n")
tasks=[]
def add(id,repo,category,prompt,command,filename,before,after,extra=None,answer=None,requires=None):
    t=dict(id=id,repo=repo,category=category,tags=['smoke'] if len(tasks)<8 else [],prompt=prompt,setup=[],check=dict(command=command,expectExit=0),extraChecks=extra or [],answerContains=answer or [],requires=requires or [],timeoutSec=180,maxTurns=3)
    tasks.append(t)
    write(f'tasks/{id}.json',json.dumps(t,indent=2)+'\n')
    patch=''.join(difflib.unified_diff(before.splitlines(True),after.splitlines(True),fromfile='a/'+filename if before else '/dev/null',tofile='b/'+filename))
    write(f'solutions/{id}.patch',patch)
for repo,rows,src,file in [('ts-lib',js,source,'src/lib.js'),('py-cli',py,pysource,'cli.py')]:
    for n,args,base,solution,_,category,prompt in rows:
        command=f'node --test --test-name-pattern="^{n}$"' if repo=='ts-lib' else f'python3 -m unittest test_cli.Tests.test_{n}'
        extra=[]
        old = f'export const {n} = ({args}) => {base};' if repo=='ts-lib' else f'def {n}({args}):\n    {base}'
        new = f"export const {n} = ({args}) => {solution};" if repo=="ts-lib" else f"def {n}({args}):\n    {solution}"
        add(('ts' if repo=='ts-lib' else 'py')+'-'+category+'-'+n,repo,category,prompt+' Run '+command+'.',command,file,src,src.replace(old,new),extra)
for id,repo,prompt,answer in [
 ('q-range','ts-lib','Which file exports range, and what is its current length for n=3?','src/lib.js 4'),
 ('q-clamp','ts-lib','Which Math method does clamp currently call?','Math.max'),
 ('q-tests','ts-lib','Which built-in modules do the tests import?','node:test node:assert/strict'),
 ('q-python','py-cli','Which standard library classes are imported by cli.py?','Path Counter'),
 ('q-entry','py-cli','Which function does the CLI call with sys.argv[1]?','parse'),
 ('q-frequency','py-cli','What does frequency currently return?','{}'),
]: add(id,repo,'question',prompt,'true','.answer','',answer+'\n',answer=answer.split())
for id,require,prompt in [('browser-inventory','browser','Use the browser'),('computer-inventory','computerUse','Use computer use')]:
    add(id,'web-page',require,prompt+' to open {{url}} and report the rendered Inventory number.','true','.answer','','217\n',answer=['217'],requires=[require])
catalog={'tasks':tasks,'repos':{}}
for repo in ['ts-lib','py-cli','web-page']:
    catalog['repos'][repo]={str(p.relative_to(ROOT/'repos'/repo)):p.read_text() for p in sorted((ROOT/'repos'/repo).rglob('*')) if p.is_file() and '__pycache__' not in p.parts}
write('catalog.json',json.dumps(catalog,indent=2)+'\n')

