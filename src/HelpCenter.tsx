import { useState, type ReactNode } from "react";
import { Archive, BarChart3, Boxes, CircleHelp, Database, FileInput, ListChecks, Scale, ShieldCheck, ShoppingCart, Workflow, X } from "lucide-react";
import { HelpIllustration } from "./HelpIllustration";
import "./HelpCenter.css";

type SectionId = "start" | "data" | "pr" | "compare" | "po" | "mapping" | "report" | "system";

const sections = [
  { id: "start", label: "Bắt đầu", icon: Workflow, title: "Quy trình từ PR đến PO", description: "Đi theo thứ tự này cho mỗi đợt mua hàng.", image: 1 },
  { id: "data", label: "Dữ liệu", icon: FileInput, title: "Nhập và quản lý dữ liệu", description: "Đưa PR, lịch sử mua và báo giá vào kho dữ liệu cục bộ.", image: 6 },
  { id: "pr", label: "Xử lý PR", icon: ListChecks, title: "Lọc và chọn PR", description: "Tìm đúng phạm vi cần mua trước khi mở so sánh.", image: 2 },
  { id: "compare", label: "So sánh giá", icon: Scale, title: "Chọn NCC và phân bổ số lượng", description: "Duyệt gợi ý, chọn tay hoặc chia một dòng cho nhiều NCC.", image: 3 },
  { id: "po", label: "Đơn mua", icon: ShoppingCart, title: "Lưu đợt, rà soát và xuất PO", description: "Kiểm tra phần thiếu trước khi tạo workbook báo cáo.", image: 4 },
  { id: "mapping", label: "Mapping dự án", icon: Boxes, title: "Đối chiếu PO, sản xuất và kho", description: "Ghép đúng cột để biết hàng đã đặt, đã về và kho còn cần bắn code.", image: 5 },
  { id: "report", label: "Báo cáo", icon: BarChart3, title: "Đọc dashboard và cảnh báo", description: "Tập trung vào các dòng thiếu, lệch hoặc cần người mua quyết định.", image: 7 },
  { id: "system", label: "Sao lưu & cập nhật", icon: ShieldCheck, title: "Bảo toàn dữ liệu và cập nhật app", description: "Sao lưu trên máy, kiểm tra bản mới và khôi phục phiên bản đã phát hành.", image: 8 },
] as const;

function SectionContent({ section }: { section: SectionId }) {
  const content: Record<SectionId, ReactNode> = {
    start: <>
      <div className="help-flow-list">
        <article><b>01</b><span><strong>Nhập nguồn</strong><small>Đưa PR và báo giá vào <b>Dữ liệu</b>; kiểm tra preview và cột đã ghép.</small></span></article>
        <article><b>02</b><span><strong>Chọn phạm vi</strong><small>Lọc PR theo tháng/dự án hoặc dán danh sách, rồi chọn các dòng cần mua.</small></span></article>
        <article><b>03</b><span><strong>So sánh, phân bổ</strong><small>Duyệt NCC/giá đề xuất, sửa tay hoặc chia số lượng cho nhiều NCC.</small></span></article>
        <article><b>04</b><span><strong>Lưu và xuất</strong><small>Lưu đợt, rà soát thiếu báo giá/phân bổ rồi xuất PO và báo cáo.</small></span></article>
      </div>
      <aside className="help-callout"><CircleHelp size={16}/><span>Phím <kbd>?</kbd> mở Help ở bất cứ màn hình nào. Có thể quay lại đây bằng nút <b>?</b> trên thanh đầu.</span></aside>
    </>,
    data: <>
      <h3>Chọn đúng loại nguồn trước khi chọn file</h3>
      <div className="help-source-cards">
        <article><strong>PR</strong><span>Nhu cầu mua, số PR, mã hàng và số lượng cần.</span></article>
        <article><strong>Lịch sử mua</strong><span>PO cũ, NCC và đơn giá đã mua để tham khảo.</span></article>
        <article><strong>Báo giá NCC</strong><span>Nhập Excel hoặc PDF có chữ; PDF scan ảnh cần OCR ngoài app.</span></article>
      </div>
      <h3>Các bước nhập Excel hoặc báo giá PDF</h3>
      <ol className="help-ordered"><li>Thả file vào vùng chọn hoặc bấm <b>Chọn file</b>; chọn sheet nếu là Excel.</li><li>Trong bảng preview, bấm số hàng đang chứa tiêu đề. Ví dụ tiêu đề ở hàng 6 thì dữ liệu bắt đầu từ hàng 7.</li><li>Kiểm tra các cột tự ghép; đổi từng trường nếu chưa đúng. Trường có dấu <b>*</b> là bắt buộc.</li><li>Bấm <b>Rà soát dữ liệu</b>, xem số dòng mới/thay đổi/trùng/lỗi, rồi mới xác nhận nhập.</li></ol>
      <p className="help-muted">Có thể lưu mẫu ánh xạ để dùng lại. Báo giá có thể gắn mẫu theo NCC; lần sau vẫn cần nhìn lại preview trước khi nhập.</p>
      <h3>Quản lý sau khi nhập</h3>
      <p>Trong <b>Dữ liệu</b>, tìm nguồn theo tên file/NCC, xem nhanh báo giá, gỡ riêng một lần nhập hoặc xóa riêng PR, lịch sử hay báo giá. Sao lưu tạo một gói ZIP để chuyển hoặc phục hồi dữ liệu.</p>
    </>,
    pr: <>
      <h3>Tìm trên toàn bộ dữ liệu</h3>
      <ul className="help-bullets"><li>Lọc theo tháng và dự án; dùng tìm kiếm mã hàng/tên hàng/số PR.</li><li><b>Dán danh sách PR</b> để giới hạn theo nhiều số PR cùng lúc.</li><li>Thêm nhiều điều kiện lọc, chọn AND/OR, lưu bộ lọc để dùng lại.</li><li><b>Chọn tất cả kết quả</b> áp dụng cho toàn bộ dữ liệu đang lọc, kể cả dòng chưa tải trên màn hình.</li><li>Cuộn bảng để tự tải thêm; sắp xếp và lọc áp dụng trên tập kết quả, không chỉ trang đang thấy.</li></ul>
      <p className="help-muted">Sau khi đánh dấu PR, chọn <b>So sánh đã chọn</b>. PR không tìm thấy sẽ không tạo dòng hàng.</p>
    </>,
    compare: <>
      <h3>Chọn NCC theo đề xuất hoặc bằng tay</h3>
      <p>Mỗi dòng chỉ hiện các thẻ báo giá của NCC có mã hàng khớp. Chọn luật <b>Giá thấp nhất</b>, <b>Giá cao nhất</b> hoặc <b>Ưu tiên NCC</b>; bấm thẻ/giá của NCC để chọn thủ công. Giá mua cũ chỉ là dữ liệu tham khảo.</p>
      <h3>Chia số lượng và kiểm tra giá</h3>
      <ul className="help-bullets"><li>Dùng <b>Thêm NCC</b> trên đúng dòng để chia số lượng; một mã có thể đặt từ nhiều NCC.</li><li>Sửa số lượng và đơn giá trực tiếp trên thẻ. Tổng phân bổ không được vượt số lượng cần mua.</li><li>Chọn chip tổng quan của NCC để mở danh sách mã hàng, số lượng và tổng tiền đã chọn.</li><li>Dùng lọc mã chưa có báo giá, điều kiện PR/dự án/mã hàng, lịch sử giá và sắp xếp toàn bộ kết quả.</li><li><b>Mô phỏng phương án mua</b> so sánh giá, ưu tiên, giao nhanh hoặc gộp NCC trước khi áp dụng.</li><li><b>Rà soát mã gần giống</b> chỉ gợi ý; người mua phải duyệt ghép mã.</li></ul>
      <p className="help-muted">Báo giá có thể thêm tiền tệ, tỷ giá, VAT, vận chuyển, MOQ và thời gian giao. Giá quy đổi dùng để so sánh; đơn giá gốc giữ khi phân bổ/xuất.</p>
    </>,
    po: <>
      <h3>Thao tác theo thứ tự</h3>
      <ol className="help-ordered"><li>Ở màn so sánh, bấm <b>Lưu đợt phân bổ</b>.</li><li>Qua <b>Đơn mua</b>, chọn đợt gần đây hoặc bấm <b>Tạo / cập nhật PO</b>.</li><li>Bấm <b>Rà soát &amp; xuất</b>; xem dòng thiếu báo giá, chưa phân bổ hoặc phân bổ thiếu.</li><li>Xác nhận xuất. Workbook hiện có sheet tổng hợp các NCC, sheet riêng từng NCC và báo cáo.</li></ol>
      <div className="help-callout"><Archive size={16}/><span>Lịch sử phiên bản cho phép xem và khôi phục mốc phân bổ trước; sau khôi phục cần tạo PO nháp lại.</span></div>
      <p className="help-muted">Form Asoft chính thức và upload PO lên Asoft chưa được cấu hình vì chưa có mẫu/API.</p>
    </>,
    mapping: <>
      <div className="help-mapping-rule"><b>Quy tắc bắt buộc</b><span>Với mọi nguồn: ghép đúng <b>Mã hàng</b> và <b>Số lượng</b>. Đồng thời phải xác định <b>Dự án</b>; riêng PO mua hàng có thể dùng <b>Số PR</b> để suy ra dự án.</span></div>
      <h3>1. Chọn loại nguồn và ghép cột</h3>
      <div className="help-mapping-table-wrap"><table className="help-mapping-table"><thead><tr><th>Nguồn</th><th>Cột cần ghép</th><th>Ý nghĩa số lượng</th></tr></thead><tbody>
        <tr><td><b>PO mua hàng</b></td><td>Dự án hoặc Số PR · Số PO · Mã hàng · NCC</td><td>Số lượng đặt; thêm Số lượng đã về nếu file có</td></tr>
        <tr><td><b>PO sản xuất XGC</b></td><td>Dự án · Số PO · Mã hàng · Tên hàng</td><td>Số lượng kế hoạch và Số lượng hoàn tất</td></tr>
        <tr><td><b>Bắn code kho</b></td><td>Dự án · Mã hàng; NCC/ngày là tùy chọn</td><td>Số lượng hàng đã tìm và bắn code</td></tr>
      </tbody></table></div>
      <p className="help-muted">Trong hộp thoại, trường <b>Số lượng đặt / BOM</b> là tên dùng chung: map sang SL đặt của PO, SL kế hoạch của XGC, hoặc SL bắn của kho tùy loại nguồn. Trường ngày và ghi chú chỉ ghép khi có cột tương ứng.</p>
      <h3>2. Chọn đúng hàng tiêu đề</h3>
      <ol className="help-ordered"><li>Chọn file, sheet và xem preview.</li><li>Tìm hàng có tên cột thật, rồi bấm vào <b>số hàng ở cột đầu tiên</b>. Không chọn hàng tên báo cáo hoặc dòng ngày tháng.</li><li>App đặt dòng dữ liệu bắt đầu ngay bên dưới. Ví dụ tiêu đề hàng 6 → dữ liệu bắt đầu hàng 7.</li></ol>
      <h3>3. Ghép từng trường với đúng cột</h3>
      <p>Đọc tên tiêu đề ở preview rồi chọn cột tương ứng bên dưới. Ví dụ file giả có tiêu đề hàng 6:</p>
      <div className="help-mapping-example"><code>Dự án ← Cột A</code><code>Số PR ← Cột B</code><code>Số PO ← Cột C</code><code>Mã hàng ← Cột E</code><code>Nhà cung cấp ← Cột G</code><code>Số lượng đặt ← Cột H</code><code>Số lượng đã về ← Cột I</code></div>
      <p className="help-muted">Ví dụ dùng cột và dữ liệu minh họa; file thật có thể sắp cột khác. Luôn ghép theo tiêu đề trong file của bạn.</p>
      <h3>4. Hiểu tổng hợp và cảnh báo kho</h3>
      <div className="help-status-list"><p><b>BOM</b><span>Tổng số lượng của cùng dự án + mã hàng lấy từ PR đang hoạt động.</span></p><p><b>PO / XGC</b><span>Đơn đặt, đã về, kế hoạch sản xuất và hoàn tất được cộng theo dự án + mã hàng. Mã XGC có hậu tố <code>_GC</code> được đối chiếu với mã gốc.</span></p><p><b>Đã bắn code</b><span>Số lượng kho đã bắn khớp BOM.</span></p><p><b>Chưa bắn code</b><span>PO đã về đủ nhưng số lượng kho đã bắn chưa khớp BOM; giữ nguyên trạng thái này để kho tìm hàng và bắn code.</span></p><p><b>Chưa có dữ liệu bắn</b><span>Chưa nhập nguồn bắn code kho. Cảnh báo “Kho cần tìm hàng · bắn code” đếm dòng đã về đủ nhưng số lượng kho chưa khớp.</span></p></div>
      <div className="help-callout"><CircleHelp size={16}/><span>Mapping chỉ dùng nguồn đang hoạt động. Nhập lại cùng file + sheet + ánh xạ thì app nhận ra không đổi; cập nhật file cùng đường dẫn sẽ thay nguồn active cũ bằng snapshot mới.</span></div>
    </>,
    report: <>
      <h3>Bắt đầu từ các thẻ tổng quan</h3>
      <p>Chọn đợt đã lưu để xem tổng giá trị, số dòng đã phân bổ, thiếu báo giá, phân bổ thiếu/vượt, báo giá cũ/quá hạn, giá nhập tay và giá bất thường.</p>
      <ul className="help-bullets"><li>Bấm thẻ hoặc bộ lọc vấn đề để xem các dòng tương ứng.</li><li>So sánh giá đã chọn với giá thấp nhất và giá mua cũ; đọc lý do phân bổ.</li><li>Truy nguồn báo giá theo file, sheet/trang và dòng nếu dữ liệu nguồn có thông tin đó.</li><li>Xem tổng tiền và tỷ trọng theo NCC để rà trước khi báo cáo.</li></ul>
      <p className="help-muted">Ngưỡng lệch giá và số ngày báo giá được coi là cũ có thể điều chỉnh trong màn báo cáo.</p>
    </>,
    system: <>
      <h3>Dữ liệu và bản sao lưu</h3>
      <p>Dữ liệu chuẩn hóa được lưu trong SQLite cục bộ trên máy. Trong <b>Dữ liệu</b>, kiểm tra nguồn đã nhập, file nguồn thiếu, dòng báo giá lặp và thời điểm backup. Tạo gói backup trước khi chuyển máy hoặc phục hồi.</p>
      <h3>Cập nhật và rollback</h3>
      <p>Bấm biểu tượng tải xuống trên thanh đầu để nhập URL HTTPS, kiểm tra bản phát hành, cài bản mới hoặc khôi phục bản khác. App xác minh chữ ký gói cập nhật trước khi cài.</p>
      <div className="help-callout"><Database size={16}/><span>Bản hiện tại chưa đồng bộ CSDL giữa nhiều máy. Hãy dùng backup/restore để chuyển dữ liệu; Pi/server chỉ phục vụ file cập nhật theo cấu hình.</span></div>
      <h3>Phím tắt</h3>
      <div className="help-shortcut-grid"><span><kbd>Ctrl</kbd>+<kbd>F</kbd><small>Tìm trong PR/so sánh</small></span><span><kbd>Ctrl</kbd>+<kbd>S</kbd><small>Lưu đợt phân bổ</small></span><span><kbd>Ctrl</kbd>+<kbd>Z</kbd><small>Hoàn tác phân bổ</small></span><span><kbd>Esc</kbd><small>Đóng cửa sổ</small></span><span><kbd>?</kbd><small>Mở Help</small></span></div>
    </>,
  };
  return content[section];
}

export function HelpCenter({ onClose }: { onClose: () => void }) {
  const [activeId, setActiveId] = useState<SectionId>("mapping");
  const active = sections.find(section => section.id === activeId)!;
  const Icon = active.icon;
  return <div className="scrim help-center-scrim" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}>
    <section className="dialog help-center-dialog" role="dialog" aria-modal="true" aria-labelledby="help-center-title">
      <header className="help-center-header"><div><span className="help-kicker">PR2PO · TRUNG TÂM TRỢ GIÚP</span><h2 id="help-center-title">Hướng dẫn theo từng màn hình</h2><p>Chọn mục bên trái để xem cách thao tác, hình minh họa và dữ liệu mẫu.</p></div><button className="icon-button" aria-label="Đóng hướng dẫn" title="Đóng hướng dẫn" onClick={onClose}><X size={18}/></button></header>
      <div className="help-center-layout">
        <nav className="help-center-nav" role="tablist" aria-label="Các mục hướng dẫn" aria-orientation="vertical">{sections.map(section => {const NavIcon = section.icon;return <button key={section.id} type="button" role="tab" id={`help-tab-${section.id}`} aria-controls="help-panel" aria-selected={activeId===section.id} className={activeId===section.id?"active":""} onClick={()=>setActiveId(section.id)}><NavIcon size={16}/><span>{section.label}</span></button>})}</nav>
        <main className="help-center-content" id="help-panel" role="tabpanel" aria-labelledby={`help-tab-${activeId}`} key={activeId}>
          <div className="help-topic-heading"><div><span className="help-topic-icon"><Icon size={17}/></span><div><h3>{active.title}</h3><p>{active.description}</p></div></div><span className="help-topic-count">{String(sections.findIndex(section=>section.id===activeId)+1).padStart(2,"0")} / {String(sections.length).padStart(2,"0")}</span></div>
          <HelpIllustration step={active.image}/>
          <div className="help-topic-body"><SectionContent section={activeId}/></div>
          <div className="help-center-pagination"><button type="button" className="button" disabled={sections[0].id===activeId} onClick={()=>setActiveId(sections[Math.max(0,sections.findIndex(section=>section.id===activeId)-1)].id)}>Mục trước</button><span>Hình minh họa dùng dữ liệu giả</span><button type="button" className="button" disabled={sections[sections.length-1].id===activeId} onClick={()=>setActiveId(sections[Math.min(sections.length-1,sections.findIndex(section=>section.id===activeId)+1)].id)}>Mục tiếp theo</button></div>
        </main>
      </div>
      <footer className="help-center-footer"><span><kbd>?</kbd> mở Help <i/> <kbd>Esc</kbd> đóng cửa sổ</span><button className="button primary" onClick={onClose}>Đã hiểu</button></footer>
    </section>
  </div>;
}
