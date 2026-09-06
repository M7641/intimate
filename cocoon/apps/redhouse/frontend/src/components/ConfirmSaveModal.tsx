import ReactModal from 'react-modal';

import { FaCheck } from "react-icons/fa";
import { MdOutlineRefresh } from "react-icons/md";


// Set the app element for accessibility
if (typeof document !== 'undefined') {
  ReactModal.setAppElement('#root');
}

export default function ConfirmSaveModal({
  isOpen,
  message,
  onRequestClose,
  setTriggerSave,
}: {
  isOpen: boolean;
  message: string;
  onRequestClose: () => void;
  setTriggerSave: (value: boolean) => void;
}) {

  const modalContent = (
    <div className="p-4 min-w-[400px] min-h-[120px] flex flex-col justify-center items-center">
        <p className='text-[20px] font-semibold'>{message}</p>
        <div className="flex gap-2">
            <button
              className="base-modal-button modal-button-green"
              onClick={() => {
                setTriggerSave(true);
                onRequestClose();
              }}
            >
              <FaCheck className="mr-2" />
              <span>Confirm Save</span>
            </button>
            <button
                className="base-modal-button modal-button-dark"
                onClick={onRequestClose}
            >
              <MdOutlineRefresh className="mr-2" />
              <span>Go Back</span>
            </button>
        </div>
    </div>
  )

  return (
    <ReactModal
      isOpen={isOpen}
      onRequestClose={onRequestClose}
      contentLabel="Model KPIs"
      className={`
        absolute top-[60px] left-[50%] right-auto bottom-auto translate-x-[-50%]
        bg-[rgba(0,0,51,0.9)] text-white rounded-[25px] py-[15px] px-[40px]
        max-w-[800px] overflow-y-auto outline-none
      `}
      closeTimeoutMS={250}
    >{modalContent}
    </ReactModal>
  );
}
