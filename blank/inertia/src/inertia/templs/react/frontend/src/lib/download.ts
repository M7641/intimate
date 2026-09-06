function convertToCSV(objArray: string, includesHeader: boolean = true) {
  const array = typeof objArray !== 'object' ? JSON.parse(objArray) : objArray;
  let str = '';

  let headerLine = '';
  if (includesHeader) {
        for (let index in array[0]) {
            if (headerLine !== '') headerLine += ',';

            headerLine += index;
        }
        str += headerLine + '\r\n';
    }

  for (let i = 0; i < array.length; i++) {
    let line = '';
    for (let index in array[i]) {
      if (line !== '') line += ',';

      line += array[i][index];
    }
    str += line + '\r\n';
  }

  return str;
};

export function downloadCSV(
  data: string,
  fileName: string
) {
  const csvData = new Blob([convertToCSV(data)], { type: 'text/csv' });
  const csvURL = URL.createObjectURL(csvData);
  const link = document.createElement('a');
  link.href = csvURL;
  link.download = `${fileName}.csv`;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
};
