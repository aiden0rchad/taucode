export function checkout(amounts:number[],discount:number) {
  return amounts.map(amount=>amount-discount);
}
