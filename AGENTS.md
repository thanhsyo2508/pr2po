# Hướng dẫn làm việc với PR2PO

- Đọc `docs/architecture.md` và `docs/ui.md` trước khi thay đổi phạm vi hoặc giao diện.
- Giữ bản đầu là ứng dụng Tauri chạy cục bộ; chỉ thêm server khi có yêu cầu đồng bộ nhiều máy.
- Rust sở hữu đọc file, kiểm tra dữ liệu, tính tiền, phân bổ và SQLite. React sở hữu hiển thị và tương tác; không đưa toàn bộ file Excel vào JavaScript.
- Giữ thay đổi nhỏ theo Ponytail nhưng không cắt kiểm tra ở ranh giới nhập dữ liệu, phép tính tiền, phục hồi dữ liệu và khả năng dùng bàn phím.
- Dữ liệu trong `dataset/` có thể chứa thông tin nghiệp vụ; không đưa vào Git, log, ảnh chụp hay dữ liệu demo công khai.

## CodeGraph

Khi repo đã có `.codegraph/`, dùng `codegraph explore "<câu hỏi hoặc symbol>"` trước khi tìm/đọc mã nguồn để lần theo symbol và đường gọi. Chạy `codegraph sync` sau các thay đổi lớn. Chỉ khởi tạo chỉ mục khi đã có mã nguồn để tránh lập chỉ mục tài liệu và skill thay cho ứng dụng.

