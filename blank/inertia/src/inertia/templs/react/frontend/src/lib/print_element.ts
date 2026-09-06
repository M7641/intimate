import html2canvas from 'html2canvas-pro';
import jsPDF from 'jspdf';

export function printElement(elementId: string) {
    const element = document.getElementById(elementId);
    if (!element) {
        console.error(`Element with ID ${elementId} not found.`);
        return;
    }
    html2canvas(element).then(canvas => {
        const imgData = canvas.toDataURL('image/jpeg', 0.98);

        const pdf = new jsPDF();

        const pdfWidth = pdf.internal.pageSize.getWidth();
        const pdfHeight = pdf.internal.pageSize.getHeight();

        const pageHeight = (canvas.height * pdfWidth) / canvas.width;
        let heightLeft = pageHeight;
        let position = 0;

        // Add new pages if the content exceeds one page
        while (heightLeft > 0) {
            pdf.addImage(imgData, 'JPEG', 0, position, pdfWidth, pageHeight);
            heightLeft -= pdfHeight;
            position -= pdfHeight;
            if (heightLeft > 0) {
                pdf.addPage();
            }
        }

        pdf.save('report.pdf');
    });
}
