


use alkanes_runtime::storage::StoragePointer;
use alkanes_support::id::AlkaneId;
use anyhow::{anyhow, Result};
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

pub trait BytesPointer {
    fn set_bytes(&mut self, v: Vec<u8>);
    fn get_bytes(&self) -> Vec<u8>;
}

impl BytesPointer for StoragePointer {
    fn set_bytes(&mut self, v: Vec<u8>) {
        self.set(Arc::new(v));
    }
    fn get_bytes(&self) -> Vec<u8> {
        self.get().as_ref().clone()
    }
}

pub fn pack_id(id: &AlkaneId) -> Vec<u8> {
    let mut out = Vec::with_capacity(32);
    out.extend_from_slice(&id.block.to_le_bytes());
    out.extend_from_slice(&id.tx.to_le_bytes());
    out
}

pub fn unpack_id(data: &[u8]) -> Result<AlkaneId> {
    if data.len() != 32 {
        return Err(anyhow!("alkane id must be 32 bytes"));
    }
    Ok(AlkaneId {
        block: u128::from_le_bytes(data[0..16].try_into().unwrap()),
        tx: u128::from_le_bytes(data[16..32].try_into().unwrap()),
    })
}


pub fn id_from_storage(data: &[u8]) -> Result<AlkaneId> {
    unpack_id(data)
}

pub fn id_words(id: &AlkaneId) -> Vec<u128> {
    vec![id.block, id.tx]
}
