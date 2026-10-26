import {Store, type Line} from './store.ts';
export function reserve(store:Store, lines:Line[]):boolean {
  for(const line of lines) {
    if(!Number.isInteger(line.quantity) || line.quantity<0) throw new RangeError('quantity');
    if(store.get(line.sku,line.warehouse)<line.quantity) return false;
  }
  for(const line of lines) store.set(line.sku,line.warehouse,store.get(line.sku,line.warehouse)-line.quantity);
  return true;
}
