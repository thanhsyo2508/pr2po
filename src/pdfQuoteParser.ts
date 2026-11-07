import * as pdfjs from "pdfjs-dist/legacy/build/pdf.mjs";
import pdfWorkerUrl from "pdfjs-dist/legacy/build/pdf.worker.mjs?url";

pdfjs.GlobalWorkerOptions.workerSrc = pdfWorkerUrl;

export type PdfQuoteRow = {
  page: number;
  lineNumber: number;
  itemCode: string;
  quantity: string;
  unit: string;
  unitPrice: string;
  amount: string;
  note: string;
  cells: string[];
};

type PositionedText = { str: string; x: number; y: number };

const codePattern = /^[A-Z0-9]+(?:-[A-Z0-9]+)+$/i;
const moneyPattern = /^\d[\d,]*(?:\.\d+)?$/;

function pageRows(items: PositionedText[], page: number): PdfQuoteRow[] {
  const sorted = items.filter(item => item.str.trim()).sort((a, b) => b.y - a.y || a.x - b.x);
  const baselines: { y: number; items: PositionedText[] }[] = [];

  for (const item of sorted) {
    const baseline = baselines[baselines.length - 1];
    if (baseline && Math.abs(baseline.y - item.y) <= 2) {
      baseline.items.push(item);
      baseline.y = (baseline.y * (baseline.items.length - 1) + item.y) / baseline.items.length;
    } else {
      baselines.push({ y: item.y, items: [item] });
    }
  }

  const rows: PdfQuoteRow[] = [];
  for (const baseline of baselines) {
    const cells = baseline.items.sort((a, b) => a.x - b.x);
    const numberCell = cells.find(cell => cell.x < 100 && /^\d{1,3}$/.test(cell.str.trim()));
    const codeCell = cells.find(cell => cell.x >= 90 && cell.x < 300 && codePattern.test(cell.str.trim()));
    if (!numberCell || !codeCell) continue;

    const lineNumber = Number(numberCell.str.trim());
    const quantityCell = cells.find(cell => cell.x >= 300 && cell.x < 334 && moneyPattern.test(cell.str.trim()));
    const unitCell = cells.find(cell => cell.x >= 330 && cell.x < 365 && /^[a-z]{1,8}$/i.test(cell.str.trim()));
    if (!lineNumber || !quantityCell || !unitCell) continue;

    const priceCell = cells.find(cell => cell.x >= 360 && cell.x < 430 && moneyPattern.test(cell.str.trim()));
    const amountCell = cells.find(cell => cell.x >= 425 && cell.x < 480 && moneyPattern.test(cell.str.trim()));
    const note = cells.filter(cell => cell.x >= 465 && !moneyPattern.test(cell.str.trim()) && cell.str.trim() !== "-").map(cell => cell.str.trim()).join(" ");

    const values = [
      String(lineNumber),
      codeCell.str.trim(),
      quantityCell.str.trim(),
      unitCell.str.trim(),
      priceCell?.str.trim() ?? "",
      amountCell?.str.trim() ?? "",
      note,
      String(page),
    ];
    rows.push({
      page,
      lineNumber,
      itemCode: codeCell.str.trim(),
      quantity: quantityCell.str.trim(),
      unit: unitCell.str.trim(),
      unitPrice: priceCell?.str.trim() ?? "",
      amount: amountCell?.str.trim() ?? "",
      note,
      cells: values,
    });
  }
  return rows;
}

export async function parseQuotePdf(data: Uint8Array): Promise<{ pageCount: number; rows: PdfQuoteRow[] }> {
  const loadingTask = pdfjs.getDocument({ data, isEvalSupported: false });
  try {
    const document = await loadingTask.promise;
    const rows: PdfQuoteRow[] = [];
    for (let pageNumber = 1; pageNumber <= document.numPages; pageNumber += 1) {
      const page = await document.getPage(pageNumber);
      const content = await page.getTextContent();
      const items = content.items.flatMap(item => "str" in item ? [{ str: item.str, x: item.transform[4], y: item.transform[5] }] : []);
      rows.push(...pageRows(items, pageNumber));
      page.cleanup();
    }
    rows.sort((a, b) => a.page - b.page || a.lineNumber - b.lineNumber);
    if (!rows.length) throw new Error("Không tìm thấy bảng hàng hóa có thể đọc trong PDF. Nếu file là bản scan, cần OCR.");

    const numbered = rows.map(row => row.lineNumber);
    const duplicate = numbered.find((number, index) => numbered.indexOf(number) !== index);
    if (duplicate !== undefined) throw new Error(`Số thứ tự ${duplicate} xuất hiện nhiều lần; hãy kiểm tra PDF trước khi nhập.`);
    return { pageCount: document.numPages, rows };
  } finally {
    await loadingTask.destroy();
  }
}

export function quotePdfPreviewRows(rows: PdfQuoteRow[]): string[][] {
  return [
    ["STT", "Mã hàng", "Số lượng", "ĐVT", "Đơn giá", "Thành tiền", "Ghi chú", "Trang"],
    ...rows.map(row => row.cells),
  ];
}

