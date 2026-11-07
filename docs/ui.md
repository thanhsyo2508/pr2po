# Quy chuẩn giao diện PR2PO

## Hướng thiết kế

Ứng dụng là bàn làm việc mua hàng dùng nhiều giờ mỗi ngày. Màn hình đầu trả lời: **PR nào cần xử lý và còn bao nhiêu hàng chưa được phân bổ?** Thẩm mỹ nền sáng, khung hơi ngả xám và vùng bảng trắng kiểu `framed-sheet`; màu chỉ nhấn vào trạng thái và quyết định giá. Gợi ý từ `design-taste-frontend` (app screen, seed `1389186966`) kết hợp kiểm tra UX có kết quả phù hợp của `ui-ux-pro-max`: điều hướng bàn phím, chọn hàng loạt, ảo hóa danh sách.

Hai lượt tìm design system tự động của UI/UX Pro Max trả về cấu trúc landing page, nên không dùng bố cục/CTA đó cho ứng dụng nghiệp vụ.

## Bố cục desktop

```text
┌─────────────────────────────────────────────────────────────────────┐
│ PR2PO  [Đợt xử lý] [Báo giá] [PO]        Tìm PR...        Nhập file │  48px
├─────────────────────────────────────────────────────────────────────┤
│ Kỳ: 10–11/2026   Dự án ▾   Trạng thái ▾   Dán danh sách PR   Bộ lọc │  40px
├─────────────────────────────────────────────────────────────────────┤
│ □  PR / dự án      Mã hàng      Tên hàng     SL cần   Đã đặt   Còn │
│ □  ...             ...          ...             ...       ...   ... │
│                 Bảng cuộn dọc, chiếm toàn bộ chiều cao còn lại     │
├─────────────────────────────────────────────────────────────────────┤
│ 226 dòng · 38 PR · 12 dòng lỗi          Chọn 25 dòng  |  Xử lý    │  28px
└─────────────────────────────────────────────────────────────────────┘
```

- Không đặt hàng KPI hoặc banner lớn phía trên bảng. Thanh điều hướng 48px, hàng lọc khoảng 40px, tiêu đề bảng 32px, thanh trạng thái 28px.
- Mật độ mặc định 38–40px/hàng; tùy chọn gọn 34–36px/hàng. Chữ dữ liệu 13–14px, không giảm dưới 12px. Người dùng có thể tăng mật độ thoáng.
- Cột checkbox, PR và mã hàng cố định. Số lượng, tiền và ngày căn phải; số dùng tabular figures. Mã PR và mã hàng dùng font mono nếu cần, tên hàng dùng IBM Plex Sans.
- Sidebar chỉ mở rộng khi cần điều hướng; vùng so sánh giá dành tối đa chiều ngang cho cột NCC. Chi tiết dòng mở bằng panel hoặc dialog, không chiếm chiều cao bảng thường trực.
- Trạng thái rỗng, thiếu giá, giá bằng 0, lỗi import và dữ liệu cũ được hiển thị khác nhau. Không dùng màu làm tín hiệu duy nhất.

## Các màn hình chính

1. **Nhập file:** chọn loại nguồn/sheet, xem trước 50–100 dòng, đặt dòng bắt đầu, chọn cột tương ứng; PDF báo giá hiển thị thành bảng và dùng cùng bộ ánh xạ như Excel. Rà soát tổng số dòng mới/thay đổi/không đổi/trùng/lỗi trước khi xác nhận; tiến độ có thể hủy.
2. **Chọn PR:** lọc theo kỳ và dự án; checkbox dự án/từng PR; dán danh sách PR; có cảnh báo không tìm thấy/trùng; chọn hàng loạt áp dụng cho toàn bộ tập lọc theo quy tắc rõ ràng.
3. **So sánh:** giữ thông tin PR và mã hàng bên trái; mỗi NCC một cột giá và điều kiện; cột giá cũ và giá chốt ở cuối. Có thể chọn nhiều dòng để gán NCC, sao chép bảng hoặc dán phân bổ từ Excel; bản nháp tự lưu và có thể hoàn tác.
4. **PO nháp:** xem từng NCC, sửa phân bổ, mở lại đợt đã lưu; danh sách đợt gần đây cho biết số mã đã phân bổ, mã thiếu báo giá và PO đã tạo. Rà soát dòng chưa có báo giá, chưa phân bổ hoặc phân bổ thiếu trước khi xuất.
5. **Kho dữ liệu:** xem nguồn đã nhập, kiểm tra file nguồn không còn ở vị trí cũ, mã lặp trong báo giá và thời điểm sao lưu gần nhất; app nhắc sao lưu khi chưa có bản hoặc đã quá 7 ngày. Dùng menu chuột phải để hoàn tác riêng một lần nhập.
6. **Báo cáo tổng hợp:** chọn một đợt đã lưu để xem tổng giá trị, thiếu báo giá, phân bổ thiếu/vượt, báo giá quá hạn/cũ, giá nhập tay và giá bất thường. Chọn thẻ hoặc bộ lọc để tra danh sách chi tiết; ngưỡng lệch giá mặc định 30% so với giá mua gần nhất hoặc biên độ giữa các NCC, tuổi báo giá mặc định 90 ngày và đều có thể chỉnh. Mỗi dòng giải trình NCC, giá chốt so với giá thấp nhất, lý do chọn và nguồn báo giá (file, sheet/trang, dòng) nếu có.

Trong hướng dẫn nhanh có mục tiện ích và phím tắt: Ctrl+F tìm kiếm, Ctrl+S lưu đợt, Ctrl+Z hoàn tác, Esc đóng cửa sổ và `?` mở hướng dẫn.

Phím Tab đi qua điều khiển theo thứ tự nhìn; phím mũi tên/Space thao tác bảng theo khả năng của grid. Mọi nút biểu tượng có nhãn truy cập, focus nhìn rõ. Kiểm tra trên kích thước cửa sổ 1280×720 và 1440×900 trước khi duyệt giao diện.

## Kiểm tra khi bắt đầu dựng UI

- Dùng dữ liệu mẫu thật đã khử thông tin nhạy cảm; tổng tiền/số lượng phải cộng đúng.
- Chụp và xem lại các màn hình ở 1280×720 và 1440×900 bằng browser/Tauri, sửa lỗi bố cục trước khi ký duyệt.
- Đo số hàng nhìn thấy, cuộn bảng, thay đổi bộ lọc, chọn hàng loạt và thao tác chỉ bằng bàn phím.
- Giữ các gợi ý skill ở mức nguyên tắc; nghiệp vụ bảng dữ liệu và yêu cầu tiết kiệm chiều cao quyết định layout cuối.

