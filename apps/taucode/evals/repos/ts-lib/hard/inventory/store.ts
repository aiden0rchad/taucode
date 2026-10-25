export type Line = {sku:string; warehouse:string; quantity:number};
const key = (sku:string, warehouse:string) => `${sku}:${warehouse}`;
export class Store {
  stock = new Map<string,number>();
  get(sku:string,warehouse:string) { return this.stock.get(key(sku,warehouse)) ?? 0; }
  set(sku:string,warehouse:string,quantity:number) { this.stock.set(key(sku,warehouse),quantity); }
}
