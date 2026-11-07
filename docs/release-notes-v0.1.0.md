# PR2PO v0.1.0

Bản phát hành đầu tiên để xử lý quy trình PR → so sánh báo giá → phân bổ nhà cung cấp → xuất dữ liệu PO. So với bản thử nghiệm trước, phiên bản này hoàn thiện luồng làm việc và bổ sung công cụ theo dõi sau khi phân bổ.

## Điểm mới

- Dashboard báo cáo theo đợt mua hàng: tổng hợp dòng PR, mã chưa có báo giá, vấn đề phân bổ, báo giá hết hạn/cũ, giá nhập tay và giá bất thường.
- Giải thích lựa chọn nhà cung cấp và giá: đối chiếu với giá thấp nhất, lịch sử mua và nguồn báo giá khi dữ liệu nguồn có sẵn.
- Lọc và theo dõi các vấn đề trên toàn bộ đợt, kèm danh sách chi tiết phân trang và thống kê chi tiêu theo nhà cung cấp.
- Nhập PR, lịch sử mua và báo giá Excel; hỗ trợ bóc tách bảng báo giá PDF nhiều trang để người dùng rà soát và ánh xạ cột.
- So sánh, phân bổ số lượng cho nhiều nhà cung cấp, lưu đợt làm việc và xuất workbook PO cùng sheet tổng hợp báo giá.
- Dữ liệu được xử lý và lưu trong SQLite cục bộ; không cần máy chủ để chạy bản này.

## Tệp tải xuống

- `pr2po-v0.1.0.exe`: bộ cài đặt Windows x64 (NSIS).
- `pr2po-v0.1.0.msi`: bộ cài đặt Windows x64 (MSI).
- `pr2po-v0.1.0-portable.exe`: ứng dụng chạy trực tiếp, không qua bộ cài. WebView2 Runtime cần có trên Windows.

## Giới hạn đã biết

- Tự động tải PO lên Asoft chưa được triển khai vì chưa có biểu mẫu và thông tin tích hợp.
- Báo cáo chỉ truy xuất được nguồn/lý do đã được ghi nhận từ phiên bản này trở đi; bản ghi cũ có thể thiếu sheet/trang hoặc lý do chọn.
- Ứng dụng hiện lưu dữ liệu trên máy đang chạy; đồng bộ nhiều máy chưa nằm trong bản phát hành này.
