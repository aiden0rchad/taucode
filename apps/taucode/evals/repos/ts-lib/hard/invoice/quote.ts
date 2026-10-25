import {sum} from './money.ts';
export function quote(lines:{id:string; cents:number}[],discount:number) {
  const items=lines.map(line=>({...line,cents:line.cents-discount}));
  return {items,total:sum(items.map(line=>line.cents))};
}
