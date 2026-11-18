import { Modal, type ModalCloseReason, type ModalProps } from '../..';

export function PublishModal(props: React.PropsWithChildren<ModalProps>) {
  const { onBack, onClose, size = 'small', ...rest } = props;

  const handleClose = (event?: React.MouseEvent<HTMLButtonElement>, reason?: ModalCloseReason) => {
    if (reason === 'backdropClick') return;
    onClose?.(event);
  };

  return (
    <Modal
      {...rest}
      size={size}
      onBack={onBack}
      onClose={handleClose}
    >
      {props.children}
    </Modal>
  );
}
