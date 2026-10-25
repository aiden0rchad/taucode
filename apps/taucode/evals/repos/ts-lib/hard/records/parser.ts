export function parseRecords(text:string):{key:string;value:string}[] {
  return text.split('\n').flatMap((raw,i)=>{
    const line=raw.replace(/\r$/,'');
    if(!line.trim()||line.trimStart().startsWith('#')) return [];
    const at=line.indexOf('=');
    if(at<1||!line.slice(0,at).trim()) throw new SyntaxError(`Invalid record at line ${i+1}`);
    return [{key:line.slice(0,at).trim(),value:line.slice(at+1)}];
  });
}
export function parseConfig(text:string):Record<string,string> {
  return Object.fromEntries(parseRecords(text).map(row=>[row.key,row.value]));
}
