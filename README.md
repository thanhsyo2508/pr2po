# PR2PO

Ứng dụng desktop xử lý PR, so sánh báo giá, phân bổ nhà cung cấp và tạo dữ liệu PO.

Trạng thái hiện tại: vertical slice đầu tiên đã chạy được trên desktop. Ứng dụng có màn hình chọn PR mẫu, wizard xem trước/ánh xạ cột Excel, lưu batch PR vào SQLite cục bộ và đọc danh sách theo trang 200 dòng. Xem [kiến trúc](docs/architecture.md) và [quy chuẩn giao diện](docs/ui.md).

## Phạm vi bản đầu

1. Nhập file nhu cầu PR từ Excel, xem trước sheet/dòng và ánh xạ cột.
2. Chọn đợt xử lý theo thời gian, dự án, PR hoặc danh sách PR được dán vào.
3. Nhập báo giá Excel, so sánh giá và chia số lượng cho nhiều NCC.
4. Tạo PO nháp và báo cáo trung gian. Form Asoft cần mẫu chuẩn; bóc tách PDF triển khai sau khi kiểm thử đủ các kiểu báo giá.

File trong `dataset/` là dữ liệu tham chiếu tại máy làm việc, không thuộc mã nguồn và được loại khỏi Git.

## Chạy bản phát triển

```powershell
pnpm install
pnpm tauri dev
```

Dữ liệu SQLite được tạo trong thư mục app data của Windows. Mỗi lần import hiện tạo một batch mới; các batch cũ được giữ nguyên. Cơ chế đối chiếu và cập nhật riêng những dòng thay đổi sẽ được triển khai sau khi xác nhận khóa định danh trong export PR. Hiệu năng với workbook lớn chưa được đo trên file đầy đủ.

## Công cụ hỗ trợ khi phát triển

`ui-ux-pro-max` và `ponytail` đã được cài cục bộ trong `.agents/skills/` trên máy này; thư mục skill không đưa vào Git. Quy chuẩn giao diện đã dùng các kết quả phù hợp của chúng và `design-taste-frontend`. CodeGraph CLI đã được khởi tạo; dùng `codegraph explore` trước khi tìm luồng mã và `codegraph sync .` sau khi sửa mã.

