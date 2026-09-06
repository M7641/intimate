import { printElement } from '@/lib/print_element.ts';
import { MdLocalPrintshop } from "react-icons/md";


export function PrintButton(
    {
      DivIdToSave,
      innerText = null,
      className = ''
    }: {
      DivIdToSave: string,
      innerText?: string | null,
      className?: string
    }
) {

  let innerTextJSX = null;
  if (!innerText) {
    innerTextJSX = <div className='flex items-center justify-center'>
      <span>Print</span>
      <MdLocalPrintshop className='ml-1' />
    </div>;
  } else {
    innerTextJSX = innerText;
  }

  return (
      <button
        onClick={() => {printElement(DivIdToSave)}}
        className={`
          mr-[16px] rounded-full font-semibold text-[14px]
          min-w-[80px] bg-(--peak-blue) text-white transition
          h-[32px] ${className}
        `}
      >{innerTextJSX}</button>
  );
}
