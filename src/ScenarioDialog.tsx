import { Check, Clock3, PackageCheck, X } from "lucide-react";
import type { AllocationScenario } from "./allocationScenarios";

type Props = { scenarios:AllocationScenario[]; useTotalCost:boolean; onApply:(scenario:AllocationScenario)=>void; onClose:()=>void };
const money = new Intl.NumberFormat("vi-VN");

export function ScenarioDialog({scenarios,useTotalCost,onApply,onClose}:Props){
  return <div className="dialog-backdrop" role="presentation" onMouseDown={event=>{if(event.target===event.currentTarget)onClose()}}>
    <section className="dialog scenario-dialog" role="dialog" aria-modal="true" aria-labelledby="scenario-title">
      <header className="dialog-header"><div><h2 id="scenario-title">Mô phỏng phương án mua</h2><p>So sánh trên toàn bộ {scenarios[0]?.covered+scenarios[0]?.uncovered||0} dòng PR. Áp dụng sẽ thay các phân bổ hiện tại.</p></div><button className="icon-button" aria-label="Đóng" onClick={onClose}><X size={18}/></button></header>
      <div className="scenario-grid">{scenarios.map(scenario=><article className="scenario-card" key={scenario.id+scenario.name}>
        <div className="scenario-card-title"><h3>{scenario.name}</h3><span>{scenario.rule}</span></div>
        <div className="scenario-stats"><span><PackageCheck size={15}/><b>{scenario.covered}</b> đáp ứng</span><span className={scenario.uncovered?"scenario-warning":""}><b>{scenario.uncovered}</b> thiếu báo giá</span><span><Clock3 size={15}/><b>{scenario.maxLeadDays}</b> ngày tối đa</span></div>
        <div className="scenario-total"><small>{useTotalCost?"Tổng chi phí ước tính":"Tổng tiền báo giá"}</small><strong>{money.format(scenario.estimatedTotal)} ₫</strong></div>
        <button className="button primary" onClick={()=>onApply(scenario)}><Check size={15}/>Dùng phương án</button>
      </article>)}</div>
      <footer className="dialog-actions"><span className="scenario-note">Mô phỏng không tự lưu; rà soát rồi lưu đợt như bình thường.</span><button className="button" onClick={onClose}>Đóng</button></footer>
    </section>
  </div>;
}
