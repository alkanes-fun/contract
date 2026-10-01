










use metashrew_support::byte_view::ByteView;
use ruint::Uint;




pub const PROTOCOL_FEE_SHARE_BPS: u128 = 2_000;
pub const BPS: u128 = 10_000;

pub type U256 = Uint<256, 4>;
pub trait Sqrt {
    fn sqrt(self) -> Self;
}
impl Sqrt for U256 {
    fn sqrt(self) -> U256 {
        if self.is_zero() {
            return self;
        }

        let mut x = self;
        let mut y = (self + U256::ONE) / U256::from(2);


        while y < x {
            x = y;
            y = (self / x + x) / U256::from(2);
        }

        x
    }
}


#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StorableU256(pub U256);

impl ByteView for StorableU256 {
    fn from_bytes(v: Vec<u8>) -> Self {
        assert!(v.len() == 32, "Expected a byte vector of length 32.");

        let mut bytes_array = [0u8; 32];
        bytes_array.copy_from_slice(&v);
        StorableU256(U256::from_le_bytes(bytes_array))
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.0.to_le_bytes::<32>().to_vec()
    }

    fn maximum() -> Self {
        StorableU256(U256::MAX)
    }

    fn zero() -> Self {
        StorableU256(U256::ZERO)
    }
}

impl From<U256> for StorableU256 {
    fn from(value: U256) -> Self {
        StorableU256(value)
    }
}

impl From<StorableU256> for U256 {
    fn from(value: StorableU256) -> Self {
        value.0
    }
}
