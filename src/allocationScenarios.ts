export type ScenarioOffer = {
  supplier:string;
  unitPrice:number;
  effectiveUnitPrice:number;
  quoteLineId:number;
  leadTimeDays:number;
  eligible:boolean;
};

export type ScenarioRow = { id:number; code:string; quantity:number; offers:ScenarioOffer[] };
export type ScenarioAllocation = { prLineId:number; supplier:string; quantity:number; unitPrice:number; quoteLineId:number; selectionReason:string };
export type AllocationScenario = { id:string; name:string; rule:string; allocations:ScenarioAllocation[]; covered:number; uncovered:number; estimatedTotal:number; maxLeadDays:number };

type Rule = "lowest"|"priority"|"fastest";

function offerCost(offer:ScenarioOffer,useTotalCost:boolean){return useTotalCost?offer.effectiveUnitPrice:offer.unitPrice}

function compareOffers(a:ScenarioOffer,b:ScenarioOffer,rule:Rule,priorities:Record<string,number>,useTotalCost:boolean){
  const cost=offerCost(a,useTotalCost)-offerCost(b,useTotalCost);
  if(rule==="priority")return (priorities[a.supplier]??100)-(priorities[b.supplier]??100)||a.leadTimeDays-b.leadTimeDays||cost;
  if(rule==="fastest")return a.leadTimeDays-b.leadTimeDays||cost||(priorities[a.supplier]??100)-(priorities[b.supplier]??100);
  return cost||a.leadTimeDays-b.leadTimeDays||(priorities[a.supplier]??100)-(priorities[b.supplier]??100);
}

function summarize(id:string,name:string,rule:string,rows:ScenarioRow[],selected:Map<number,ScenarioOffer>,useTotalCost:boolean):AllocationScenario{
  const allocations:ScenarioAllocation[]=[];
  let estimatedTotal=0;let maxLeadDays=0;
  for(const row of rows){const offer=selected.get(row.id);if(!offer)continue;allocations.push({prLineId:row.id,supplier:offer.supplier,quantity:row.quantity,unitPrice:offer.unitPrice,quoteLineId:offer.quoteLineId,selectionReason:`Mô phỏng: ${rule} · ${offer.leadTimeDays} ngày giao`});estimatedTotal+=row.quantity*offerCost(offer,useTotalCost);maxLeadDays=Math.max(maxLeadDays,offer.leadTimeDays)}
  return{id,name,rule,allocations,covered:allocations.length,uncovered:rows.length-allocations.length,estimatedTotal,maxLeadDays};
}

export function buildAllocationScenarios(rows:ScenarioRow[],priorities:Record<string,number>,useTotalCost:boolean):AllocationScenario[]{
  const scenarios:AllocationScenario[]=[];
  for(const [id,name,rule] of [["lowest","Giá tốt nhất","lowest"],["priority","Ưu tiên NCC","priority"],["fastest","Giao nhanh nhất","fastest"]] as const){
    const selected=new Map<number,ScenarioOffer>();
    for(const row of rows){const eligible=row.offers.filter(offer=>offer.eligible).sort((a,b)=>compareOffers(a,b,rule,priorities,useTotalCost));if(eligible[0])selected.set(row.id,eligible[0])}
    scenarios.push(summarize(id,name,name,rows,selected,useTotalCost));
  }
  const suppliers=[...new Set(rows.flatMap(row=>row.offers.filter(offer=>offer.eligible).map(offer=>offer.supplier)))];
  const oneSupplierCandidates=suppliers.map(supplier=>{
    const selected=new Map<number,ScenarioOffer>();
    for(const row of rows){const offer=row.offers.find(candidate=>candidate.supplier===supplier&&candidate.eligible);if(offer)selected.set(row.id,offer)}
    return{supplier,scenario:summarize("single",`Một NCC · ${supplier}`,"Một nhà cung cấp",rows,selected,useTotalCost)};
  }).sort((a,b)=>b.scenario.covered-a.scenario.covered||a.scenario.estimatedTotal-b.scenario.estimatedTotal||(priorities[a.supplier]??100)-(priorities[b.supplier]??100));
  if(oneSupplierCandidates[0])scenarios.push(oneSupplierCandidates[0].scenario);
  return scenarios;
}
