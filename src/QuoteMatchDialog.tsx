import { useState } from "react";
import { Check, X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

export type QuoteMatchSuggestion={prLineId:number;prCode:string;prName:string;quoteCode:string;quoteName:string;supplier:string;unitPrice:number;quoteLineId:number;confidence:number;reason:string};
type Props={suggestions:QuoteMatchSuggestion[];loading:boolean;onClose:()=>void;onAccepted:(suggestion:QuoteMatchSuggestion)=>Promise<void>;onError:(message:string)=>void};
const money=new Intl.NumberFormat("vi-VN");

export function QuoteMatchDialog({suggestions,loading,onClose,onAccepted,onError}:Props){
  const [busy,setBusy]=useState<number|null>(null);
  async function accept(item:QuoteMatchSuggestion){if(busy!==null)return;setBusy(item.quoteLineId);try{await invoke("accept_quote_code_match",{prLineId:item.prLineId,quoteLineId:item.quoteLineId,confidence:item.confidence});await onAccepted(item)}catch(error){onError(String(error))}finally{setBusy(null)}}
  return <div className="scrim" onMouseDown={event=>{if(event.target===event.currentTarget)onClose()}}><section className="dialog quote-match-dialog" role="dialog" aria-modal="true" aria-labelledby="quote-match-title">
    <div className="dialog-title"><div><span className="help-kicker">ĐỀ XUẤT CÓ DUYỆT</span><h2 id="quote-match-title">Rà soát mã báo giá gần giống</h2><p>Phần trăm là độ giống ký tự, không phải xác suất đúng. Chỉ lưu liên kết sau khi bạn duyệt; giá sẽ hiện trong bảng so sánh.</p></div><button className="icon-button" aria-label="Đóng" onClick={onClose}><X size={17}/></button></div>
    {loading?<div className="po-review-loading" role="status">Đang tìm mã tương tự…</div>:suggestions.length?<div className="quote-match-list">{suggestions.map(item=><article key={`${item.prLineId}-${item.quoteLineId}`}><div className="quote-match-pair"><small>PR</small><strong>{item.prCode}</strong><span>{item.prName}</span></div><div className="quote-match-pair"><small>BÁO GIÁ · {item.supplier}</small><strong>{item.quoteCode}</strong><span>{item.quoteName||item.reason}</span></div><div className="quote-match-confidence"><b>{Math.round(item.confidence*100)}% giống</b><span>{money.format(item.unitPrice)} ₫</span><button className="button primary" disabled={busy!==null} onClick={()=>void accept(item)}><Check size={14}/>{busy===item.quoteLineId?"Đang lưu…":"Duyệt ghép"}</button></div></article>)}</div>:<div className="report-no-rows">Không tìm thấy mã gần giống trong báo giá mới nhất của các NCC.</div>}
    <div className="dialog-actions"><button className="button" onClick={onClose}>Đóng</button></div>
  </section></div>;
}
