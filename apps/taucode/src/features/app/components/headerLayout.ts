// Reserve room for the branch/engine chips and gaps before exposing secondary actions.
export function headerActionsOverflow(width: number, actionWidths: number[]) {
  return width < 240 + actionWidths.reduce((sum, action) => sum + action, 0) + Math.max(0, actionWidths.length - 1) * 8;
}
