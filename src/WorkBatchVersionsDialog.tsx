import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FileClock, X } from "lucide-react";

type WorkBatchVersion = { id:number; version:number; createdAt:string; actor:string; reason:string; allocationCount:number; scopeCount:number };

type Props = { workBatchId:number; onClose:()=>void; onRestored:()=>void; onError:(message:string)=>void };

function formatTimestamp(value:string){
  const normalized=value.includes("T")?value:value.replace(" ","T");
  const date=new Date(/[zZ]|[+-]\d\d:\d\d$/.test(normalized)?normalized:`${normalized}Z`);
  return Number.isNaN(date.getTime())?value:date.toLocaleString("vi-VN",{dateStyle:"short",timeStyle:"short"});
}

export function WorkBatchVersionsDialog({workBatchId,onClose,onRestored,onError}:Props){
  const [versions,setVersions]=useState<WorkBatchVersion[]>([]);
  const [loading,setLoading]=useState(true);
  const [restoring,setRestoring]=useState(false);

  useEffect(()=>{
    let active=true;
    void invoke<WorkBatchVersion[]>("get_work_batch_versions",{workBatchId})
      .then(rows=>{if(active)setVersions(rows)})
      .catch(error=>{if(active)onError(String(error))})
      .finally(()=>{if(active)setLoading(false)});
    return()=>{active=false};
  },[workBatchId,onError]);

  async function restore(version:WorkBatchVersion){
    if(restoring)return;
    if(!window.confirm(`Khôi phục phiên bản ${version.version}? Phân bổ hiện tại sẽ được lưu thành phiên bản mới và PO nháp cần tạo lại.`))return;
    setRestoring(true);
    try{await invoke("restore_work_batch_version",{workBatchId,versionId:version.id});onRestored()}
    catch(error){onError(String(error))}
    finally{setRestoring(false)}
  }

  return <div className="scrim" onMouseDown={event=>{if(event.target===event.currentTarget)onClose()}}>
    <section className="dialog batch-changes-dialog" role="dialog" aria-modal="true" aria-labelledby="batch-version-title">
      <div className="dialog-title"><div><span className="help-kicker"><FileClock size={13}/> NHẬT KÝ ĐỢT XỬ LÝ</span><h2 id="batch-version-title">Lịch sử phiên bản</h2><p>Mỗi lần lưu tạo một mốc có thể khôi phục. Khi khôi phục, PO nháp cũ được gỡ để tránh dùng nhầm.</p></div><button className="icon-button" aria-label="Đóng lịch sử phiên bản" onClick={onClose}><X size={17}/></button></div>
      {loading?<div className="po-review-loading" role="status">Đang tải lịch sử…</div>:<div className="batch-change-list">{versions.map(version=><article key={version.id}><span className="change-updated">V{version.version}</span><div><strong>{version.reason}</strong><small>{formatTimestamp(version.createdAt)} · {version.actor||"Người dùng cục bộ"} · {version.scopeCount.toLocaleString("vi-VN")} PR · {version.allocationCount.toLocaleString("vi-VN")} phân bổ</small></div><button className="button" disabled={restoring} onClick={()=>void restore(version)}>{restoring?"Đang khôi phục…":"Khôi phục"}</button></article>)}{!versions.length&&<div className="report-no-rows">Chưa có phiên bản. Bản đầu tiên được tạo sau khi lưu đợt.</div>}</div>}
      <div className="dialog-actions"><button className="button" onClick={onClose}>Đóng</button></div>
    </section>
  </div>;
}
