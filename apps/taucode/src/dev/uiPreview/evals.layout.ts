// Opt-in browser DOM regression test; jsdom cannot measure label widths.
export async function checkEvalsLayout() {
 const waitFor=async(selector:string)=>{
  for(let i=0;i<400;i++){const node=document.querySelector<HTMLElement>(selector);if(node)return node;await new Promise(resolve=>setTimeout(resolve,25));}
  throw new Error("Missing "+selector);
 };
 const assert=(ok:boolean,message:string)=>{if(!ok)throw new Error(message);};
 const frame=()=>new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve())));
 const labels=()=>{
  for(const node of document.querySelectorAll<HTMLElement>(".evals-page label,.evals-page .evals-chip,.evals-page .evals-cell,.evals-page th button")){
   if(!node.getClientRects().length)continue;
   assert(node.clientWidth>0,"No measurable layout for "+node.textContent);
   assert(node.scrollWidth<=node.clientWidth,"Clipped label: "+node.textContent);
  }
  const page=document.querySelector<HTMLElement>(".evals-page")!;
  assert(page.scrollWidth<=page.clientWidth,"Page overflows horizontally");
 };
 try {
  const form=await waitFor(".evals-new-run");await document.fonts.ready;await frame();
  assert(!form.querySelector("select[multiple]"),"Native multi-select in New run");
  labels();
  const picker=[...form.querySelectorAll<HTMLButtonElement>("button")].find(b=>b.textContent?.includes("Choose models"))!;
  picker.click();const popover=await waitFor(".evals-model-popover");await frame();
  const rect=popover.getBoundingClientRect();
  assert(rect.left>=0&&rect.right<=innerWidth&&rect.top>=0&&rect.bottom<=innerHeight,"Model picker exceeds viewport");
  assert(popover.scrollWidth<=popover.clientWidth,"Model picker labels overflow");
  popover.querySelector<HTMLButtonElement>('[aria-label="Close model picker"]')!.click();
  (await waitFor('[data-eval-run="eval-b"]')).click();
  await waitFor('.evals-summary-card');await frame();labels();
  assert(document.querySelectorAll('.evals-summary-card').length===2,"Expected one summary card per configuration");
  assert(!document.querySelector('table[aria-label="Configuration comparison"]'),"Wide table in Settings");
  [...document.querySelectorAll<HTMLButtonElement>('.evals-page button')].find(b=>b.textContent==='Open results')!.click();
  await waitFor('.evals-full .evals-details');
  const disclosure=document.querySelector('.evals-new-run .settings-group-disclosure')!;
  assert(getComputedStyle(disclosure).justifyContent==='flex-start','New run is centered by toolbar styles');
  const header=document.querySelector('.evals-header')!;
  assert(!getComputedStyle(header).backgroundColor.startsWith('rgba('),'Sticky header is translucent');
  [...document.querySelectorAll<HTMLElement>('.evals-page summary')].find(b=>b.textContent==='Detailed metrics')!.click();
  await waitFor('table[aria-label="Configuration comparison"]');await frame();labels();
  for(const node of document.querySelectorAll('table[aria-label="Configuration comparison"] th,table[aria-label="Configuration comparison"] th button'))assert(getComputedStyle(node).whiteSpace==='nowrap','Wrapped table header');
  await waitFor('table[aria-label="Task by configuration"]');
  [...document.querySelectorAll<HTMLElement>('.evals-page summary')].find(b=>b.textContent==='Compare with a previous run')!.click();
  const select=document.querySelector<HTMLSelectElement>(".evals-compare select")!;
  select.value="eval-a";select.dispatchEvent(new Event("change",{bubbles:true}));
  await waitFor('table[aria-label="A/B deltas"]');await frame();labels();
  assert(!!document.querySelector(".evals-regression"),"Missing A/B regression");
  (await waitFor('[data-eval-run="eval-live"]')).click();
  await waitFor(".evals-cell.running");await frame();labels();
  document.documentElement.dataset.evalsLayout="passed";
 } catch(error) {
  document.documentElement.dataset.evalsLayout="failed: "+String(error);throw error;
 }
}

