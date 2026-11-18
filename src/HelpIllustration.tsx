import "./HelpIllustration.css";

type HelpIllustrationProps = { step: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 };

const line = (x: number, y: number, width: number, key: string) => (
  <rect key={key} x={x} y={y} width={width} height="5" rx="2.5" fill="#dce6e6" />
);

export function HelpIllustration({ step }: HelpIllustrationProps) {
  return (
    <svg className="help-illustration" viewBox="0 0 360 104" role="img" aria-label={`Hình minh họa bước ${step}`}>
      <rect x=".5" y=".5" width="359" height="103" rx="7" fill="#fff" stroke="#dce7e7" />
      <rect x="1" y="1" width="358" height="20" rx="6" fill="#f5f8f8" />
      <circle cx="13" cy="11" r="3" fill="#9bb3b4" />
      <circle cx="23" cy="11" r="3" fill="#c5d5d5" />
      <circle cx="33" cy="11" r="3" fill="#dce6e6" />
      <text x="48" y="14" fill="#50666a" fontSize="8" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">
        {step === 1 && "DỮ LIỆU PR"}
        {step === 2 && "CHỌN PHẠM VI"}
        {step === 3 && "SO SÁNH & PHÂN BỔ"}
        {step === 4 && "ĐỢT MUA HÀNG"}
        {step === 5 && "MAPPING DỰ ÁN · DỮ LIỆU MINH HỌA"}
        {step === 6 && "NHẬP & QUẢN LÝ DỮ LIỆU"}
        {step === 7 && "BÁO CÁO TỔNG HỢP"}
        {step === 8 && "SAO LƯU & CẬP NHẬT"}
      </text>

      {step === 1 && <>
        <rect x="12" y="29" width="68" height="12" rx="3" fill="#edf4f4" />
        <rect x="86" y="29" width="57" height="12" rx="3" fill="#edf4f4" />
        <rect x="149" y="29" width="93" height="12" rx="3" fill="#f6f8f8" stroke="#e4ebeb" />
        <rect x="13" y="51" width="7" height="7" rx="1" fill="#287781" />
        <path d="m15 54 2 2 4-5" fill="none" stroke="#fff" strokeWidth="1.5" />
        <rect x="27" y="51" width="72" height="5" rx="2.5" fill="#708286" />
        {line(115, 51, 55, "a")}{line(184, 51, 24, "b")}
        <path d="M12 66h336M12 81h336" stroke="#edf1f1" />
        <rect x="13" y="71" width="7" height="7" rx="1" fill="#fff" stroke="#b8c8c9" />
        <rect x="27" y="72" width="62" height="5" rx="2.5" fill="#dce6e6" />
        {line(115, 72, 71, "c")}{line(205, 72, 25, "d")}
        <rect x="284" y="85" width="63" height="12" rx="3" fill="#287781" />
        <text x="294" y="93" fill="#fff" fontSize="6.5" fontFamily="IBM Plex Sans, sans-serif">SO SÁNH ĐÃ CHỌN</text>
      </>}

      {step === 2 && <>
        <rect x="12" y="28" width="336" height="19" rx="4" fill="#f6f9f9" stroke="#e5ecec" />
        <rect x="20" y="34" width="19" height="8" rx="2" fill="#dcebea" />
        <text x="45" y="40" fill="#52666a" fontSize="7" fontFamily="IBM Plex Sans, sans-serif">file-PR-mau.xlsx · Sheet1</text>
        <rect x="12" y="54" width="336" height="17" rx="3" fill="#f5f8f8" />
        <text x="19" y="65" fill="#64777a" fontSize="6.5" fontFamily="IBM Plex Sans, sans-serif">DÒNG</text>
        <text x="61" y="65" fill="#64777a" fontSize="6.5" fontFamily="IBM Plex Sans, sans-serif">SỐ PR</text>
        <text x="145" y="65" fill="#64777a" fontSize="6.5" fontFamily="IBM Plex Sans, sans-serif">MÃ HÀNG</text>
        <text x="234" y="65" fill="#64777a" fontSize="6.5" fontFamily="IBM Plex Sans, sans-serif">SỐ LƯỢNG</text>
        <rect x="13" y="73" width="334" height="13" rx="2" fill="#eef7f4" stroke="#d4e9df" />
        <text x="20" y="82" fill="#25734f" fontSize="7" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">6</text>
        <text x="61" y="82" fill="#52666a" fontSize="7" fontFamily="IBM Plex Sans, sans-serif">Tiêu đề được chọn</text>
        <rect x="275" y="88" width="72" height="10" rx="3" fill="#287781" />
        <text x="285" y="95" fill="#fff" fontSize="6" fontFamily="IBM Plex Sans, sans-serif">RÀ SOÁT DỮ LIỆU</text>
      </>}

      {step === 3 && <>
        <rect x="12" y="29" width="97" height="15" rx="3" fill="#f5f8f8" />
        <text x="18" y="39" fill="#51666a" fontSize="7" fontFamily="IBM Plex Sans, sans-serif">ITEM-001 · SL cần 12</text>
        <rect x="117" y="27" width="102" height="42" rx="4" fill="#edf6f3" stroke="#8bb9aa" />
        <text x="125" y="39" fill="#315d54" fontSize="7" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">NCC A</text>
        <text x="125" y="53" fill="#315d54" fontSize="8" fontWeight="700" fontFamily="IBM Plex Sans, sans-serif">24.500 ₫</text>
        <text x="125" y="63" fill="#718783" fontSize="6" fontFamily="IBM Plex Sans, sans-serif">Đang được chọn</text>
        <rect x="227" y="27" width="102" height="42" rx="4" fill="#fff" stroke="#e2eaea" />
        <text x="235" y="39" fill="#52666a" fontSize="7" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">NCC B</text>
        <text x="235" y="53" fill="#52666a" fontSize="8" fontWeight="700" fontFamily="IBM Plex Sans, sans-serif">26.000 ₫</text>
        <rect x="117" y="76" width="102" height="17" rx="3" fill="#fff" stroke="#dce7e7" />
        <text x="126" y="87" fill="#52666a" fontSize="6.5" fontFamily="IBM Plex Sans, sans-serif">SL 8 · Đơn giá 24.500</text>
        <rect x="227" y="76" width="102" height="17" rx="3" fill="#fff" stroke="#dce7e7" />
        <text x="236" y="87" fill="#52666a" fontSize="6.5" fontFamily="IBM Plex Sans, sans-serif">SL 4 · Đơn giá 26.000</text>
      </>}

      {step === 4 && <>
        <rect x="12" y="29" width="95" height="26" rx="4" fill="#f5f8f8" stroke="#e5ecec" />
        <text x="20" y="40" fill="#7b8c8f" fontSize="6.5" fontFamily="IBM Plex Sans, sans-serif">TỔNG SỐ PO</text>
        <text x="20" y="50" fill="#334f54" fontSize="9" fontWeight="700" fontFamily="IBM Plex Sans, sans-serif">02</text>
        <rect x="115" y="29" width="105" height="26" rx="4" fill="#f5f8f8" stroke="#e5ecec" />
        <text x="123" y="40" fill="#7b8c8f" fontSize="6.5" fontFamily="IBM Plex Sans, sans-serif">MÃ THIẾU BÁO GIÁ</text>
        <text x="123" y="50" fill="#9b6d2e" fontSize="9" fontWeight="700" fontFamily="IBM Plex Sans, sans-serif">01</text>
        <rect x="228" y="29" width="120" height="26" rx="4" fill="#287781" />
        <text x="241" y="45" fill="#fff" fontSize="7" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">RÀ SOÁT & XUẤT</text>
        <path d="M12 64h336" stroke="#edf1f1" />
        <circle cx="21" cy="76" r="5" fill="#eaf2f3" />
        <text x="32" y="78" fill="#52666a" fontSize="7" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">NCC A</text>
        {line(94, 74, 72, "e")}
        <text x="291" y="78" fill="#315d54" fontSize="7" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">294.000 ₫</text>
        <path d="M12 86h336" stroke="#edf1f1" />
        <circle cx="21" cy="95" r="5" fill="#eaf2f3" />
        <text x="32" y="97" fill="#52666a" fontSize="7" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">NCC B</text>
        {line(94, 93, 72, "f")}
        <text x="291" y="97" fill="#315d54" fontSize="7" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">104.000 ₫</text>
      </>}

      {step === 5 && <>
        <rect x="12" y="28" width="111" height="15" rx="3" fill="#f5f8f8" stroke="#e5ecec" />
        <text x="19" y="38" fill="#52666a" fontSize="6.5" fontFamily="IBM Plex Sans, sans-serif">Nguồn: PO mua hàng · Sheet1</text>
        <rect x="12" y="48" width="336" height="13" rx="2" fill="#eaf1f1" />
        <text x="19" y="57" fill="#52666a" fontSize="6" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">DỰ ÁN</text>
        <text x="77" y="57" fill="#52666a" fontSize="6" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">SỐ PR</text>
        <text x="137" y="57" fill="#52666a" fontSize="6" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">MÃ HÀNG</text>
        <text x="222" y="57" fill="#52666a" fontSize="6" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">NCC</text>
        <text x="285" y="57" fill="#52666a" fontSize="6" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">SL ĐẶT</text>
        <rect x="13" y="63" width="334" height="13" rx="2" fill="#eef7f4" stroke="#cde5d8" />
        <text x="19" y="72" fill="#25734f" fontSize="6.3" fontFamily="IBM Plex Sans, sans-serif">DEMO-PJ-01</text>
        <text x="77" y="72" fill="#52666a" fontSize="6.3" fontFamily="IBM Plex Sans, sans-serif">PR-DEMO-01</text>
        <text x="137" y="72" fill="#52666a" fontSize="6.3" fontFamily="IBM Plex Sans, sans-serif">ITEM-001</text>
        <text x="222" y="72" fill="#52666a" fontSize="6.3" fontFamily="IBM Plex Sans, sans-serif">NCC A</text>
        <text x="293" y="72" fill="#52666a" fontSize="6.3" fontFamily="IBM Plex Sans, sans-serif">8</text>
        <rect x="13" y="80" width="334" height="13" rx="2" fill="#fff" stroke="#e6eded" />
        <text x="19" y="89" fill="#52666a" fontSize="6.3" fontFamily="IBM Plex Sans, sans-serif">DEMO-PJ-01</text>
        <text x="77" y="89" fill="#52666a" fontSize="6.3" fontFamily="IBM Plex Sans, sans-serif">PR-DEMO-01</text>
        <text x="137" y="89" fill="#52666a" fontSize="6.3" fontFamily="IBM Plex Sans, sans-serif">ITEM-002</text>
        <text x="222" y="89" fill="#52666a" fontSize="6.3" fontFamily="IBM Plex Sans, sans-serif">NCC B</text>
        <text x="293" y="89" fill="#52666a" fontSize="6.3" fontFamily="IBM Plex Sans, sans-serif">4</text>
      </>}

      {(step === 6 || step === 7 || step === 8) && <>
        <rect x="12" y="29" width="102" height="29" rx="4" fill="#eef6f4" stroke="#d9e9e5" />
        <text x="21" y="41" fill="#60787a" fontSize="6.5" fontFamily="IBM Plex Sans, sans-serif">{step === 6 ? "NGUỒN ĐÃ NHẬP" : step === 7 ? "ĐỢT ĐANG XEM" : "BẢN SAO LƯU"}</text>
        <text x="21" y="52" fill="#315e62" fontSize="8" fontWeight="700" fontFamily="IBM Plex Sans, sans-serif">{step === 6 ? "12 tệp" : step === 7 ? "Đợt tháng 10" : "Cục bộ · ZIP"}</text>
        <rect x="121" y="29" width="107" height="29" rx="4" fill="#f8faf9" stroke="#e2eaea" />
        <text x="130" y="41" fill="#718286" fontSize="6.5" fontFamily="IBM Plex Sans, sans-serif">{step === 6 ? "DÒNG ĐÃ LỌC" : step === 7 ? "THIẾU BÁO GIÁ" : "TRẠNG THÁI"}</text>
        <text x="130" y="52" fill="#315e62" fontSize="8" fontWeight="700" fontFamily="IBM Plex Sans, sans-serif">{step === 6 ? "1.240 dòng" : step === 7 ? "08 mã" : "Đã kiểm tra"}</text>
        <rect x="235" y="29" width="113" height="29" rx="4" fill="#287781" />
        <text x="246" y="47" fill="#fff" fontSize="7" fontWeight="600" fontFamily="IBM Plex Sans, sans-serif">{step === 6 ? "XEM CHI TIẾT" : step === 7 ? "MỞ CẢNH BÁO" : "KIỂM TRA CẬP NHẬT"}</text>
        <path d="M12 68h336M12 85h336" stroke="#edf1f1" />
        <circle cx="21" cy="77" r="4" fill="#dcebea" />
        <text x="32" y="79" fill="#52666a" fontSize="7" fontFamily="IBM Plex Sans, sans-serif">{step === 6 ? "File nguồn · Sheet · thời điểm nhập" : step === 7 ? "Mã hàng · PR · NCC · giá đã chọn" : "Phiên bản · ngày · chữ ký gói"}</text>
        <circle cx="21" cy="94" r="4" fill="#dcebea" />
        <text x="32" y="96" fill="#52666a" fontSize="7" fontFamily="IBM Plex Sans, sans-serif">{step === 6 ? "Xem nhanh / gỡ đúng nguồn" : step === 7 ? "Bấm dòng để truy về nguồn" : "Khôi phục khi cần"}</text>
      </>}
    </svg>
  );
}
