pub mod frame;
pub mod r#virtual;

#[cfg(target_arch = "x86_64")]
pub struct Page([u8; Self::SIZE]);

impl Page {
	pub const SIZE: usize = 4096;
	pub const OFFSET_BITS: u8 = 12;
	pub const OFFSET_MASK: usize = 0xfff;
}