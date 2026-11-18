# PR2PO v0.1.6

## Cập nhật mới so với bản OTA v0.1.5

- Báo cáo tổng hợp gọn hơn: 7 thẻ chỉ số nằm trên một hàng ở màn hình rộng; cửa sổ hẹp có thể cuộn ngang dải thẻ để giữ vùng bảng.
- Thêm nút **Aa** để chọn cỡ giao diện 90%, 100%, 110% hoặc 120%. Lựa chọn được lưu trên máy; cỡ hiển thị tự giới hạn theo chiều rộng cửa sổ để tránh tràn thanh điều hướng.
- Tăng độ tương phản và khả năng đọc của chỉ số, trạng thái và nội dung bảng báo cáo.
- Ổn định truy vấn báo cáo thay đổi PR bằng cách đặt tên rõ ràng cho các trường kết quả.

## Tính năng đã có trong mã nguồn của bản phát hành này

> Phần mapping dự án, hướng dẫn theo màn hình và luồng nhập PO sản xuất XGC đã được đưa lên updater từ v0.1.5; lần này được đồng bộ vào mã nguồn và GitHub Release.

- Mapping hàng dự án giữa PR/BOM, PO mua hàng, PO sản xuất XGC và file bắn code kho; người dùng chọn sheet, dòng dữ liệu, ánh xạ cột và lưu mẫu ánh xạ.
- Tổng quan tiến độ đặt hàng, số lượng đã về/đã sản xuất/đã bắn code, tìm kiếm và lọc danh sách; cảnh báo kho cần tìm hàng và bắn code.
- Trung tâm trợ giúp theo từng màn hình, có hình minh họa và dữ liệu mẫu.
- Sửa luồng nhập nhiều nguồn mapping, rà soát file PO sản xuất XGC và bảo toàn liên kết dữ liệu khi xóa PR.

## Gói phát hành

- Windows NSIS: `PR2PO_0.1.6_x64-setup.exe` và chữ ký Tauri `.sig`.
- Windows MSI: `PR2PO_0.1.6_x64_en-US.msi` và chữ ký Tauri `.sig`.
- Bản portable: `pr2po-v0.1.6-portable.exe`.
- `SHA256SUMS.txt` chứa checksum SHA-256 của các gói.

Chữ ký `.sig` dùng cho updater Tauri, không phải chữ ký Authenticode của Windows.
