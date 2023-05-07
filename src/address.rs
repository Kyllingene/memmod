use crate::POINTER_WIDTH;

pub trait Address {
    fn from_bytes(bytes: [u8; POINTER_WIDTH]) -> Self;
}

impl Address for usize {
    fn from_bytes(bytes: [u8; POINTER_WIDTH]) -> Self {
        let mut ptr = 0;

        for (i, byte) in bytes.iter().enumerate() {
            ptr |= (*byte as usize) << (i * 8);
        }
    
        ptr
    }
}
