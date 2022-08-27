use super::process_list;

const MESSAGE_SIZE: usize = 64;

#[derive(Debug, Clone)]
pub enum IpcError {
    DeliveryError
}

#[derive(Debug, Clone)]
#[repr(u8)]
enum MessageType {
    // A raw 55 byte message
    Raw = 0,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Message {
    to_or_from_id: usize,
    ty: MessageType,
    data: [u8; 55]
}

/// This function is never invoked but used to verify at compile time that a message is 64 bytes
#[allow(dead_code)]
unsafe fn assert_msg_size(msg: Message) {
    core::mem::transmute::<Message, [u8;MESSAGE_SIZE]>(msg);
}

pub fn send_message(message: &Message) -> Result<(), IpcError> {
    let recipient = process_list().get(message.to_or_from_id).ok_or(IpcError::DeliveryError)?;

    {
        recipient.write().add_pending_message(message.clone());
    }
    Ok(())
}
