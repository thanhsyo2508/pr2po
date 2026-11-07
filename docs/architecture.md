# Kiến trúc PR2PO — quyết định ban đầu

Ngày: 2026-11-03. Trạng thái: quyết định cho MVP, cần đo hiệu năng trên máy đích trước khi khóa bộ đọc Excel.

## 1. Bối cảnh và ranh giới

- Một máy Windows chạy app độc lập. SQLite và file nguồn ở máy đó; chưa có API hoặc cơ sở dữ liệu trên Pi.
- Người dùng xử lý theo đợt một hoặc vài tháng, được chọn dự án, PR hoặc dán danh sách số PR.
- File Excel có thể khoảng 200.000 dòng; nguồn export có thể đổi thứ tự, sai tiêu đề hoặc thiếu ID dòng ổn định.
- Một PR và mã hàng có thể lặp nhiều dòng hợp lệ. Phân bổ có thể chia một dòng cho nhiều NCC.
- File `THDH` mẫu không có cột dự án tường minh ở vùng dữ liệu. Việc chọn theo dự án cần quy tắc ánh xạ có kiểm chứng.
- Giá, số lượng và PO đã chốt phải truy được về file và dòng nguồn.

## 2. Lựa chọn công cụ

| Phần | Chọn cho MVP | Lý do và giới hạn |
|---|---|---|
| Desktop | Tauri 2 | Nhẹ, Rust xử lý file/SQLite, giao diện web cục bộ. |
| Giao diện | React + TypeScript + Vite | Phù hợp UI nhiều trạng thái và hệ sinh thái data grid; Tauri hỗ trợ Vite trực tiếp. Không cần Next.js/SSR. |
| Bảng | AG Grid Community | Có ảo hóa, chọn hàng, cố định cột, điều hướng bàn phím, chỉnh độ rộng. Danh sách lớn dùng Infinite Row Model lấy từng khối từ SQLite; nhóm dự án ở bộ lọc/danh sách riêng vì row model miễn phí không nhóm dữ liệu từ nguồn theo cách cần. Excel export của grid thuộc Enterprise; xuất PO bằng Rust. |
| Dialog/form | CSS tokens + Radix Primitives khi cần | Giữ giao diện riêng, tránh thêm cả thư viện thành phần trước khi có màn hình. |
| Trạng thái UI | React state cục bộ | Chưa cần kho trạng thái toàn cục. Grid tự quản lý cache khối hàng. |
| Database | SQLite + `rusqlite` trong Rust | Giao dịch rõ ràng, import theo lô, FTS nếu cần. Không cho frontend SQL trực tiếp. |
| Đọc Excel | `calamine` là ứng viên đầu | Kiểm chứng tốc độ và RAM trên file 200.000 dòng; nếu cách đọc sheet còn giữ quá nhiều trong RAM, dùng API streaming XLSX hoặc parser XML tuần tự cho nguồn lớn. |
| Xuất | CSV chuẩn ở MVP; `rust_xlsxwriter` khi có form PO | Tránh phụ thuộc vào tính năng Excel export trả phí của grid. |
| PDF | PDF.js đọc bảng nhiều trang, hiển thị thành bảng trung gian để người dùng chọn dòng bắt đầu và ánh xạ cột như Excel | Parser hiện nhận diện theo bố cục báo giá mẫu đã thử; PDF scan cần OCR và mẫu khác phải được kiểm chứng trước khi dùng. |
| Log | `tracing` + log file xoay vòng | Chỉ ghi metadata, lỗi, thời gian xử lý; không ghi nội dung báo giá/PR vào log. |
| Cập nhật | Tauri Updater ở giai đoạn triển khai nội bộ | Pi sau này có thể chỉ phục vụ gói cập nhật đã ký và manifest tĩnh. |

**Phương án không chọn lúc này:** TanStack Table + Virtual cho phép kiểm soát markup tối đa nhưng cần tự làm nhiều hành vi grid; Tauri SQL plugin cho frontend làm ranh giới kiểm tra dữ liệu khó quản lý hơn; server trung tâm chưa phục vụ bản standalone.

## 3. Ranh giới module

```text
pr2po/
  src/                         React/TypeScript
    app/                       điều hướng, shell, IPC client
    features/import/           xem trước, ánh xạ, tiến độ, đối chiếu
    features/pr/               chọn đợt, dự án, PR, bảng nhu cầu
    features/quotes/           nhập và so sánh báo giá
    features/allocations/      chia số lượng, xem tổng
    features/po/               PO nháp và báo cáo
    ui/                        tokens và thành phần dùng chung thật sự
  src-tauri/
    src/commands/              API IPC mỏng, đầu vào/đầu ra có kiểu
    src/import/                đọc Excel, mapping, chuẩn hóa, đối chiếu
    src/domain/                luật chọn giá, phân bổ, tính tiền
    src/storage/               SQLite, truy vấn, migration
    src/export/                CSV và bộ chuyển đổi PO
  docs/                        quyết định kiến trúc và UI
```

Đây là ranh giới dự kiến; chỉ tạo thư mục khi có mã nguồn tương ứng. Một nghiệp vụ nằm trong một module Rust, command chỉ gọi nghiệp vụ và chuyển kết quả. React không tính thành tiền bằng số thực JavaScript.

## 4. Dữ liệu cục bộ

- `import_batch`: loại báo cáo, hash file, sheet, khoảng ngày, phiên bản mapping, thời gian nhập, đường dẫn file gốc.
- `source_row`: batch, số dòng để truy vết, hash nội dung, các trường tối thiểu đã chuẩn hóa. Số dòng **không** là ID nghiệp vụ.
- `pr_line`: ID nội bộ của dòng nhu cầu, số PR, dự án, mã hàng, mô tả, đơn vị, số lượng, ngày cần giao, trạng thái đối chiếu.
- `purchase_history`: PO cũ, PR, mã hàng, NCC, số lượng, đơn giá và tham chiếu dòng nguồn.
- `quote_document` và `quote_line`: NCC, số báo giá, điều kiện, giá từng mã và tham chiếu file gốc.
- `work_batch`, `work_batch_scope`, `allocation`, `po_header`, `po_line`: đợt xử lý, snapshot các PR trong phạm vi, chia số lượng và PO nháp. Snapshot phạm vi giữ được các dòng chưa phân bổ để rà soát sau này.
- `recovery_draft`: bản nháp phân bổ đang làm, tự lưu vào SQLite cục bộ để khôi phục sau khi app đóng đột ngột; chỉ xóa sau khi người dùng lưu đợt hoặc bỏ bản nháp.
- `allocation.selection_reason`: lưu nguồn quyết định (luật giá, chọn tay, chia NCC, dán Excel) để trang báo cáo giải trình lựa chọn sau khi mở lại đợt.
- `quote_batch.sheet_name` và `quote_line.source_page`: giữ sheet Excel hoặc trang PDF cùng số dòng/STT để lần theo nguồn báo giá; migration cũ mặc định để trống/0.
- `audit_event`: lần nhập, hoàn tác nguồn và thay đổi đợt phân bổ quan trọng.

Tiền lưu theo đơn vị nguyên nhỏ nhất kèm mã tiền tệ; tính toán không dùng `f32`/`f64`. Giá đã duyệt và dòng PO là snapshot, không thay đổi theo lần nhập sau.

## 5. Import và đối chiếu

1. Mở file lấy danh sách sheet và xem trước một đoạn nhỏ; người dùng xác nhận sheet, dòng dữ liệu, cột và phạm vi export. PDF báo giá cũng hiển thị bảng đã bóc tách và dùng cùng bộ chọn cột.
2. Rà soát toàn bộ dòng trước khi nhập: dòng mới, thay đổi, không đổi, trùng và lỗi. Dòng không hợp lệ được bỏ qua và có ví dụ số dòng để người dùng kiểm tra.
3. Chạy công việc trong Rust, chỉ đọc trường được ánh xạ; chuẩn hóa mã hàng, ngày, số lượng, tiền và ghi trong giao dịch có thể hủy. Hủy sẽ rollback dữ liệu đang nhập.
4. Cùng hash file + cùng mapping thì bỏ qua import. File khác vẫn phải đọc một lượt; chỉ ghi các bản ghi cần thay đổi.
5. Với nguồn không có ID dòng ổn định, giữ bản chụp theo batch/phạm vi. Dòng lặp được báo để rà soát; không tự gắn quyết định mua cũ sang dòng mới khi chưa chắc chắn.
6. Chỉ đánh dấu dòng cũ vắng mặt khi người dùng xác nhận file mới là export đầy đủ cho cùng phạm vi. Export tháng chồng lấn không được tự xóa dữ liệu ngoài phạm vi.

Dự án được lấy từ cột nguồn khi có; nếu không, thử ánh xạ từ số PR hoặc đối chiếu lịch sử. Quy tắc chỉ tự gán khi kết quả duy nhất và đã được xác nhận. Các PR còn lại nằm trong nhóm **Chưa xác định dự án** để người dùng gán thủ công. Với sheet báo giá có vùng định dạng rất rộng, preview và parser chỉ xử lý các cột được ánh xạ, bỏ qua cột trống ở xa.

Với lịch sử 140.000 dòng, lưu các trường cần tra giá và tạo chỉ mục `(item_code, supplier)`; không tải toàn bộ Excel vào UI. Với danh sách PR, lọc ngày/dự án/PR trong SQLite trước khi trả khối hàng cho grid.

`get_work_report` tổng hợp theo snapshot đợt, lịch sử giá và báo giá hiện hành trong Rust. Giao diện nhận số tổng toàn đợt và từng trang chi tiết; ngưỡng lệch giá mặc định 30% và tuổi báo giá mặc định 90 ngày, người dùng có thể chỉnh.

## 6. Quy tắc nghiệp vụ không được vi phạm

- Tổng số lượng đã phân bổ của một dòng PR không vượt số lượng còn cần đặt.
- Báo giá không có giá số hợp lệ, sai đơn vị hoặc hết hiệu lực không tự được chọn.
- Giá cũ là dữ liệu tham chiếu, không thay cho báo giá hiện tại.
- Tính tiền thống nhất tại Rust; tổng PO là tổng các dòng đã làm tròn theo quy tắc được xác nhận.
- PO nháp có thể sửa; PO đã xuất có snapshot và sự kiện chỉnh sửa/điều chỉnh riêng.
- Đợt đã lưu có thể mở lại để sửa; cập nhật phân bổ sẽ làm mới PO nháp và ghi audit event.
- Hoàn tác từng nguồn chỉ gỡ dữ liệu chưa được PO cũ tham chiếu; dữ liệu PR đã tham chiếu được giữ lại.
- Gói sao lưu là ZIP gồm snapshot SQLite nhất quán và các file nguồn còn tồn tại. Phục hồi kiểm tra database, đưa lại file nguồn vào thư mục dữ liệu app và giữ snapshot an toàn trước khi thay database hiện tại.

## 7. Ngân sách hiệu năng và cách đo

Các số sau là **mục tiêu kiểm thử**, chưa phải cam kết tốc độ: xem trước trong 3 giây, import 200.000 dòng trong 30 giây, tìm kiếm/bộ lọc dưới 200 ms trên máy đích. Đo thời gian mở file, parse, chuẩn hóa, ghi DB và dựng grid riêng biệt; đo RAM đỉnh. Chạy với file mẫu và file export lớn thật. Sau phép đo mới quyết định có cần parser XLSX chuyên biệt hay tối ưu truy vấn sâu.

Grid lấy khối khoảng 100–200 dòng, thực hiện lọc/sắp xếp trên SQLite và render số hàng đang nhìn thấy. Lệnh chọn tất cả lưu điều kiện lọc hoặc tập ID trong SQLite; không phụ thuộc vào các hàng grid đã tải.

## 8. Thứ tự triển khai

1. Khởi tạo Tauri, React và SQLite; màn hình chọn PR với dữ liệu mẫu nhỏ.
2. Import `THDH` có preview/mapping, đối chiếu và tra cứu nhanh.
3. Import `THNHT` cho giá lịch sử, nhập báo giá Excel và màn hình so sánh.
4. Phân bổ, PO nháp, CSV và báo cáo.
5. Bóc tách PDF có chữ với preview/mapping, hoàn tác nguồn, lưu/mở lại đợt, sao lưu/phục hồi và hủy import; tiếp tục kiểm thử thêm kiểu PDF, form Asoft sau khi nhận mẫu chuẩn, updater nội bộ khi triển khai. Chỉ thiết kế đồng bộ nhiều máy khi cần.

## Nguồn kỹ thuật đã kiểm tra

- [Tauri 2: tạo dự án](https://v2.tauri.app/start/create-project/) và [cấu hình frontend](https://v2.tauri.app/start/frontend/).
- [AG Grid: các row model](https://www.ag-grid.com/react-data-grid/row-models/) và [Excel export](https://www.ag-grid.com/javascript-data-grid/excel-export/).
- [Calamine: đọc bảng tính bằng Rust](https://github.com/tafia/calamine).
- [SQLite: WAL](https://www.sqlite.org/wal.html).

## Quan sát từ workbook mẫu

- `THDH261030-2.xlsx`: `Sheet1` có vùng `A2:O27727`, gồm tiêu đề báo cáo ở các dòng đầu và header tại dòng 5; `Sheet2` có vùng `A1:M226` với dữ liệu PR ngay từ dòng 1, không có header. Importer phải để người dùng chọn sheet, dòng dữ liệu đầu tiên và cột; không áp mapping mặc định.
- `THNHT1-211.xlsx`: `Sheet1` có 140.193 dòng, header ở dòng 3 và dữ liệu bắt đầu ở dòng 4. Workbook khoảng 9,5 MB nén; thời gian và RAM khi parse/import chưa được đo.
- File so sánh/báo giá có bố cục khác nhau; không dùng parser PR chung để đoán cấu trúc báo giá. Mỗi loại nguồn cần mapping/preview riêng và giữ tham chiếu sheet, dòng nguồn.

