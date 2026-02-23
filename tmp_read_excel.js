const XLSX = require('xlsx');
try {
    const workbook = XLSX.readFile('SugarLand_20 (excel to manager).xlsx');
    const sheet_name_list = workbook.SheetNames;
    const xlData = XLSX.utils.sheet_to_json(workbook.Sheets[sheet_name_list[0]], { header: 1 });
    console.log("Sheet names:", sheet_name_list);
    console.log("Headers (Row 1):", xlData[0]);
    console.log("Data (Row 2):", xlData[1]);
} catch (e) {
    console.error("Error reading file:", e.message);
}
