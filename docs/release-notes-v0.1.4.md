# PR2PO v0.1.4

## Sửa lỗi màn hình trắng khi chạy bản cài đặt

- Loại bỏ cấu hình tách riêng chunk AG Grid React gây lỗi runtime trong bản production (`TypeError: t is not a function`).
- Giữ đường dẫn tương đối cho tài nguyên frontend nhúng trong Tauri.
- Kiểm tra lại production build trong browser và app Tauri đã cài: giao diện tải đầy đủ.

## Tệp phát hành

- Bộ cài Windows NSIS (`.exe`) và MSI (`.msi`).
- Bản chạy portable và chữ ký updater Tauri cho từng bộ cài.
- `SHA256SUMS.txt` để kiểm tra tính toàn vẹn của các tệp đính kèm.
